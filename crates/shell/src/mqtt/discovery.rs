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
//!
//! The HA discovery spec requires per-entity config payloads on a
//! well-known topic path. See
//! <https://www.home-assistant.io/integrations/mqtt/#mqtt-discovery>.

use anyhow::Result;
use rumqttc::{AsyncClient, QoS};
use serde_json::json;
use tracing::{debug, info};

use victron_controller_core::types::{ActuatedId, KnobId};

use super::serialize::{knob_name, knob_range};

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
        + publish_phases(client, topic_root).await?;
    info!(count = total, "HA discovery published");
    Ok(())
}

async fn publish_knobs(client: &AsyncClient, topic_root: &str) -> Result<usize> {
    let mut count = 0;
    for (id, component, extra) in knob_schemas() {
        let name = knob_name(id);
        let state_topic = format!("{topic_root}/knob/{name}/state");
        let command_topic = format!("{topic_root}/knob/{name}/set");
        let unique_id = format!("{NODE_ID}_knob_{name}");
        let config_topic = format!("{HA_ROOT}/{component}/{NODE_ID}/knob_{name}/config");

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
    let ids = [
        (ActuatedId::GridSetpoint, "grid_setpoint"),
        (ActuatedId::InputCurrentLimit, "input_current_limit"),
        (ActuatedId::ZappiMode, "zappi_mode"),
        (ActuatedId::EddiMode, "eddi_mode"),
        (ActuatedId::Schedule0, "schedule_0"),
        (ActuatedId::Schedule1, "schedule_1"),
    ];
    let mut count = 0;
    for (_id, name) in ids {
        let topic = format!("{HA_ROOT}/sensor/{NODE_ID}/phase_{name}/config");
        let config = json!({
            "name": format!("Phase: {name}"),
            "unique_id": format!("{NODE_ID}_phase_{name}"),
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

/// Per-knob schema: (id, HA component, extras like min/max/step/options).
fn knob_schemas() -> Vec<(KnobId, &'static str, serde_json::Value)> {
    vec![
        (KnobId::ForceDisableExport, "switch", json!({"payload_on": "true", "payload_off": "false"})),
        (KnobId::DisableNightGridDischarge, "switch", json!({"payload_on": "true", "payload_off": "false"})),
        (KnobId::ChargeCarBoost, "switch", json!({"payload_on": "true", "payload_off": "false"})),
        (KnobId::ChargeCarExtended, "switch", json!({"payload_on": "true", "payload_off": "false"})),
        (KnobId::AllowBatteryToCar, "switch", json!({"payload_on": "true", "payload_off": "false"})),

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

        (KnobId::DischargeTime, "select", json!({"options": ["02:00", "23:00"]})),
        (KnobId::DebugFullCharge, "select", json!({"options": ["none", "force", "forbid"]})),
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
    ]
}
