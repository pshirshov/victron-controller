//! Home Assistant MQTT-discovery publishing. One-shot at startup.
//!
//! We advertise:
//! - **switch** entities for the bool knobs (`force_disable_export`,
//!   `allow_battery_to_car`, `disable_night_grid_discharge`,
//!   `charge_car_boost`, `charge_car_extended`, `writes_enabled` kill).
//! - **number** entities for the float / uint knobs
//!   (`export_soc_threshold`, `discharge_soc_target`, …).
//! - **select** entities for the enum knobs (`discharge_time`,
//!   `debug_full_charge`, `forecast_disagreement_strategy`).
//! - **sensor** entities for each actuated entity's phase.
//! - **PR-ha-discovery-expand:** `sensor` entities for every scalar
//!   `SensorId` (D-Bus, outdoor_temperature, session_kwh) and for the
//!   numeric controller-relevant bookkeeping fields; `binary_sensor`
//!   entities for the boolean bookkeeping fields.
//!
//! ## Topic taxonomy
//!
//! Every retained topic this module owns. State topics are written
//! per-tick by `SensorBroadcastCore` (via `serialize::encode_publish_payload`);
//! discovery topics are written once at startup by the functions below.
//!
//! ### Knob entities (existing — unchanged)
//! - Discovery: `homeassistant/{switch,number,select}/victron_controller/knob_<name>/config`
//! - State:     `<topic_root>/knob/<name>/state`            (retained)
//! - Command:   `<topic_root>/knob/<name>/set`              (live)
//!
//! ### Kill switch (existing — unchanged)
//! - Discovery: `homeassistant/switch/victron_controller/writes_enabled/config`
//! - State:     `<topic_root>/writes_enabled/state`         (retained)
//! - Command:   `<topic_root>/writes_enabled/set`           (live)
//!
//! ### Actuated phases (existing — unchanged)
//! - Discovery: `homeassistant/sensor/victron_controller/phase_<name>/config`
//! - State:     `<topic_root>/entity/<name>/phase`          (retained, JSON)
//!
//! ### NEW — Scalar sensors (20 ids)
//! - Discovery: `homeassistant/sensor/victron_controller/sensor_<name>/config`
//! - State:     `<topic_root>/sensor/<name>/state`          (retained,
//!   numeric string OR `"unavailable"` when Stale/Unknown — HA convention)
//!
//!   `<name>` ∈ { battery_soc, battery_soh, battery_installed_capacity,
//!   battery_dc_power, mppt_power_0, mppt_power_1, soltaro_power,
//!   power_consumption, grid_power, grid_voltage, grid_current,
//!   consumption_current, offgrid_power, offgrid_current,
//!   vebus_input_current, evcharger_ac_power, evcharger_ac_current,
//!   ess_state, outdoor_temperature, session_kwh }
//!
//! ### NEW — Numeric bookkeeping (3 ids)
//! - Discovery: `homeassistant/sensor/victron_controller/bookkeeping_<name>/config`
//! - State:     `<topic_root>/bookkeeping/<name>/state`     (retained, numeric string)
//!
//!   `<name>` ∈ { soc_end_of_day_target, effective_export_soc_threshold,
//!   battery_selected_soc_target }
//!
//! ### NEW — Boolean bookkeeping (3 ids)
//! - Discovery: `homeassistant/binary_sensor/victron_controller/bookkeeping_<name>/config`
//! - State:     `<topic_root>/bookkeeping/<name>/state`     (retained, `"true"`/`"false"`)
//!
//!   `<name>` ∈ { zappi_active, charge_to_full_required, charge_battery_extended_today }
//!
//! Volume: 27 new discovery topics (~10 KB retained at ~400 B JSON),
//! 27 new state topics (publish-on-change, dedup'd in
//! `SensorBroadcastCore` via `world.published_cache`).
//!
//! The HA discovery spec requires per-entity config payloads on a
//! well-known topic path. See
//! <https://www.home-assistant.io/integrations/mqtt/#mqtt-discovery>.

use anyhow::Result;
use rumqttc::{AsyncClient, QoS};
use serde_json::json;
use tracing::{debug, info};

use victron_controller_core::types::{ActuatedId, BookkeepingId, KnobId, SensorId};

use super::serialize::{knob_name, knob_range, sensor_name};

/// Build an HA discovery schema for a `number`-component knob that's
/// validated by `parse_knob_value`. Reuses `knob_range` as the
/// authoritative min/max so discovery and ingest validation can't
/// drift.
fn number_knob(
    id: KnobId,
    step: f64,
    unit: Option<&'static str>,
) -> (KnobId, &'static str, serde_json::Value) {
    let (min, max) = knob_range(id).unwrap_or_else(|| {
        panic!("knob_range missing for {id:?} — extend knob_range in serialize.rs first")
    });
    let mut extra = json!({ "min": min, "max": max, "step": step });
    if let Some(u) = unit {
        extra
            .as_object_mut()
            .expect("json object")
            .insert("unit_of_measurement".to_string(), json!(u));
    }
    (id, "number", extra)
}

const HA_ROOT: &str = "homeassistant";
const NODE_ID: &str = "victron_controller";

/// HA's MQTT discovery spec restricts `unique_id` and the topic-tail
/// `<object_id>` segment to `[a-zA-Z0-9_-]+`. Dots silently make HA
/// reject the entire entity. Our user-visible / MQTT-state-topic
/// names use the dotted form (e.g. `battery.soc.threshold.export.
/// forced-value`), so for HA-bound strings we substitute `.` → `_`.
/// State topics (which the controller publishes to and HA reads from)
/// are NOT HA-restricted — those keep the dotted form.
fn ha_safe(name: &str) -> String {
    name.replace('.', "_")
}

fn device_block() -> serde_json::Value {
    json!({
        "identifiers": ["victron_controller"],
        "name": "Victron Controller",
        "manufacturer": "7mind.io",
        "model": "victron-controller-rust",
    })
}

/// Publish HA discovery config payloads. Retained so HA sees them even
/// if it comes online after the service.
pub async fn publish_ha_discovery(client: &AsyncClient, topic_root: &str) -> Result<()> {
    let total = publish_knobs(client, topic_root).await?
        + publish_kill_switch(client, topic_root).await?
        + publish_phases(client, topic_root).await?
        + publish_sensors(client, topic_root).await?
        + publish_bookkeeping(client, topic_root).await?;
    info!(count = total, "HA discovery published");
    Ok(())
}

async fn publish_knobs(client: &AsyncClient, topic_root: &str) -> Result<usize> {
    let mut count = 0;
    for (id, component, extra) in knob_schemas() {
        let name = knob_name(id);
        let ha_name = ha_safe(name);
        let state_topic = format!("{topic_root}/knob/{name}/state");
        let command_topic = format!("{topic_root}/knob/{name}/set");
        let unique_id = format!("{NODE_ID}_knob_{ha_name}");
        let config_topic = format!("{HA_ROOT}/{component}/{NODE_ID}/knob_{ha_name}/config");

        let mut config = json!({
            "name": format!("Knob: {name}"),
            "unique_id": unique_id,
            "state_topic": state_topic,
            "command_topic": command_topic,
            "device": device_block(),
            "retain": false,
        });
        if let Some(extra_obj) = extra.as_object() {
            let config_obj = config.as_object_mut().unwrap();
            for (k, v) in extra_obj {
                config_obj.insert(k.clone(), v.clone());
            }
        }

        client
            .publish(&config_topic, QoS::AtLeastOnce, true, config.to_string())
            .await?;
        debug!(topic = %config_topic, "HA discovery knob published");
        count += 1;
    }
    Ok(count)
}

async fn publish_kill_switch(client: &AsyncClient, topic_root: &str) -> Result<usize> {
    let config = json!({
        "name": "Writes enabled (kill switch)",
        "unique_id": format!("{NODE_ID}_writes_enabled"),
        "state_topic": format!("{topic_root}/writes_enabled/state"),
        "command_topic": format!("{topic_root}/writes_enabled/set"),
        "payload_on": "true",
        "payload_off": "false",
        "state_on": "true",
        "state_off": "false",
        "device": device_block(),
    });
    let topic = format!("{HA_ROOT}/switch/{NODE_ID}/writes_enabled/config");
    client
        .publish(&topic, QoS::AtLeastOnce, true, config.to_string())
        .await?;
    Ok(1)
}

async fn publish_phases(client: &AsyncClient, topic_root: &str) -> Result<usize> {
    // PR-rename-entities: topic-tails are dotted form. Distinct from
    // the actuated decision/target disambiguation in `actuated_name` —
    // phases publish only the actuated side, so `.target` is implicit.
    let ids = [
        (ActuatedId::GridSetpoint, "grid.setpoint"),
        (ActuatedId::InputCurrentLimit, "inverter.input.current-limit"),
        (ActuatedId::ZappiMode, "evcharger.mode.target"),
        (ActuatedId::EddiMode, "eddi.mode.target"),
        (ActuatedId::Schedule0, "schedule.0"),
        (ActuatedId::Schedule1, "schedule.1"),
    ];
    let mut count = 0;
    for (_id, name) in ids {
        let ha_name = ha_safe(name);
        let topic = format!("{HA_ROOT}/sensor/{NODE_ID}/phase_{ha_name}/config");
        let config = json!({
            "name": format!("Phase: {name}"),
            "unique_id": format!("{NODE_ID}_phase_{ha_name}"),
            "state_topic": format!("{topic_root}/entity/{name}/phase"),
            "value_template": "{{ value_json.phase }}",
            "device": device_block(),
        });
        client
            .publish(&topic, QoS::AtLeastOnce, true, config.to_string())
            .await?;
        count += 1;
    }
    Ok(count)
}

/// PR-ha-discovery-expand: emit a `sensor` discovery config for every
/// scalar `SensorId`. Per-sensor unit/device_class/state_class come from
/// `sensor_meta`. Stale/Unknown freshness on the wire is `"unavailable"`
/// (encoded in the state body — no separate `availability_topic`); HA's
/// MQTT integration recognises that token natively.
async fn publish_sensors(client: &AsyncClient, topic_root: &str) -> Result<usize> {
    let mut count = 0;
    for &id in SensorId::ALL {
        // PR-AS-C: skip actuated-mirror variants — their values are
        // surfaced via the dedicated `Actuated` HA entities
        // (`PublishPayload::ActuatedPhase`), so re-publishing them as
        // plain sensors would clutter discovery.
        if id.actuated_id().is_some() {
            continue;
        }
        let name = sensor_name(id);
        let ha_name = ha_safe(name);
        let meta = sensor_meta(id);
        let state_topic = format!("{topic_root}/sensor/{name}/state");
        let unique_id = format!("{NODE_ID}_sensor_{ha_name}");
        let config_topic = format!("{HA_ROOT}/sensor/{NODE_ID}/sensor_{ha_name}/config");

        let mut config = json!({
            "name": format!("Sensor: {name}"),
            "unique_id": unique_id,
            "state_topic": state_topic,
            "state_class": meta.state_class,
            "device": device_block(),
        });
        if let Some(unit) = meta.unit {
            config
                .as_object_mut()
                .expect("json object")
                .insert("unit_of_measurement".to_string(), json!(unit));
        }
        if let Some(dc) = meta.device_class {
            config
                .as_object_mut()
                .expect("json object")
                .insert("device_class".to_string(), json!(dc));
        }

        client
            .publish(&config_topic, QoS::AtLeastOnce, true, config.to_string())
            .await?;
        debug!(topic = %config_topic, "HA discovery sensor published");
        count += 1;
    }
    Ok(count)
}

/// PR-ha-discovery-expand: discovery configs for the controller-relevant
/// bookkeeping fields. Booleans go to `binary_sensor`; numerics to
/// `sensor`. SoC fields carry `unit_of_measurement = "%"`.
async fn publish_bookkeeping(client: &AsyncClient, topic_root: &str) -> Result<usize> {
    let mut count = 0;

    // Booleans
    for id in [
        BookkeepingId::ZappiActive,
        BookkeepingId::ChargeToFullRequired,
        BookkeepingId::ChargeBatteryExtendedToday,
    ] {
        let name = id.name();
        let ha_name = ha_safe(name);
        let state_topic = format!("{topic_root}/bookkeeping/{name}/state");
        let config_topic =
            format!("{HA_ROOT}/binary_sensor/{NODE_ID}/bookkeeping_{ha_name}/config");
        let config = json!({
            "name": format!("Bookkeeping: {name}"),
            "unique_id": format!("{NODE_ID}_bookkeeping_{ha_name}"),
            "state_topic": state_topic,
            "payload_on": "true",
            "payload_off": "false",
            "device": device_block(),
        });
        client
            .publish(&config_topic, QoS::AtLeastOnce, true, config.to_string())
            .await?;
        count += 1;
    }

    // Numerics.
    for (id, unit) in [
        (BookkeepingId::SocEndOfDayTarget, Some("%")),
        (BookkeepingId::EffectiveExportSocThreshold, Some("%")),
        (BookkeepingId::BatterySelectedSocTarget, Some("%")),
    ] {
        let name = id.name();
        let ha_name = ha_safe(name);
        let state_topic = format!("{topic_root}/bookkeeping/{name}/state");
        let config_topic =
            format!("{HA_ROOT}/sensor/{NODE_ID}/bookkeeping_{ha_name}/config");
        let mut config = json!({
            "name": format!("Bookkeeping: {name}"),
            "unique_id": format!("{NODE_ID}_bookkeeping_{ha_name}"),
            "state_topic": state_topic,
            "state_class": "measurement",
            "device": device_block(),
        });
        if let Some(u) = unit {
            config
                .as_object_mut()
                .expect("json object")
                .insert("unit_of_measurement".to_string(), json!(u));
        }
        client
            .publish(&config_topic, QoS::AtLeastOnce, true, config.to_string())
            .await?;
        count += 1;
    }

    Ok(count)
}

/// Per-sensor HA discovery metadata. See plan §"Discovery payload table".
struct SensorMeta {
    unit: Option<&'static str>,
    device_class: Option<&'static str>,
    state_class: &'static str,
}

/// PR-AS-C: actuated-mirror `SensorId` variants are filtered out by
/// `publish_sensors` before reaching this function — their HA entity
/// surface lives under the actuated table (`PublishPayload::
/// ActuatedPhase`), not the sensor table. Hitting this fallthrough
/// indicates a missing filter at a caller.
fn sensor_meta(id: SensorId) -> SensorMeta {
    use SensorId::*;
    match id {
        // PR-ev-soc-sensor: `EvSoc` is also a `%`-unit battery — same
        // HA shape as `BatterySoc` / `BatterySoh`. Folded into this arm
        // so future tweaks to the battery-percent sensor metadata
        // (precision, icon, …) propagate uniformly.
        //
        // PR-auto-extended-charge: `EvChargeTarget` is also a `%`-unit
        // EV-side battery threshold — same shape.
        BatterySoc | BatterySoh | EvSoc | EvChargeTarget => SensorMeta {
            unit: Some("%"),
            device_class: Some("battery"),
            state_class: "measurement",
        },
        BatteryInstalledCapacity => SensorMeta {
            // Victron's /InstalledCapacity is in Ah (not kWh); the Wh
            // computation downstream multiplies by SoH and nominal
            // voltage. No HA `device_class` fits Ah cleanly — leave it
            // None so HA renders the value with the unit only.
            unit: Some("Ah"),
            device_class: None,
            state_class: "measurement",
        },
        // PR-ZD-1: HeatPumpPower and CookerPower are W / power sensors,
        // merged with the existing instantaneous-power arm.
        BatteryDcPower | MpptPower0 | MpptPower1 | SoltaroPower | PowerConsumption
        | GridPower | OffgridPower | EvchargerAcPower
        | HeatPumpPower | CookerPower => SensorMeta {
            unit: Some("W"),
            device_class: Some("power"),
            state_class: "measurement",
        },
        GridCurrent | ConsumptionCurrent | OffgridCurrent | VebusInputCurrent
        | EvchargerAcCurrent => SensorMeta {
            unit: Some("A"),
            device_class: Some("current"),
            state_class: "measurement",
        },
        GridVoltage => SensorMeta {
            unit: Some("V"),
            device_class: Some("voltage"),
            state_class: "measurement",
        },
        OutdoorTemperature => SensorMeta {
            unit: Some("°C"),
            device_class: Some("temperature"),
            state_class: "measurement",
        },
        // PR-ZD-1: MPPT operation-mode codes are dimensionless integer
        // enums — merged with EssState (also dimensionless) per clippy.
        EssState | Mppt0OperationMode | Mppt1OperationMode => SensorMeta {
            unit: None,
            device_class: None,
            state_class: "measurement",
        },
        SessionKwh => SensorMeta {
            unit: Some("kWh"),
            device_class: Some("energy"),
            state_class: "total_increasing",
        },
        GridSetpointActual
        | InputCurrentLimitActual
        | Schedule0StartActual
        | Schedule0DurationActual
        | Schedule0SocActual
        | Schedule0DaysActual
        | Schedule0AllowDischargeActual
        | Schedule1StartActual
        | Schedule1DurationActual
        | Schedule1SocActual
        | Schedule1DaysActual
        | Schedule1AllowDischargeActual => unreachable!(
            "actuated-mirror SensorId {id:?} reached sensor_meta — caller \
             must filter via id.actuated_id().is_some()"
        ),
    }
}

/// Per-knob schema: (id, HA component, extras like min/max/step/options).
fn knob_schemas() -> Vec<(KnobId, &'static str, serde_json::Value)> {
    vec![
        (KnobId::ForceDisableExport, "switch", json!({"payload_on": "true", "payload_off": "false"})),
        (KnobId::DisableNightGridDischarge, "switch", json!({"payload_on": "true", "payload_off": "false"})),
        (KnobId::ChargeCarBoost, "switch", json!({"payload_on": "true", "payload_off": "false"})),
        // PR-auto-extended-charge: tri-state select replaces the legacy
        // bool switch entity for EV extended charge.
        (
            KnobId::ChargeCarExtendedMode,
            "select",
            json!({"options": ["auto", "forced", "disabled"]}),
        ),
        (KnobId::AllowBatteryToCar, "switch", json!({"payload_on": "true", "payload_off": "false"})),
        // PR-inverter-safe-discharge-knob.
        (KnobId::InverterSafeDischargeEnable, "switch", json!({"payload_on": "true", "payload_off": "false"})),

        // PR-06-D01: min/max come from knob_range() — the single
        // source of truth shared with parse_knob_value's ingest
        // validation. Only step + unit are owned here.
        number_knob(KnobId::ExportSocThreshold, 1.0, Some("%")),
        number_knob(KnobId::DischargeSocTarget, 1.0, Some("%")),
        number_knob(KnobId::BatterySocTarget, 1.0, Some("%")),
        number_knob(KnobId::FullChargeDischargeSocTarget, 1.0, Some("%")),
        number_knob(KnobId::FullChargeExportSocThreshold, 1.0, Some("%")),
        number_knob(KnobId::PessimismMultiplierModifier, 0.05, None),
        number_knob(KnobId::ZappiCurrentTarget, 0.5, Some("A")),
        // A-14: zappi_limit is kWh (per-session EV charge ceiling), not
        // a percentage. Step 0.5 kWh — matches typical EV metering
        // granularity.
        number_knob(KnobId::ZappiLimit, 0.5, Some("kWh")),
        number_knob(KnobId::ZappiEmergencyMargin, 0.5, Some("A")),
        number_knob(KnobId::GridExportLimitW, 50.0, Some("W")),
        number_knob(KnobId::GridImportLimitW, 10.0, Some("W")),
        number_knob(KnobId::EddiEnableSoc, 1.0, Some("%")),
        number_knob(KnobId::EddiDisableSoc, 1.0, Some("%")),
        number_knob(KnobId::EddiDwellS, 5.0, Some("s")),
        number_knob(KnobId::WeathersocWinterTemperatureThreshold, 0.5, Some("°C")),
        number_knob(KnobId::WeathersocLowEnergyThreshold, 1.0, Some("kWh")),
        number_knob(KnobId::WeathersocOkEnergyThreshold, 1.0, Some("kWh")),
        number_knob(KnobId::WeathersocHighEnergyThreshold, 1.0, Some("kWh")),
        number_knob(KnobId::WeathersocTooMuchEnergyThreshold, 1.0, Some("kWh")),
        // PR-baseline-forecast: 4 runtime knobs.
        // Date knobs use step=1 (integer MMDD literal); operator types
        // 1101 / 301 etc. The HA UI is a plain integer slider — not
        // ideal but keeps the knob count at 4. Step 1 matches the
        // integer wire form.
        number_knob(KnobId::BaselineWinterStartMmDd, 1.0, None),
        number_knob(KnobId::BaselineWinterEndMmDd, 1.0, None),
        number_knob(KnobId::BaselineWhPerHourWinter, 10.0, Some("Wh/h")),
        number_knob(KnobId::BaselineWhPerHourSummer, 10.0, Some("Wh/h")),
        // PR-keep-batteries-charged.
        (
            KnobId::KeepBatteriesChargedDuringFullCharge,
            "switch",
            json!({"payload_on": "true", "payload_off": "false"}),
        ),
        number_knob(KnobId::SunriseSunsetOffsetMin, 5.0, Some("min")),
        (
            KnobId::FullChargeDeferToNextSunday,
            "switch",
            json!({"payload_on": "true", "payload_off": "false"}),
        ),
        number_knob(KnobId::FullChargeSnapBackMaxWeekday, 1.0, None),

        (KnobId::DischargeTime, "select", json!({"options": ["02:00", "23:00"]})),
        (KnobId::DebugFullCharge, "select", json!({"options": ["auto", "force", "forbid"]})),
        (
            KnobId::ForecastDisagreementStrategy,
            "select",
            json!({"options": ["max", "min", "mean", "solcast_if_available_else_mean"]}),
        ),
        (
            KnobId::ChargeBatteryExtendedMode,
            "select",
            json!({"options": ["auto", "forced", "disabled"]}),
        ),
        // PR-gamma-hold-redesign: 4 source-selector knobs that pick
        // between weather_soc-derived bookkeeping ("weather") and the
        // user's forced override ("forced"). Lowercase wire values
        // match the parse arm in `serialize.rs::parse_knob_value`.
        (
            KnobId::ExportSocThresholdMode,
            "select",
            json!({"options": ["weather", "forced"]}),
        ),
        (
            KnobId::DischargeSocTargetMode,
            "select",
            json!({"options": ["weather", "forced"]}),
        ),
        (
            KnobId::BatterySocTargetMode,
            "select",
            json!({"options": ["weather", "forced"]}),
        ),
        (
            KnobId::DisableNightGridDischargeMode,
            "select",
            json!({"options": ["weather", "forced"]}),
        ),
    ]
}
