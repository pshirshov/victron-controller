#!/usr/bin/env bash
# Grep Victron D-Bus over SSH for register paths matching a pattern.
#
# Use this to find the (service, path, value) triplets you want to pin
# in `dbus_pinned_registers` config — e.g.
#
#   ./scripts/grep-victron-dbus.sh user@victron PowerAssist
#   ./scripts/grep-victron-dbus.sh user@victron AcPowerSetPoint
#   ./scripts/grep-victron-dbus.sh user@victron 'inverter|assist'
#
# Output: matching lines in the form
#
#   <service> <TAB> <path> <TAB> <value>
#
# (case-insensitive regex match against path and value, extended regex).
#
# Progress is streamed to stderr so a slow service does not look like a
# hang. Each service has a 10s `timeout` cap.
set -euo pipefail

if [[ $# -lt 2 ]]; then
  cat >&2 <<EOF
Usage: $0 user@victron-host PATTERN [ssh-opts...]

  PATTERN      extended regex (egrep -E), matched against path and value
               (case-insensitive)

Examples:
  $0 root@venus.local PowerAssist
  $0 root@venus.local 'AcPowerSetPoint|AssistEnabled'
  $0 root@venus.local 'Settings/SystemSetup'
EOF
  exit 2
fi

TARGET="$1"; shift
PATTERN="$1"; shift
SSH_OPTS=("$@")

REMOTE_SCRIPT=$(cat <<'REMOTE'
set -u
LC_ALL=C

PATTERN=$1

log() { echo "[remote] $*" >&2; }

if ! command -v dbus-send >/dev/null 2>&1; then
  echo "ERROR: dbus-send not found on remote" >&2
  exit 3
fi

HAS_DBUS_HELPER=0
command -v dbus >/dev/null 2>&1 && HAS_DBUS_HELPER=1

if command -v timeout >/dev/null 2>&1; then
  TIMEOUT="timeout 10"
else
  TIMEOUT=""
  log "WARNING: no \`timeout\` on remote; hangs in dbus calls won't be capped"
fi

list_services() {
  $TIMEOUT dbus-send --system --print-reply \
    --dest=org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus.ListNames 2>/dev/null </dev/null \
    | grep -oE "com\.victronenergy\.[A-Za-z0-9._-]+" \
    | sort -u
}

dump_items() {
  svc=$1
  if [ "$HAS_DBUS_HELPER" = 1 ]; then
    $TIMEOUT dbus -y "$svc" / GetItems 2>/dev/null </dev/null || true
  else
    $TIMEOUT dbus-send --system --print-reply --dest="$svc" / \
      com.victronenergy.BusItem.GetItems 2>/dev/null </dev/null || true
  fi
}

# Parse `dbus -y` Python pprint output ("/path': {'Value': ..., ...},")
# and dbus-send raw output into "service<TAB>path<TAB>value" lines.
#
# Strategy (handles multi-line wrapped pprint entries):
#   1. Collapse newlines so each path entry is on a single logical line.
#   2. Split before every `'/...':` boundary so each entry starts a line.
#   3. Per line, sed-extract the path and the first 'Value': ... field.
parse_and_emit() {
  svc=$1
  TAB=$(printf "\t")
  tr "\n" " " \
    | sed "s/, *'\//\n'\//g; s/^items = {//; s/^{//" \
    | sed -n "s/^'\([^']*\)':.*'Value':[[:space:]]*\([^,}]*\).*/\1${TAB}\2/p" \
    | sed "s|^|${svc}${TAB}|"
}

log "listing services..."
SERVICES=$(list_services)
if [ -z "$SERVICES" ]; then
  echo "ERROR: no com.victronenergy.* services found on remote" >&2
  exit 4
fi

N=$(printf "%s\n" "$SERVICES" | grep -c .)
log "$N services found; scanning each (10s cap)..."

I=0
for svc in $SERVICES; do
  I=$((I+1))
  log "  [$I/$N] $svc"
  # Stream matches as they are found — no column buffering, line-buffered.
  dump_items "$svc" | parse_and_emit "$svc" | grep -iE "$PATTERN" || true
done

log "done"
REMOTE
)

# `sh -s -- PATTERN` reads the script body from stdin and makes PATTERN
# available as $1 inside the script.
ssh -T "${SSH_OPTS[@]}" "$TARGET" "sh -s -- $(printf "%q" "$PATTERN")" <<<"$REMOTE_SCRIPT"
