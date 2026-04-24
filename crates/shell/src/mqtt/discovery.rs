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

use super::serialize::knob_name;

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

        (KnobId::ExportSocThreshold, "number", json!({"min": 0, "max": 100, "step": 1, "unit_of_measurement": "%"})),
        (KnobId::DischargeSocTarget, "number", json!({"min": 0, "max": 100, "step": 1, "unit_of_measurement": "%"})),
        (KnobId::BatterySocTarget, "number", json!({"min": 0, "max": 100, "step": 1, "unit_of_measurement": "%"})),
        (KnobId::FullChargeDischargeSocTarget, "number", json!({"min": 0, "max": 100, "step": 1, "unit_of_measurement": "%"})),
        (KnobId::FullChargeExportSocThreshold, "number", json!({"min": 0, "max": 100, "step": 1, "unit_of_measurement": "%"})),
        (KnobId::PessimismMultiplierModifier, "number", json!({"min": 0.0, "max": 2.0, "step": 0.05})),
        (KnobId::ZappiCurrentTarget, "number", json!({"min": 6.0, "max": 32.0, "step": 0.5, "unit_of_measurement": "A"})),
        (KnobId::ZappiLimit, "number", json!({"min": 1.0, "max": 100.0, "step": 1.0, "unit_of_measurement": "%"})),
        (KnobId::ZappiEmergencyMargin, "number", json!({"min": 0.0, "max": 10.0, "step": 0.5, "unit_of_measurement": "A"})),
        (KnobId::GridExportLimitW, "number", json!({"min": 0, "max": 10000, "step": 50, "unit_of_measurement": "W"})),
        (KnobId::GridImportLimitW, "number", json!({"min": 0, "max": 10000, "step": 10, "unit_of_measurement": "W"})),
        (KnobId::EddiEnableSoc, "number", json!({"min": 50, "max": 100, "step": 1, "unit_of_measurement": "%"})),
        (KnobId::EddiDisableSoc, "number", json!({"min": 50, "max": 100, "step": 1, "unit_of_measurement": "%"})),
        (KnobId::EddiDwellS, "number", json!({"min": 0, "max": 3600, "step": 5, "unit_of_measurement": "s"})),
        (KnobId::WeathersocWinterTemperatureThreshold, "number", json!({"min": -30, "max": 40, "step": 0.5, "unit_of_measurement": "°C"})),
        (KnobId::WeathersocLowEnergyThreshold, "number", json!({"min": 0, "max": 500, "step": 1, "unit_of_measurement": "kWh"})),
        (KnobId::WeathersocOkEnergyThreshold, "number", json!({"min": 0, "max": 500, "step": 1, "unit_of_measurement": "kWh"})),
        (KnobId::WeathersocHighEnergyThreshold, "number", json!({"min": 0, "max": 500, "step": 1, "unit_of_measurement": "kWh"})),
        (KnobId::WeathersocTooMuchEnergyThreshold, "number", json!({"min": 0, "max": 500, "step": 1, "unit_of_measurement": "kWh"})),

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
