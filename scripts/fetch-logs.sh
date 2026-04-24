#!/usr/bin/env bash
# Fetch a diagnostic bundle from the Venus and write it to a timestamped
# file under /tmp/exchange/ on the local machine.
#
# Usage:
#   ./scripts/fetch-logs.sh user@venus-host [--lines N] [ssh-opts...]
#
# What it collects (in order):
#   - `uname -a`, free, df for /data
#   - svstat for the service + its logger
#   - the daemontools run scripts (to confirm what supervision is running)
#   - the last N lines of the log (default 500), with TAI64N converted to
#     human time if tai64nlocal is available on the Venus
#   - the active config.toml with secrets redacted (mqtt.password,
#     myenergi.password, and any *api_key* keys zeroed out)
#
# Output: /tmp/exchange/victron-bundle-YYYYmmdd-HHMMSS.txt
# A symlink /tmp/exchange/victron-bundle-latest.txt is updated to point
# at the newest bundle so tailing the same path works across runs.

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 user@venus-host [--lines N] [ssh-opts...]" >&2
  exit 2
fi

TARGET="$1"; shift
LINES=500
SSH_OPTS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --lines)
      LINES="$2"; shift 2 ;;
    --lines=*)
      LINES="${1#--lines=}"; shift ;;
    *)
      SSH_OPTS+=("$1"); shift ;;
  esac
done

REMOTE_SERVICE_DIR=/service/victron-controller
# Logs live on tmpfs (/var/volatile is the actual tmpfs mount on Venus).
# Volatile on reboot — the MQTT archive is the durable log sink.
REMOTE_LOG_DIR=/var/volatile/log/victron-controller
REMOTE_CONFIG=/data/etc/victron-controller/config.toml

OUT_DIR=/tmp/exchange
mkdir -p "$OUT_DIR"
STAMP="$(date +%Y%m%d-%H%M%S)"
OUT_FILE="$OUT_DIR/victron-bundle-$STAMP.txt"
LATEST="$OUT_DIR/victron-bundle-latest.txt"

echo "[local] fetching from $TARGET → $OUT_FILE" >&2

# Safety options: fail fast on unreachable host and keep the session
# alive over idle NAT. No `-T` — it's redundant when passing a command
# and has been observed to hang on some Venus ssh setups.
SSH_COMMON_OPTS=(-o ServerAliveInterval=30 -o ServerAliveCountMax=3 -o ConnectTimeout=15)

# The bash heredoc runs remotely. LINES is expanded locally; everything
# else is escaped so the remote shell evaluates it. stdin is the heredoc,
# fed to `bash -s` on the far side.
ssh "${SSH_COMMON_OPTS[@]}" "${SSH_OPTS[@]}" "$TARGET" bash -s > "$OUT_FILE" <<REMOTE_SCRIPT
set -u
LINES=${LINES}
REMOTE_SERVICE_DIR=${REMOTE_SERVICE_DIR}
REMOTE_LOG_DIR=${REMOTE_LOG_DIR}
REMOTE_CONFIG=${REMOTE_CONFIG}

section() {
  echo
  echo "===== \$1 ====="
}

section "host"
uname -a 2>&1 || true
echo
echo "-- uptime --"
uptime 2>&1 || true
echo
echo "-- /data disk --"
df -h /data 2>&1 || true
echo
echo "-- memory --"
free -h 2>&1 || free 2>&1 || true

section "service supervision"
svstat \$REMOTE_SERVICE_DIR \$REMOTE_SERVICE_DIR/log 2>&1 || true
echo
echo "-- run --"
cat \$REMOTE_SERVICE_DIR/run 2>&1 || true
echo
echo "-- log/run --"
cat \$REMOTE_SERVICE_DIR/log/run 2>&1 || true

section "process"
MAIN_PID=\$(svstat \$REMOTE_SERVICE_DIR 2>/dev/null | awk '{for(i=1;i<=NF;i++) if(\$i=="(pid") print \$(i+1)}' | tr -d ')')
if [ -n "\$MAIN_PID" ] && [ -d /proc/\$MAIN_PID ]; then
  echo "-- pid \$MAIN_PID --"
  cat /proc/\$MAIN_PID/status 2>&1 | head -20 || true
  echo
  echo "-- fds (interested in stdout/stderr = 1/2) --"
  ls -la /proc/\$MAIN_PID/fd/0 /proc/\$MAIN_PID/fd/1 /proc/\$MAIN_PID/fd/2 2>&1 || true
else
  echo "(main service not running)"
fi

section "log directory"
ls -la \$REMOTE_LOG_DIR 2>&1 || true

section "last \$LINES log lines"
if [ -f \$REMOTE_LOG_DIR/current ]; then
  if command -v tai64nlocal >/dev/null 2>&1; then
    tail -n \$LINES \$REMOTE_LOG_DIR/current | tai64nlocal
  else
    tail -n \$LINES \$REMOTE_LOG_DIR/current
  fi
else
  echo "(no log file yet at \$REMOTE_LOG_DIR/current)"
fi

section "config.toml (secrets redacted)"
if [ -f \$REMOTE_CONFIG ]; then
  sed -E \\
    -e 's/^([[:space:]]*password[[:space:]]*=[[:space:]]*).*/\1"<REDACTED>"/' \\
    -e 's/^([[:space:]]*api_key[[:space:]]*=[[:space:]]*).*/\1"<REDACTED>"/' \\
    \$REMOTE_CONFIG
else
  echo "(no config.toml at \$REMOTE_CONFIG)"
fi

section "binary info"
BIN=/data/opt/victron-controller/bin/victron-controller
if [ -f \$BIN ]; then
  ls -la \$BIN
  file \$BIN 2>&1 | head -3 || true
else
  echo "(no binary at \$BIN)"
fi

section "recent dmesg (last 40 lines)"
dmesg 2>&1 | tail -n 40 || true

echo
echo "===== end of bundle ====="
REMOTE_SCRIPT

ln -sf "$(basename "$OUT_FILE")" "$LATEST"
echo "[local] wrote $(wc -l <"$OUT_FILE") lines, $(wc -c <"$OUT_FILE") bytes" >&2
echo "[local] latest symlink: $LATEST → $(readlink "$LATEST")" >&2
echo "$OUT_FILE"
