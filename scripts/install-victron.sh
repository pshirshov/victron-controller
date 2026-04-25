#!/usr/bin/env bash
# Build the armv7 binary via `nix build .#victron-controller-armv7`,
# copy it + config to the Venus, and install a `/data/rcS.local` hook
# so it survives firmware upgrades (per SPEC §5.1).
#
# Usage:
#   ./scripts/install-victron.sh user@victron-host [ssh-opts...]
#
# Requirements on the build host (local):
#   - Nix with flakes enabled. The flake's `victron-controller-armv7`
#     derivation builds the bundle, cross-compiles the binary, and
#     patches the ELF interpreter to /lib/ld-linux-armhf.so.3.
#
# Requirements on the target (Venus):
#   - SSH access as root (the default on stock Venus).
#   - `/data/` mounted and writable.
#
# What this does on the target:
#   - Creates /data/opt/victron-controller/bin/ and /data/etc/victron-controller/
#   - Copies the release binary into bin/
#   - Writes a minimal config.toml template to etc/ if none exists
#   - Writes a systemd-style run script + ensures /data/rcS.local starts it on boot
#
# The service is started under `daemontools`-style supervision via
# Venus's /service/ directory, mirroring how stock Venus services run.
#
# Run with `--dry-run` to print what would happen without touching the
# target.

set -euo pipefail

DRY_RUN=0
for a in "$@"; do
  case "$a" in
    --dry-run) DRY_RUN=1 ;;
  esac
done

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 user@victron-host [--dry-run] [ssh-opts...]" >&2
  exit 2
fi

TARGET="$1"; shift
SSH_OPTS=()
for a in "$@"; do
  case "$a" in
    --dry-run) ;;
    *) SSH_OPTS+=("$a") ;;
  esac
done

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

# ----- build --------------------------------------------------------------
echo "[local] building via nix build .#victron-controller-armv7..."
NIX_OUT=$(nix build --no-link --print-out-paths .#victron-controller-armv7)
NIX_BIN="$NIX_OUT/bin/victron-controller"
if [[ ! -f "$NIX_BIN" ]]; then
  echo "ERROR: $NIX_BIN not found after nix build" >&2
  exit 3
fi
# Copy out of /nix/store (read-only, perms 0555) into a writable path
# so the optional UPX step can rewrite it in place. ELF interpreter
# is already /lib/ld-linux-armhf.so.3 (postFixup in the derivation).
BINARY_PATH="$(mktemp -t victron-controller.XXXXXX)"
cp "$NIX_BIN" "$BINARY_PATH"
chmod u+w "$BINARY_PATH"
echo "[local] nix output: $NIX_BIN"

# UPX-compress the binary to minimise eMMC write volume on each deploy
# and cut scp transfer time. UPX decompresses in-process on startup
# (~50 ms on ARMv7) — insignificant next to our event-driven steady
# state. Skip if UPX is absent or the binary already looks compressed
# (first 4 bytes aren't the ELF magic sometimes after UPX, so we
# check by size: a `NO_UPX=1 ./scripts/install-victron.sh ...` escape
# hatch covers the rare case where UPX mangles a binary).
UPX_BEFORE_SIZE=$(stat -c %s "$BINARY_PATH" 2>/dev/null || wc -c < "$BINARY_PATH")
if [[ "${NO_UPX:-0}" == "1" ]]; then
  echo "[local] NO_UPX=1 — skipping UPX compression"
elif command -v upx >/dev/null 2>&1; then
  # --lzma takes longer to compress but gives ~10-20% smaller output
  # than default deflate; worth it for a once-per-deploy cost.
  # -q keeps output terse; -9 = best ratio at a small cpu cost.
  upx --lzma -q -9 "$BINARY_PATH" >/dev/null 2>&1 || {
    echo "[local] WARNING: upx failed; continuing with uncompressed binary"
  }
  UPX_AFTER_SIZE=$(stat -c %s "$BINARY_PATH" 2>/dev/null || wc -c < "$BINARY_PATH")
  echo "[local] UPX: $((UPX_BEFORE_SIZE / 1024)) KiB → $((UPX_AFTER_SIZE / 1024)) KiB"
else
  echo "[local] (upx not found — re-enter nix develop to get it)"
fi

echo "[local] $(file "$BINARY_PATH" 2>/dev/null || echo "built $BINARY_PATH")"

# ----- upload + install ---------------------------------------------------
REMOTE_INSTALL_DIR=/data/opt/victron-controller
REMOTE_CONFIG_DIR=/data/etc/victron-controller
# Logs go to tmpfs (/var/volatile is the real tmpfs mount on Venus;
# /var/log is a symlink to /data/log — writing there would wear flash).
# No flash wear, volatile on reboot. The MQTT log publisher is the
# authoritative archive; this dir is just a local tail buffer.
# Capped at 2 MiB below (s524288 n4).
REMOTE_LOG_DIR=/var/volatile/log/victron-controller
# /service is tmpfs on Venus — contents vanish every reboot. We keep
# the canonical run scripts in $REMOTE_SERVICE_STAGE (on /data), and
# the rcS.local hook copies them into /service on boot.
REMOTE_SERVICE_DIR=/service/victron-controller
REMOTE_SERVICE_STAGE=/data/opt/victron-controller/service-stage

CONFIG_TEMPLATE="$REPO_ROOT/config.example.toml"
if [[ ! -f "$CONFIG_TEMPLATE" ]]; then
  echo "ERROR: $CONFIG_TEMPLATE missing" >&2
  exit 4
fi

# Common ssh options. `-o ServerAliveInterval=30` keeps the session
# healthy over NAT / idle timers. `-o ConnectTimeout=15` fails fast
# on unreachable hosts instead of hanging. No `-T` — it's redundant
# when passing a command and has been observed to hang on some Venus
# ssh setups (interaction with how busybox-sshd multiplexes stdin).
SSH_COMMON_OPTS=(-o ServerAliveInterval=30 -o ServerAliveCountMax=3 -o ConnectTimeout=15)

remote() {
  if [[ $DRY_RUN -eq 1 ]]; then
    printf "[dry-run] ssh %s %q\n" "$TARGET" "$*"
  else
    # `-n` redirects stdin from /dev/null — we never pipe data into
    # these one-shot command calls, and redirecting stdin avoids the
    # ssh client holding the terminal's stdin open.
    ssh -n "${SSH_COMMON_OPTS[@]}" "${SSH_OPTS[@]}" "$TARGET" "$@"
  fi
}

copy() {
  local src="$1" dst="$2"
  if [[ $DRY_RUN -eq 1 ]]; then
    printf "[dry-run] scp %s %s:%s\n" "$src" "$TARGET" "$dst"
  else
    scp "${SSH_COMMON_OPTS[@]}" "${SSH_OPTS[@]}" "$src" "$TARGET:$dst"
  fi
}

echo "[remote] creating directories..."
# $REMOTE_LOG_DIR is on tmpfs (/var/volatile/log) — recreated by log/run
# at service start, so we do NOT pre-create it here.
# $REMOTE_SERVICE_DIR is on tmpfs (/service) — recreated by the
# rcS.local hook on boot, so we do NOT pre-create it here either.
remote "mkdir -p $REMOTE_INSTALL_DIR/bin $REMOTE_CONFIG_DIR $REMOTE_SERVICE_STAGE/log"

echo "[remote] uploading binary..."
copy "$BINARY_PATH" "$REMOTE_INSTALL_DIR/bin/victron-controller.new"
remote "chmod +x $REMOTE_INSTALL_DIR/bin/victron-controller.new && mv $REMOTE_INSTALL_DIR/bin/victron-controller.new $REMOTE_INSTALL_DIR/bin/victron-controller"

echo "[remote] uploading config template (only if config.toml is absent)..."
copy "$CONFIG_TEMPLATE" "$REMOTE_CONFIG_DIR/config.example.toml"
remote "test -f $REMOTE_CONFIG_DIR/config.toml || cp $REMOTE_CONFIG_DIR/config.example.toml $REMOTE_CONFIG_DIR/config.toml"

# daemontools run script
RUN_SCRIPT="#!/bin/sh
exec 2>&1
exec $REMOTE_INSTALL_DIR/bin/victron-controller --config $REMOTE_CONFIG_DIR/config.toml
"
# Venus 3.70 ships `multilog` (djb daemontools) but not `svlogd` (runit).
# Both rotate + timestamp stdin → a log dir; multilog's flags are:
#   t           = prepend TAI64N timestamps to every line
#   s524288     = rotate when current file hits 512 KiB
#   n4          = keep up to 4 rotated files
# Total cap: 512 KiB × 4 = 2 MiB. Target lives on tmpfs so we mkdir
# at service start — /var/log is wiped on reboot.
RUN_LOG_SCRIPT="#!/bin/sh
mkdir -p $REMOTE_LOG_DIR
exec multilog t s524288 n4 $REMOTE_LOG_DIR
"

echo "[remote] writing daemontools run scripts to stage dir (persistent)..."
# The stage dir lives on /data so the canonical scripts survive across
# reboots. The rcS.local hook (below) copies them into /service, which
# is on tmpfs and wiped on every boot.
remote "cat > $REMOTE_SERVICE_STAGE/run.new <<'EOF'
$RUN_SCRIPT
EOF
chmod +x $REMOTE_SERVICE_STAGE/run.new && mv $REMOTE_SERVICE_STAGE/run.new $REMOTE_SERVICE_STAGE/run"

remote "cat > $REMOTE_SERVICE_STAGE/log/run.new <<'EOF'
$RUN_LOG_SCRIPT
EOF
chmod +x $REMOTE_SERVICE_STAGE/log/run.new && mv $REMOTE_SERVICE_STAGE/log/run.new $REMOTE_SERVICE_STAGE/log/run"

# rcS.local hook — copies the staged run scripts into /service on every
# boot, since /service is tmpfs. Also handles the "service dir exists
# but empty" case, which happens if svscan already recreated an empty
# dir before we got here.
RCS_MARKER="# victron-controller"
RCS_BLOCK="$RCS_MARKER
if [ -d $REMOTE_SERVICE_STAGE ] && [ ! -x $REMOTE_SERVICE_DIR/run ]; then
  mkdir -p $REMOTE_SERVICE_DIR/log
  cp $REMOTE_SERVICE_STAGE/run     $REMOTE_SERVICE_DIR/run
  cp $REMOTE_SERVICE_STAGE/log/run $REMOTE_SERVICE_DIR/log/run
  chmod +x $REMOTE_SERVICE_DIR/run $REMOTE_SERVICE_DIR/log/run
  # /service is watched by svscan — once the run scripts exist, the
  # supervisor picks them up within a few seconds.
fi
# /end victron-controller"

echo "[remote] installing /data/rcS.local hook (replacing any existing block)..."
# Strip any existing block (from marker to /end marker, inclusive),
# then append the current version. This keeps the hook in sync with
# whatever this install script declares — earlier installs may have
# left a stale block behind.
remote "touch /data/rcS.local && sed -i '/^$RCS_MARKER\$/,/^# \\/end victron-controller\$/d' /data/rcS.local && cat >> /data/rcS.local <<'EOF'

$RCS_BLOCK
EOF"

echo "[remote] seeding /service from stage dir (so the service starts now)..."
remote "mkdir -p $REMOTE_SERVICE_DIR/log && cp $REMOTE_SERVICE_STAGE/run $REMOTE_SERVICE_DIR/run && cp $REMOTE_SERVICE_STAGE/log/run $REMOTE_SERVICE_DIR/log/run && chmod +x $REMOTE_SERVICE_DIR/run $REMOTE_SERVICE_DIR/log/run"

# Restart service (svc -t gracefully cycles it; on first install, the
# supervisor picks the new dir up automatically within a few seconds).
echo "[remote] restarting service..."
remote "svc -t $REMOTE_SERVICE_DIR 2>/dev/null || true"

echo "[local] done."
echo
echo "Tail the logs on the Venus with:"
echo "  ssh $TARGET 'tail -f $REMOTE_LOG_DIR/current'"
echo
echo "Stop the service with:"
echo "  ssh $TARGET 'svc -d $REMOTE_SERVICE_DIR'"
echo
echo "Start it again with:"
echo "  ssh $TARGET 'svc -u $REMOTE_SERVICE_DIR'"
