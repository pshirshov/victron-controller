#!/usr/bin/env bash
# Read-only broker diagnostic for the victron-controller namespace.
#
# Reads broker host/port/username/password/topic_root from the same
# TOML config file the service uses, then subscribes (read-only) to
# `<topic_root>/#` for a bounded window and reports:
#
#   - total retained topic count under our namespace
#   - breakdown by prefix (knob, bookkeeping, entity, writes_enabled, …)
#   - which retained knob-state topics are for knob names the CURRENT
#     core knows about vs. stale/unknown names (likely from older
#     renames / dev iterations that never got GC'd)
#   - any `/set` topics that carry retained bodies (publishers should
#     never retain /set — this catches broker-side confusion)
#
# ⚠ NEVER publishes anything. ⚠
# The broker has other topics unrelated to victron-controller; this
# script ONLY subscribes under `$topic_root/#` — it does NOT touch
# `#` or any other prefix.
#
# Usage:
#   ./scripts/diagnose-mqtt-retained.sh [--config PATH] [--window SECONDS]
#
# Defaults:
#   --config   /data/etc/victron-controller/config.toml  (on Venus)
#              ./config.toml                             (fallback)
#   --window   5 seconds
#
# Requires: mosquitto_sub (ships with mosquitto-clients).

set -euo pipefail

CONFIG=""
WINDOW=5

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config)  CONFIG="$2"; shift 2 ;;
    --config=*) CONFIG="${1#--config=}"; shift ;;
    --window)  WINDOW="$2"; shift 2 ;;
    --window=*) WINDOW="${1#--window=}"; shift ;;
    -h|--help)
      sed -n '2,32p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# Locate config.
if [[ -z "$CONFIG" ]]; then
  if [[ -f /data/etc/victron-controller/config.toml ]]; then
    CONFIG=/data/etc/victron-controller/config.toml
  elif [[ -f config.toml ]]; then
    CONFIG=config.toml
  else
    echo "no config.toml found; pass --config PATH" >&2
    exit 2
  fi
fi

if ! command -v mosquitto_sub >/dev/null 2>&1; then
  echo "mosquitto_sub not found (install mosquitto-clients)" >&2
  exit 3
fi

# Minimal TOML parser — grep the [mqtt] section's key=value pairs.
# Handles `key = "value"` and `key = 123`. Ignores commented lines.
# Not a general TOML parser; fine for our flat-keyed [mqtt] table.
parse_mqtt_key() {
  local key="$1"
  awk -v key="$key" '
    /^\s*\[/ { section = $0; next }
    section == "[mqtt]" && $0 !~ /^\s*#/ {
      n = split($0, a, "=")
      if (n >= 2) {
        k = a[1]; gsub(/[[:space:]]/, "", k)
        v = a[2]; for (i = 3; i <= n; i++) v = v "=" a[i]
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", v)
        sub(/[[:space:]]*#.*$/, "", v)
        gsub(/^"|"$/, "", v)
        if (k == key) { print v; exit }
      }
    }
  ' "$CONFIG"
}

HOST="$(parse_mqtt_key host)"
PORT="$(parse_mqtt_key port)"
USER="$(parse_mqtt_key username)"
PASS="$(parse_mqtt_key password)"
ROOT="$(parse_mqtt_key topic_root)"

[[ -z "$HOST" ]] && { echo "config: [mqtt] host missing" >&2; exit 4; }
[[ -z "$PORT" ]] && PORT=1883
[[ -z "$ROOT" ]] && { echo "config: [mqtt] topic_root missing" >&2; exit 4; }

echo "== broker =="
echo "host        : $HOST:$PORT"
echo "username    : ${USER:-<anonymous>}"
echo "topic_root  : $ROOT"
echo "subscribe   : ${ROOT}/# (read-only)"
echo "window      : ${WINDOW}s"
echo

# Assemble mosquitto_sub args.
ARGS=(-h "$HOST" -p "$PORT" -t "${ROOT}/#" -v -W "$WINDOW")
if [[ -n "$USER" ]]; then ARGS+=(-u "$USER"); fi
if [[ -n "$PASS" ]]; then ARGS+=(-P "$PASS"); fi

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT

# -W exits with 27 on window-expiry (normal). Ignore that specific code.
set +e
mosquitto_sub "${ARGS[@]}" >"$TMP"
rc=$?
set -e
if [[ $rc -ne 0 && $rc -ne 27 ]]; then
  echo "mosquitto_sub exited $rc; output follows:" >&2
  cat "$TMP" >&2
  exit 5
fi

TOTAL=$(wc -l <"$TMP" | tr -d ' ')
echo "== retained under ${ROOT}/ =="
echo "total messages  : $TOTAL"
echo

if [[ "$TOTAL" -eq 0 ]]; then
  echo "(no retained topics under this root)"
  exit 0
fi

# Breakdown by topic path shape.
echo "== breakdown by topic shape =="
awk '{print $1}' "$TMP" | \
  sed -E "s|^${ROOT}/||" | \
  awk -F/ '
    /^knob\//          { knob[$2]++; knob_total++; next }
    /^bookkeeping\//   { bk_total++; next }
    /^entity\//        { entity_total++; next }
    /^writes_enabled\// { ws_total++; next }
    /^log\//           { log_total++; next }
    { other[$0]++; other_total++ }
    END {
      printf "  knob/*             %d (%d distinct names)\n", knob_total, length(knob)
      printf "  bookkeeping/*      %d\n", bk_total
      printf "  entity/*           %d\n", entity_total
      printf "  writes_enabled/*   %d\n", ws_total
      printf "  log/*              %d\n", log_total
      printf "  other              %d\n", other_total
      if (other_total > 0) {
        print "  other paths:"
        for (k in other) printf "    %s (%d)\n", k, other[k]
      }
    }
  '
echo

# Known knob names (match KnobId::* in crates/core/src/types.rs and the
# knob_name table in crates/shell/src/mqtt/serialize.rs). Keep in sync
# when new knobs land.
KNOWN_KNOBS=(
  force_disable_export
  export_soc_threshold
  discharge_soc_target
  battery_soc_target
  full_charge_discharge_soc_target
  full_charge_export_soc_threshold
  discharge_time
  debug_full_charge
  pessimism_multiplier_modifier
  disable_night_grid_discharge
  charge_car_boost
  charge_car_extended
  zappi_current_target
  zappi_limit
  zappi_emergency_margin
  grid_export_limit_w
  grid_import_limit_w
  allow_battery_to_car
  eddi_enable_soc
  eddi_disable_soc
  eddi_dwell_s
  weathersoc_winter_temperature_threshold
  weathersoc_low_energy_threshold
  weathersoc_ok_energy_threshold
  weathersoc_high_energy_threshold
  weathersoc_too_much_energy_threshold
  forecast_disagreement_strategy
  charge_battery_extended_mode
)

KNOWN_FILE="$(mktemp)"
trap 'rm -f "$TMP" "$KNOWN_FILE"' EXIT
printf '%s\n' "${KNOWN_KNOBS[@]}" | sort -u >"$KNOWN_FILE"

# Extract distinct knob names seen on the broker.
SEEN_FILE="$(mktemp)"
awk '{print $1}' "$TMP" | \
  sed -nE "s|^${ROOT}/knob/([^/]+)/state$|\1|p" | \
  sort -u >"$SEEN_FILE"

echo "== retained knob names vs current schema =="
echo "distinct retained knobs: $(wc -l <"$SEEN_FILE" | tr -d ' ')"
echo "known knobs in schema  : $(wc -l <"$KNOWN_FILE" | tr -d ' ')"
echo

# busybox on Venus lacks `comm`; use awk for set arithmetic.
# UNKNOWN = SEEN \ KNOWN (retained but not in current schema)
UNKNOWN="$(awk 'NR==FNR{k[$0]; next} !($0 in k)' "$KNOWN_FILE" "$SEEN_FILE")"
# MISSING = KNOWN \ SEEN (schema-known but no retained state)
MISSING="$(awk 'NR==FNR{s[$0]; next} !($0 in s)' "$SEEN_FILE" "$KNOWN_FILE")"

if [[ -n "$UNKNOWN" ]]; then
  echo "-- retained knob names NOT recognised by current code --"
  echo "(these are stale retained topics from old/renamed knobs; they"
  echo " will parse to None at bootstrap and not affect control, BUT"
  echo " they clutter the broker and MAY be counted in the applied=N"
  echo " startup log.)"
  echo "$UNKNOWN" | sed 's/^/  /'
  echo
  echo "To clear a stale retained topic (only if you're sure it's stale):"
  # shellcheck disable=SC2016
  echo '  mosquitto_pub -h "$HOST" -u "$USER" -P "$PASS" -t "'"$ROOT"'/knob/<NAME>/state" -r -n'
  echo "  (NOTE: this writes an empty retained payload to clear. This"
  echo "   script itself never publishes — run the command manually if"
  echo "   you've confirmed the topic is stale.)"
  echo
fi

if [[ -n "$MISSING" ]]; then
  echo "-- known knobs WITHOUT retained state on the broker --"
  echo "(these will use cold-start defaults on next service restart; OK"
  echo " for a fresh-ish install, suspicious for a long-running one.)"
  echo "$MISSING" | sed 's/^/  /'
  echo
fi

# Check /set topics with retained payload — should never exist.
SET_RETAINED="$(awk '{print $1}' "$TMP" | grep -E "^${ROOT}/(knob/.+/set|writes_enabled/set)$" || true)"
if [[ -n "$SET_RETAINED" ]]; then
  echo "-- WARNING: retained bodies on /set topics (should never happen) --"
  echo "$SET_RETAINED" | sed 's/^/  /'
  echo "  These suggest a client is publishing to /set with retain=true."
  echo "  HA automations, scripts, or external tools are the usual cause."
  echo "  Clear them ONLY if you've audited the publishers:"
  # shellcheck disable=SC2016
  echo '    mosquitto_pub -h "$HOST" ... -t "<topic>" -r -n'
  echo
fi

echo "== first 20 retained messages (topic  payload) =="
head -20 "$TMP" | awk '{
  topic=$1; $1="";
  payload=$0; sub(/^ /, "", payload);
  if (length(payload) > 80) payload = substr(payload, 1, 80) "…";
  printf "%s  %s\n", topic, payload
}'
if [[ "$TOTAL" -gt 20 ]]; then
  echo "(... $((TOTAL - 20)) more; full dump: mosquitto_sub -h $HOST ... -t '$ROOT/#' -v -W $WINDOW)"
fi
