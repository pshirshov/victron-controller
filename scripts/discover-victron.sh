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
# Runs one remote batch over a single SSH connection.
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

# The remote batch: single shell on Venus that dumps everything to stdout.
# We split sections by known markers so the local side can fan out.
# shellcheck disable=SC2016   # intentional: this literal runs on the remote side
REMOTE_SCRIPT='
set -u
LC_ALL=C

# which dbus helper?
if command -v dbus >/dev/null 2>&1; then
  DBUS="dbus -y"
elif command -v dbus-send >/dev/null 2>&1; then
  DBUS="fallback"
else
  echo "ERROR: no dbus CLI available on the host" >&2
  exit 3
fi

get_value() {
  svc="$1"; path="$2"
  if [ "$DBUS" = "fallback" ]; then
    dbus-send --system --print-reply=literal --dest="$svc" "$path" \
      com.victronenergy.BusItem.GetValue 2>/dev/null \
      | awk "{for(i=1;i<=NF;i++)if(\$i~/variant/){for(j=i+1;j<=NF;j++)printf \"%s \",\$j;print \"\"}}" \
      | sed "s/[[:space:]]*$//"
  else
    dbus -y "$svc" "$path" GetValue 2>/dev/null || true
  fi
}

dump_items() {
  svc="$1"
  if [ "$DBUS" = "fallback" ]; then
    dbus-send --system --print-reply --dest="$svc" / com.victronenergy.BusItem.GetItems 2>&1
  else
    dbus -y "$svc" / GetItems 2>&1
  fi
}

echo "=== HEADER ==="
echo "host:           $(hostname 2>/dev/null || echo unknown)"
echo "date_utc:       $(date -u +%FT%TZ)"
echo "venus_version:  $(cat /opt/victronenergy/version 2>/dev/null || echo unknown)"
echo "kernel:         $(uname -a 2>/dev/null || echo unknown)"
echo "dbus_tool:      $DBUS"
echo

echo "=== SERVICES ==="
SERVICES="$(
  dbus-send --system --print-reply=literal \
    --dest=org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus.ListNames 2>/dev/null \
  | tr " " "\n" \
  | grep -E "^com\.victronenergy\." \
  | sort -u
)"
echo "$SERVICES"
echo

for svc in $SERVICES; do
  inst="$(get_value "$svc" /DeviceInstance)"
  prod="$(get_value "$svc" /ProductName)"
  mgmt="$(get_value "$svc" /Mgmt/Connection)"
  pos="$(get_value "$svc" /Position)"
  echo "=== BEGIN $svc ==="
  echo "DeviceInstance:  $inst"
  echo "ProductName:     $prod"
  echo "Mgmt.Connection: $mgmt"
  echo "Position:        $pos"
  echo "--- items ---"
  dump_items "$svc"
  echo "=== END $svc ==="
  echo
done
'

# shellcheck disable=SC2029
ssh "${SSH_OPTS[@]}" "$TARGET" bash -s > "$OUTDIR/dump.txt" <<< "$REMOTE_SCRIPT"

# Fan out: write services.txt and per-service/*.items.txt
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

echo "wrote:"
echo "  $OUTDIR/dump.txt"
echo "  $OUTDIR/services.txt ($(wc -l < "$OUTDIR/services.txt" | tr -d ' ') services)"
echo "  $OUTDIR/per-service/*.items.txt ($(find "$OUTDIR/per-service" -maxdepth 1 -name '*.items.txt' 2>/dev/null | wc -l | tr -d ' ') files)"
