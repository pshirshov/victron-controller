#!/usr/bin/env bash
# Build the armv7 binary, copy it + config to the Venus, and install a
# `/data/rcS.local` hook so it survives firmware upgrades (per SPEC §5.1).
#
# Usage:
#   ./scripts/install-victron.sh user@victron-host [ssh-opts...]
#
# Requirements on the build host (local):
#   - A Rust toolchain with the `armv7-unknown-linux-gnueabihf` target.
#     `nix develop` provides it; if you run outside the flake, you'll
#     need `rustup target add armv7-unknown-linux-gnueabihf` and a
#     matching linker (see flake.nix's shellHook for the linker env var).
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

TARGET_TRIPLE="armv7-unknown-linux-gnueabihf"
BINARY_PATH="target/${TARGET_TRIPLE}/release/victron-controller"

# ----- build --------------------------------------------------------------
echo "[local] building web frontend..."
"$REPO_ROOT/scripts/build-web.sh"

echo "[local] building ${TARGET_TRIPLE} release..."
cargo build -p victron-controller-shell --release --target "${TARGET_TRIPLE}"

if [[ ! -f "$BINARY_PATH" ]]; then
  echo "ERROR: $BINARY_PATH not found after build" >&2
  exit 3
fi

# Rewrite the ELF interpreter. The nix cross-toolchain bakes in a
# nix-store path for /ld-linux-armhf.so.3 which doesn't exist on the
# Venus. Patchelf to the standard location Venus provides.
if command -v patchelf >/dev/null 2>&1; then
  PATCHED_BINARY="${BINARY_PATH}.patched"
  cp "$BINARY_PATH" "$PATCHED_BINARY"
  patchelf --set-interpreter /lib/ld-linux-armhf.so.3 "$PATCHED_BINARY"
  patchelf --set-rpath '' "$PATCHED_BINARY"
  BINARY_PATH="$PATCHED_BINARY"
  echo "[local] patchelf'd ELF interpreter to /lib/ld-linux-armhf.so.3"
else
  echo "[local] WARNING: patchelf not found on \$PATH; binary ELF interpreter still points at nix-store."
  echo "[local]          If it refuses to run on the Venus, install patchelf or \`nix develop\` first."
fi

echo "[local] $(file "$BINARY_PATH" 2>/dev/null || echo "built $BINARY_PATH")"

# ----- upload + install ---------------------------------------------------
REMOTE_INSTALL_DIR=/data/opt/victron-controller
REMOTE_CONFIG_DIR=/data/etc/victron-controller
REMOTE_LOG_DIR=/data/var/log/victron-controller
REMOTE_SERVICE_DIR=/service/victron-controller

CONFIG_TEMPLATE="$REPO_ROOT/config.example.toml"
if [[ ! -f "$CONFIG_TEMPLATE" ]]; then
  echo "ERROR: $CONFIG_TEMPLATE missing" >&2
  exit 4
fi

remote() {
  if [[ $DRY_RUN -eq 1 ]]; then
    printf "[dry-run] ssh %s %q\n" "$TARGET" "$*"
  else
    ssh -T "${SSH_OPTS[@]}" "$TARGET" "$@"
  fi
}

copy() {
  local src="$1" dst="$2"
  if [[ $DRY_RUN -eq 1 ]]; then
    printf "[dry-run] scp %s %s:%s\n" "$src" "$TARGET" "$dst"
  else
    scp "${SSH_OPTS[@]}" "$src" "$TARGET:$dst"
  fi
}

echo "[remote] creating directories..."
remote "mkdir -p $REMOTE_INSTALL_DIR/bin $REMOTE_CONFIG_DIR $REMOTE_LOG_DIR $REMOTE_SERVICE_DIR/log"

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
RUN_LOG_SCRIPT="#!/bin/sh
exec svlogd -tt $REMOTE_LOG_DIR
"

echo "[remote] writing daemontools run scripts..."
remote "cat > $REMOTE_SERVICE_DIR/run.new <<'EOF'
$RUN_SCRIPT
EOF
chmod +x $REMOTE_SERVICE_DIR/run.new && mv $REMOTE_SERVICE_DIR/run.new $REMOTE_SERVICE_DIR/run"

remote "cat > $REMOTE_SERVICE_DIR/log/run.new <<'EOF'
$RUN_LOG_SCRIPT
EOF
chmod +x $REMOTE_SERVICE_DIR/log/run.new && mv $REMOTE_SERVICE_DIR/log/run.new $REMOTE_SERVICE_DIR/log/run"

# rcS.local hook — re-creates /service/victron-controller on boot so the
# install survives Venus firmware upgrades that wipe /service and /opt.
RCS_MARKER="# victron-controller"
RCS_BLOCK="$RCS_MARKER
if [ ! -d $REMOTE_SERVICE_DIR ]; then
  mkdir -p $REMOTE_SERVICE_DIR/log $REMOTE_LOG_DIR
  # On Venus, /service is watched by svscan — creating the dir enables
  # the supervision.
fi
# /end victron-controller"

echo "[remote] installing /data/rcS.local hook (idempotent)..."
remote "touch /data/rcS.local && grep -q '^$RCS_MARKER' /data/rcS.local || cat >> /data/rcS.local <<'EOF'

$RCS_BLOCK
EOF"

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
