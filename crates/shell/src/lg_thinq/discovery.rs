//! HA MQTT-discovery payloads for the LG ThinQ bridge.
//!
//! Six entities total: 2 switches (heating + DHW power), 2 numbers
//! (heating + DHW target temp), 2 sensors (DHW actual + heating-water
//! actual). All retained, one-shot at startup.

use anyhow::{Context, Result};
use rumqttc::{AsyncClient, QoS};
use serde_json::{Value, json};

use super::{
    KNOB_DHW_POWER, KNOB_DHW_TARGET, KNOB_HEAT_PUMP_POWER, KNOB_HEATING_TARGET,
    SENSOR_DHW_ACTUAL, SENSOR_HEATING_ACTUAL,
};

const HA_ROOT: &str = "homeassistant";
const NODE_ID: &str = "victron_controller_lg_thinq";

fn device_block() -> Value {
    json!({
        "identifiers": [NODE_ID],
        "name": "LG ThinQ Heat Pump",
        "manufacturer": "LG",
        "model": "HM051M.U43",
        "via_device": "victron_controller",
    })
}

fn base_config(
    name: &str,
    unique_suffix: &str,
    state_topic: &str,
    command_topic: Option<&str>,
    availability_topic: &str,
) -> Value {
    let mut cfg = json!({
        "name": name,
        "unique_id": format!("{NODE_ID}_{unique_suffix}"),
        "state_topic": state_topic,
        "availability_topic": availability_topic,
        "payload_available": "online",
        "payload_not_available": "offline",
        "device": device_block(),
    });
    if let Some(cmd) = command_topic {
        cfg.as_object_mut()
            .unwrap()
            .insert("command_topic".to_string(), json!(cmd));
    }
    cfg
}

pub async fn publish_all(
    client: &AsyncClient,
    topic_root: &str,
    availability_topic: &str,
    heating_range: (u32, u32),
    dhw_range: (u32, u32),
) -> Result<()> {
    // ---- Switches (heating + DHW power) -------------------------------
    publish_one(
        client,
        &format!("{HA_ROOT}/switch/{NODE_ID}/{KNOB_HEAT_PUMP_POWER}/config"),
        switch_payload(
            "LG Heat Pump: Heating Power",
            KNOB_HEAT_PUMP_POWER,
            topic_root,
            availability_topic,
        ),
    )
    .await?;
    publish_one(
        client,
        &format!("{HA_ROOT}/switch/{NODE_ID}/{KNOB_DHW_POWER}/config"),
        switch_payload(
            "LG Heat Pump: DHW Power",
            KNOB_DHW_POWER,
            topic_root,
            availability_topic,
        ),
    )
    .await?;

    // ---- Numbers (target temperatures) --------------------------------
    publish_one(
        client,
        &format!("{HA_ROOT}/number/{NODE_ID}/{KNOB_HEATING_TARGET}/config"),
        number_payload(
            "LG Heat Pump: Heating Target",
            KNOB_HEATING_TARGET,
            topic_root,
            availability_topic,
            heating_range,
        ),
    )
    .await?;
    publish_one(
        client,
        &format!("{HA_ROOT}/number/{NODE_ID}/{KNOB_DHW_TARGET}/config"),
        number_payload(
            "LG Heat Pump: DHW Target",
            KNOB_DHW_TARGET,
            topic_root,
            availability_topic,
            dhw_range,
        ),
    )
    .await?;

    // ---- Sensors (readbacks) ------------------------------------------
    publish_one(
        client,
        &format!("{HA_ROOT}/sensor/{NODE_ID}/{SENSOR_DHW_ACTUAL}/config"),
        sensor_payload(
            "LG Heat Pump: DHW Current Temperature",
            SENSOR_DHW_ACTUAL,
            topic_root,
            availability_topic,
        ),
    )
    .await?;
    publish_one(
        client,
        &format!("{HA_ROOT}/sensor/{NODE_ID}/{SENSOR_HEATING_ACTUAL}/config"),
        sensor_payload(
            "LG Heat Pump: Heating Water Out Temperature",
            SENSOR_HEATING_ACTUAL,
            topic_root,
            availability_topic,
        ),
    )
    .await?;

    Ok(())
}

fn switch_payload(name: &str, knob: &str, root: &str, availability: &str) -> Value {
    let state = format!("{root}/knob/{knob}/state");
    let cmd = format!("{root}/knob/{knob}/set");
    let mut cfg = base_config(name, knob, &state, Some(&cmd), availability);
    let obj = cfg.as_object_mut().unwrap();
    obj.insert("payload_on".to_string(), json!("ON"));
    obj.insert("payload_off".to_string(), json!("OFF"));
    obj.insert("state_on".to_string(), json!("ON"));
    obj.insert("state_off".to_string(), json!("OFF"));
    obj.insert("retain".to_string(), json!(false));
    cfg
}

fn number_payload(
    name: &str,
    knob: &str,
    root: &str,
    availability: &str,
    range: (u32, u32),
) -> Value {
    let state = format!("{root}/knob/{knob}/state");
    let cmd = format!("{root}/knob/{knob}/set");
    let mut cfg = base_config(name, knob, &state, Some(&cmd), availability);
    let obj = cfg.as_object_mut().unwrap();
    obj.insert("min".to_string(), json!(range.0));
    obj.insert("max".to_string(), json!(range.1));
    obj.insert("step".to_string(), json!(1));
    obj.insert("unit_of_measurement".to_string(), json!("°C"));
    obj.insert("mode".to_string(), json!("slider"));
    obj.insert("retain".to_string(), json!(false));
    cfg
}

fn sensor_payload(name: &str, sensor: &str, root: &str, availability: &str) -> Value {
    let state = format!("{root}/sensor/{sensor}/state");
    let mut cfg = base_config(name, sensor, &state, None, availability);
    let obj = cfg.as_object_mut().unwrap();
    obj.insert("device_class".to_string(), json!("temperature"));
    obj.insert("state_class".to_string(), json!("measurement"));
    obj.insert("unit_of_measurement".to_string(), json!("°C"));
    cfg
}

async fn publish_one(client: &AsyncClient, topic: &str, payload: Value) -> Result<()> {
    client
        .publish(topic, QoS::AtLeastOnce, true, payload.to_string())
        .await
        .with_context(|| format!("publish {topic}"))
}
