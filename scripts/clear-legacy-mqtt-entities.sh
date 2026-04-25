#!/usr/bin/env bash
# Clear legacy retained MQTT entities orphaned by PR-rename-entities.
#
# 0.2.0 entity names were snake_case; 0.3.0 uses a dotted hierarchical
# convention (e.g. `battery_soc` → `battery.soc`). The wire-format bump
# leaves the OLD retained topics on the broker — both the controller's
# own state topics and the HA discovery configs HA built against them.
#
# This script publishes empty retained payloads ("delete retained
# message" semantics in MQTT) to every legacy topic so:
#   - HA stops rendering ghost entities after a discovery refresh;
#   - the next bootstrap of the controller doesn't seed itself from a
#     retained message that no longer maps to a known knob.
#
# It is idempotent: re-running it after a successful clear is a no-op
# (already-empty retained slots stay empty).
#
# Configuration via env vars (with defaults appropriate to the
# Victron-on-LAN setup):
#   BROKER_HOST  default: 127.0.0.1
#   BROKER_PORT  default: 1883
#   BROKER_USER  default: (unset, anonymous)
#   BROKER_PASS  default: (unset, anonymous)
#   TOPIC_ROOT   default: victron_controller
#   HA_ROOT      default: homeassistant
#   NODE_ID      default: victron_controller   (HA discovery node_id)
#
# Requires: mosquitto_pub from the mosquitto-clients package.
set -eu

BROKER_HOST="${BROKER_HOST:-127.0.0.1}"
BROKER_PORT="${BROKER_PORT:-1883}"
TOPIC_ROOT="${TOPIC_ROOT:-victron_controller}"
HA_ROOT="${HA_ROOT:-homeassistant}"
NODE_ID="${NODE_ID:-victron_controller}"

if ! command -v mosquitto_pub >/dev/null 2>&1; then
  echo "error: mosquitto_pub not found in PATH (apt install mosquitto-clients)" >&2
  exit 1
fi

# Build the mosquitto_pub command-line auth fragment once.
AUTH_ARGS=()
if [ -n "${BROKER_USER:-}" ]; then
  AUTH_ARGS+=(-u "$BROKER_USER")
fi
if [ -n "${BROKER_PASS:-}" ]; then
  AUTH_ARGS+=(-P "$BROKER_PASS")
fi

# Publish an empty retained payload to a topic. Empty retain = delete.
clear_topic() {
  local topic="$1"
  mosquitto_pub \
    -h "$BROKER_HOST" \
    -p "$BROKER_PORT" \
    "${AUTH_ARGS[@]}" \
    -r \
    -n \
    -t "$topic"
  echo "cleared: $topic"
}

# ---------------------------------------------------------------------------
# Legacy 0.2.0 entity names. KEEP THIS LIST IN SYNC WITH THE INVENTORY
# in PR-rename-entities — these are the names that no longer exist in
# 0.3.0 and whose retained slots therefore need explicit deletion.
# ---------------------------------------------------------------------------

LEGACY_KNOBS=(
  force_disable_export
  export_soc_threshold
  export_soc_threshold_mode
  discharge_soc_target
  discharge_soc_target_mode
  battery_soc_target
  battery_soc_target_mode
  disable_night_grid_discharge
  disable_night_grid_discharge_mode
  full_charge_discharge_soc_target
  full_charge_export_soc_threshold
  discharge_time
  debug_full_charge
  pessimism_multiplier_modifier
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
  # writes_enabled is intentionally NOT cleared — its name stays
  # `writes_enabled` (master kill switch, dashboard convention).
)

LEGACY_SENSORS=(
  battery_soc
  battery_soh
  battery_installed_capacity
  battery_dc_power
  mppt_power_0
  mppt_power_1
  soltaro_power
  power_consumption
  consumption_current
  grid_power
  grid_voltage
  grid_current
  offgrid_power
  offgrid_current
  vebus_input_current
  evcharger_ac_power
  evcharger_ac_current
  ess_state
  outdoor_temperature
  session_kwh
)

LEGACY_BOOKKEEPING_NUMERIC=(
  soc_end_of_day_target
  effective_export_soc_threshold
  battery_selected_soc_target
  prev_ess_state
)

LEGACY_BOOKKEEPING_BOOL=(
  zappi_active
  charge_to_full_required
  charge_battery_extended_today
)

# Persistence-side bookkeeping retained topics (the bootstrap-restore
# slice — `BookkeepingKey` in core/src/types.rs).
LEGACY_BOOKKEEPING_PERSIST=(
  next_full_charge
  above_soc_date
  prev_ess_state
)

LEGACY_ACTUATED=(
  grid_setpoint
  input_current_limit
  zappi_mode
  eddi_mode
  schedule_0
  schedule_1
)

# ---------------------------------------------------------------------------
# 1. Controller-owned state + command topics.
# ---------------------------------------------------------------------------
echo "==> clearing legacy controller-owned state under $TOPIC_ROOT/"

for k in "${LEGACY_KNOBS[@]}"; do
  clear_topic "$TOPIC_ROOT/knob/$k/state"
  clear_topic "$TOPIC_ROOT/knob/$k/set"
done

for s in "${LEGACY_SENSORS[@]}"; do
  clear_topic "$TOPIC_ROOT/sensor/$s/state"
done

for b in "${LEGACY_BOOKKEEPING_NUMERIC[@]}" "${LEGACY_BOOKKEEPING_BOOL[@]}" "${LEGACY_BOOKKEEPING_PERSIST[@]}"; do
  clear_topic "$TOPIC_ROOT/bookkeeping/$b/state"
done

for a in "${LEGACY_ACTUATED[@]}"; do
  clear_topic "$TOPIC_ROOT/entity/$a/phase"
done

# writes_enabled topic name does NOT change in 0.3.0; only clear it
# explicitly if the operator wants a fully clean slate (commented out
# by default).
# clear_topic "$TOPIC_ROOT/writes_enabled/state"
# clear_topic "$TOPIC_ROOT/writes_enabled/set"

# ---------------------------------------------------------------------------
# 2. Home Assistant MQTT-discovery config topics.
#
# 0.2.0 schema (from crates/shell/src/mqtt/discovery.rs as it stood
# pre-rename):
#   homeassistant/{switch|number|select}/victron_controller/knob_<name>/config
#   homeassistant/switch/victron_controller/writes_enabled/config
#   homeassistant/sensor/victron_controller/phase_<name>/config
#   homeassistant/sensor/victron_controller/sensor_<name>/config
#   homeassistant/sensor/victron_controller/bookkeeping_<name>/config
#   homeassistant/binary_sensor/victron_controller/bookkeeping_<name>/config
#
# We don't know the exact component (switch/number/select) for every
# knob without regenerating the schema table here, so we publish empty
# retained payloads to every plausible component path. HA ignores
# attempts to clear a topic that was never set; an extra clear is a
# no-op on the broker.
# ---------------------------------------------------------------------------
echo "==> clearing legacy HA discovery configs under $HA_ROOT/"

KNOB_COMPONENTS=(switch number select)

for k in "${LEGACY_KNOBS[@]}"; do
  for c in "${KNOB_COMPONENTS[@]}"; do
    clear_topic "$HA_ROOT/$c/$NODE_ID/knob_$k/config"
  done
done

# Kill switch lived on the same legacy path; the topic name doesn't
# change in 0.3.0 but the HA discovery config IS regenerated by the
# new code under a different unique_id, so old configs are still safe
# to clear.
clear_topic "$HA_ROOT/switch/$NODE_ID/writes_enabled/config"

for a in "${LEGACY_ACTUATED[@]}"; do
  clear_topic "$HA_ROOT/sensor/$NODE_ID/phase_$a/config"
done

for s in "${LEGACY_SENSORS[@]}"; do
  clear_topic "$HA_ROOT/sensor/$NODE_ID/sensor_$s/config"
done

for b in "${LEGACY_BOOKKEEPING_NUMERIC[@]}"; do
  clear_topic "$HA_ROOT/sensor/$NODE_ID/bookkeeping_$b/config"
done

for b in "${LEGACY_BOOKKEEPING_BOOL[@]}"; do
  clear_topic "$HA_ROOT/binary_sensor/$NODE_ID/bookkeeping_$b/config"
done

echo "==> done"
