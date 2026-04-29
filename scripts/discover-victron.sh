#!/usr/bin/env bash
# Enumerate all com.victronenergy.* D-Bus services on a Venus OS host
# and dump each service's full item tree.
#
# Usage:  ./scripts/discover-victron.sh user@victron-host [ssh-opts...]
#
# Output: legacy/discovery-<UTC-timestamp>/
#           dump.txt                 — human-readable combined report
#           services.txt             — bus-name list, one per line
#           per-service/*.items.txt  — GetItems dump per service
#
# Runs one remote batch over a single SSH connection. The remote side is
# executed via `sh -c` (passed as an argv string, not piped through
# stdin) so it works on stock Venus OS (BusyBox ash) and avoids stdin
# buffering issues when the remote spawns dbus-send subprocesses.
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 user@victron-host [ssh-opts...]" >&2
  exit 2
fi

TARGET="$1"; shift
SSH_OPTS=("$@")

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUTDIR="$REPO_ROOT/legacy/discovery-$STAMP"
mkdir -p "$OUTDIR/per-service"

echo "target:  $TARGET"
echo "outdir:  $OUTDIR"
echo

# The remote batch. Single-quoted string: nothing here is expanded
# locally. Uses POSIX sh (no bashisms) — Venus OS has BusyBox ash as
# /bin/sh, not bash.
#
# shellcheck disable=SC2016  # intentional: this literal runs on the remote side
REMOTE_SCRIPT='
set -u
LC_ALL=C

log() { echo "[remote] $*" >&2; }

log "starting discovery on $(hostname 2>/dev/null || echo ?)"

if ! command -v dbus-send >/dev/null 2>&1; then
  echo "ERROR: dbus-send not found on remote" >&2
  exit 3
fi

HAS_DBUS_HELPER=0
command -v dbus >/dev/null 2>&1 && HAS_DBUS_HELPER=1
log "dbus helper present: $HAS_DBUS_HELPER"

# Wrap dbus calls in a short timeout so a hung daemon does not block the
# whole run. BusyBox usually ships `timeout`, but not always on Venus.
if command -v timeout >/dev/null 2>&1; then
  TIMEOUT="timeout 10"
  log "using timeout wrapper"
else
  TIMEOUT=""
  log "no timeout command available; dbus calls will not be time-limited"
fi

get_value() {
  svc=$1; path=$2
  if [ "$HAS_DBUS_HELPER" = 1 ]; then
    $TIMEOUT dbus -y "$svc" "$path" GetValue 2>/dev/null </dev/null || true
  else
    $TIMEOUT dbus-send --system --print-reply=literal --dest="$svc" "$path" \
      com.victronenergy.BusItem.GetValue 2>/dev/null </dev/null || true
  fi
}

dump_items() {
  svc=$1
  if [ "$HAS_DBUS_HELPER" = 1 ]; then
    $TIMEOUT dbus -y "$svc" / GetItems 2>&1 </dev/null || echo "(GetItems failed)"
  else
    $TIMEOUT dbus-send --system --print-reply --dest="$svc" / \
      com.victronenergy.BusItem.GetItems 2>&1 </dev/null || echo "(GetItems failed)"
  fi
}

echo "=== HEADER ==="
echo "host:           $(hostname 2>/dev/null || echo unknown)"
echo "date_utc:       $(date -u +%FT%TZ)"
echo "venus_version:  $(cat /opt/victronenergy/version 2>/dev/null || echo unknown)"
echo "kernel:         $(uname -a 2>/dev/null || echo unknown)"
echo "dbus_helper:    $HAS_DBUS_HELPER"
echo

log "listing services..."
echo "=== SERVICES ==="

# Raw ListNames output — include verbatim in dump.txt so we can debug
# parsing issues when the grep below returns nothing.
RAW_LIST=$(
  $TIMEOUT dbus-send --system --print-reply \
    --dest=org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus.ListNames 2>&1 </dev/null \
  || echo "(ListNames failed)"
)

# Extract Victron service names from wherever they appear in the output.
# Matches both `com.victronenergy.battery` and `com.victronenergy.battery.ttyUSB0`.
SERVICES=$(printf "%s\n" "$RAW_LIST" \
  | grep -oE "com\.victronenergy\.[A-Za-z0-9._-]+" \
  | sort -u)

echo "$SERVICES"
echo

N=$(printf "%s\n" "$SERVICES" | grep -c .)
if [ "$N" = 0 ]; then
  echo "=== RAW LISTNAMES (no Victron services parsed — debug dump follows) ==="
  printf "%s\n" "$RAW_LIST"
  echo "=== /RAW LISTNAMES ==="
  log "no services matched; raw ListNames captured in dump.txt for inspection"
fi
log "dumping $N services..."
I=0
for svc in $SERVICES; do
  I=$((I+1))
  log "  [$I/$N] $svc"
  echo "=== BEGIN $svc ==="
  echo "DeviceInstance:  $(get_value "$svc" /DeviceInstance)"
  echo "ProductName:     $(get_value "$svc" /ProductName)"
  echo "Mgmt.Connection: $(get_value "$svc" /Mgmt/Connection)"
  echo "Position:        $(get_value "$svc" /Position)"
  echo "--- items ---"
  dump_items "$svc"
  echo "=== END $svc ==="
  echo
done

# Focused MPPT operation-mode probe. The full GetItems dump above already
# contains every path, but Victron firmware varies in where the MPP-state
# field lives (and what its enum values are). This section probes the
# common candidate paths explicitly per solarcharger service so we can
# identify the right SensorId path without grepping through full dumps.
echo "=== MPPT OPERATION-MODE PROBE ==="
SOLAR_SERVICES=$(printf "%s\n" "$SERVICES" | grep -E "^com\.victronenergy\.solarcharger" || true)
if [ -z "$SOLAR_SERVICES" ]; then
  echo "(no com.victronenergy.solarcharger.* services found)"
else
  for svc in $SOLAR_SERVICES; do
    echo "--- $svc ---"
    echo "DeviceInstance:  $(get_value "$svc" /DeviceInstance)"
    echo "ProductName:     $(get_value "$svc" /ProductName)"
    # Candidate paths across firmware revisions. Most paths return "" if
    # absent; we print the path : value pair regardless so empty rows
    # confirm the path was probed.
    for path in \
      /MppOperationMode \
      /Mpp/Operation/Mode \
      /State \
      /ErrorCode \
      /Yield/Power \
      /Pv/V \
      /Pv/I \
      /Dc/0/Voltage \
      /Dc/0/Current \
      /Settings/ChargerMode \
      /Settings/BmsPresent \
      /Mode
    do
      echo "  $path = $(get_value "$svc" "$path")"
    done
    echo
  done
fi
echo "=== /MPPT OPERATION-MODE PROBE ==="
echo

log "done"
'

# Pass the whole script as a single argv to ssh; sshd invokes it via
# /bin/sh -c. `-T` disables pseudo-terminal allocation. No stdin is
# passed, so nothing on the remote can block waiting for input.
echo "[local] running ssh; remote progress on stderr below:"
ssh -T "${SSH_OPTS[@]}" "$TARGET" "$REMOTE_SCRIPT" > "$OUTDIR/dump.txt"

# Fan out: write services.txt and per-service/*.items.txt from dump.txt.
# Pre-create services.txt so downstream reporting works even if the
# section is empty.
: > "$OUTDIR/services.txt"
awk '
  $0 == "=== SERVICES ===" { in_services = 1; next }
  in_services && /^=== / { in_services = 0 }
  in_services && NF > 0 { print > OUTFILE }
' OUTFILE="$OUTDIR/services.txt" "$OUTDIR/dump.txt"

awk -v outdir="$OUTDIR/per-service" '
  /^=== BEGIN com\.victronenergy\./ {
    svc = $3
    safe = svc; gsub("/", "_", safe); gsub("\\.", "_", safe)
    outfile = outdir "/" safe ".items.txt"
    capturing = 1
    next
  }
  /^=== END com\.victronenergy\./ {
    capturing = 0
    if (outfile != "") close(outfile)
    outfile = ""
    next
  }
  capturing && outfile != "" { print >> outfile }
' "$OUTDIR/dump.txt"

echo
echo "[local] done."
echo "  $OUTDIR/dump.txt"
echo "  $OUTDIR/services.txt ($(wc -l < "$OUTDIR/services.txt" | tr -d ' ') services)"
echo "  $OUTDIR/per-service/*.items.txt ($(find "$OUTDIR/per-service" -maxdepth 1 -name '*.items.txt' 2>/dev/null | wc -l | tr -d ' ') files)"
