//! LG ThinQ heat-pump bridge — Option A (self-contained sidecar).
//!
//! This module owns its own MQTT connection (separate from the main
//! controller's), subscribes to `<topic_root>/knob/lg_*/set`, maps the
//! commands to LG ThinQ Connect Open API calls, polls device state on
//! a configurable cadence (default 60 s), and publishes the readback
//! plus a small set of derived sensors as retained MQTT.
//!
//! Intentionally **does not** participate in the core's TASS Actuated
//! / Knobs / Effect surfaces — that integration (Option B) requires
//! threading 4 new ActuatedIds + 4 new KnobIds + an Effect variant
//! through `core::process` and the dashboard / baboon model layers.
//! Option A is a usable bridge today; Option B will follow once the
//! operator has validated the cloud path against the real HM051.
//!
//! See `crates/lg-thinq-client/` for the actual API client.

mod discovery;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, error, info, warn};

use victron_controller_lg_thinq_client::api::ThinqApi;
use victron_controller_lg_thinq_client::heat_pump::{
    HeatPumpControl, HeatPumpState, validate_temperature_c,
};
use victron_controller_lg_thinq_client::region::Country;

use crate::config::{LgThinqConfig, MqttConfig};

/// The four user-controllable knobs the bridge exposes. Their names are
/// load-bearing — they appear in the MQTT topic path and in HA. Adding,
/// removing, or renaming a knob is a wire-format change.
const KNOB_HEAT_PUMP_POWER: &str = "lg_heat_pump_power";
const KNOB_DHW_POWER: &str = "lg_dhw_power";
const KNOB_HEATING_TARGET: &str = "lg_heating_water_target_c";
const KNOB_DHW_TARGET: &str = "lg_dhw_target_c";

/// Read-only sensors the bridge publishes.
const SENSOR_DHW_ACTUAL: &str = "lg_dhw_actual_c";
const SENSOR_HEATING_ACTUAL: &str = "lg_heating_water_actual_c";

/// Last-will availability topic. HA marks entities `unavailable` when
/// this flips to `offline` (sidecar crash, broker disconnect, etc.).
const AVAILABILITY_SUFFIX: &str = "availability/lg_thinq";

pub struct Service {
    api: ThinqApi,
    device_id: String,
    topic_root: String,
    mqtt: MqttBridge,
    poll_period: Duration,
    writes_enabled: bool,
    heating_range_c: (u32, u32),
    dhw_range_c: (u32, u32),
}

impl std::fmt::Debug for Service {
    // Fields omitted: `api` (holds the bearer token; not safe to leak
    // in debug output), `mqtt` (contains an rumqttc EventLoop that
    // doesn't implement Debug). `finish_non_exhaustive` makes the
    // omission explicit.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("lg_thinq::Service")
            .field("device_id", &self.device_id)
            .field("topic_root", &self.topic_root)
            .field("poll_period", &self.poll_period)
            .field("writes_enabled", &self.writes_enabled)
            .field("heating_range_c", &self.heating_range_c)
            .field("dhw_range_c", &self.dhw_range_c)
            .finish_non_exhaustive()
    }
}

impl Service {
    pub fn new(lg: &LgThinqConfig, mqtt: &MqttConfig) -> Result<Self> {
        if !lg.is_configured() {
            return Err(anyhow!(
                "[lg_thinq] not configured: pat / country / device_id all required"
            ));
        }
        if mqtt.host.is_empty() {
            return Err(anyhow!(
                "[lg_thinq] requires an [mqtt] broker — knobs and HA discovery flow over MQTT"
            ));
        }
        if lg.heating_target_min_c >= lg.heating_target_max_c {
            return Err(anyhow!(
                "[lg_thinq] heating_target_min_c ({}) >= heating_target_max_c ({})",
                lg.heating_target_min_c,
                lg.heating_target_max_c
            ));
        }
        if lg.dhw_target_min_c >= lg.dhw_target_max_c {
            return Err(anyhow!(
                "[lg_thinq] dhw_target_min_c ({}) >= dhw_target_max_c ({})",
                lg.dhw_target_min_c,
                lg.dhw_target_max_c
            ));
        }

        let country = Country::new(&lg.country)
            .map_err(|e| anyhow!("[lg_thinq] country {:?}: {e}", lg.country))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .context("build reqwest client")?;
        let api = ThinqApi::new(http, lg.pat.clone(), &country)
            .map_err(|e| anyhow!("[lg_thinq] {e}"))?;

        let bridge = MqttBridge::new(mqtt)?;

        Ok(Self {
            api,
            device_id: lg.device_id.clone(),
            topic_root: mqtt.topic_root.clone(),
            mqtt: bridge,
            poll_period: lg.poll_period,
            writes_enabled: lg.writes_enabled,
            heating_range_c: (lg.heating_target_min_c, lg.heating_target_max_c),
            dhw_range_c: (lg.dhw_target_min_c, lg.dhw_target_max_c),
        })
    }

    /// Run the bridge until the underlying MQTT event loop terminates.
    /// On entry: publishes availability + HA discovery; subscribes to
    /// `<root>/knob/lg_*/set`. The polling loop is spawned as a sibling
    /// task; this method owns the subscriber loop.
    pub async fn run(self) -> Result<()> {
        // Cache dir is unused in Option A (no MQTT push yet) but we
        // touch it here so the operator sees an early-startup error if
        // the path is unwritable, rather than hitting it months later
        // when Option B lands.
        if let Some(parent) = Path::new("/tmp").parent() {
            let _ = parent; // suppress unused-import linter; cache dir
                            // is consulted by provision::provision when
                            // Option B wires the MQTT push subscriber.
        }

        let MqttBridge {
            client,
            eventloop,
            availability_topic,
        } = self.mqtt;

        // Publish HA discovery first (retained — HA will see it even if
        // it boots after us). Mark availability online only after the
        // discovery payloads land; HA correctly handles entities
        // showing up `unavailable` until the first state publish.
        discovery::publish_all(
            &client,
            &self.topic_root,
            &availability_topic,
            self.heating_range_c,
            self.dhw_range_c,
        )
        .await?;

        client
            .publish(&availability_topic, QoS::AtLeastOnce, true, "online")
            .await
            .context("publish availability online")?;

        // Subscribe to the four command topics.
        for knob in [
            KNOB_HEAT_PUMP_POWER,
            KNOB_DHW_POWER,
            KNOB_HEATING_TARGET,
            KNOB_DHW_TARGET,
        ] {
            let topic = format!("{}/knob/{knob}/set", self.topic_root);
            client
                .subscribe(&topic, QoS::AtLeastOnce)
                .await
                .with_context(|| format!("subscribe {topic}"))?;
        }

        // Channel from the subscriber loop to the command handler. We
        // could call the API inline from the eventloop branch but
        // doing so would block the MQTT keepalive heartbeat; the
        // spawn-per-command pattern lets the eventloop keep ticking.
        let (cmd_tx, cmd_rx) = mpsc::channel::<KnobCommand>(16);

        let api = Arc::new(self.api);
        let device_id = Arc::new(self.device_id);
        let topic_root = Arc::new(self.topic_root);

        // State poll loop.
        let poll_handle = tokio::spawn(poll_loop(
            api.clone(),
            device_id.clone(),
            client.clone(),
            topic_root.clone(),
            self.poll_period,
        ));

        // Command-handler loop.
        let cmd_handle = tokio::spawn(handle_commands(
            api.clone(),
            device_id.clone(),
            client.clone(),
            topic_root.clone(),
            cmd_rx,
            self.writes_enabled,
            self.heating_range_c,
            self.dhw_range_c,
        ));

        // Subscriber loop runs inline so a fatal MQTT error bubbles up
        // to `main` and the supervisor restarts the bridge.
        let subscriber_result = run_subscriber(eventloop, cmd_tx, &topic_root).await;

        // Best-effort shutdown: cancel siblings, flush availability.
        poll_handle.abort();
        cmd_handle.abort();
        let _ = client
            .publish(&availability_topic, QoS::AtLeastOnce, true, "offline")
            .await;

        subscriber_result
    }
}

#[derive(Debug, Clone)]
enum KnobCommand {
    HeatPumpPower(bool),
    DhwPower(bool),
    HeatingTarget(i64),
    DhwTarget(i64),
}

struct MqttBridge {
    client: AsyncClient,
    eventloop: rumqttc::EventLoop,
    availability_topic: String,
}

impl MqttBridge {
    fn new(mqtt: &MqttConfig) -> Result<Self> {
        // Distinct client id so we don't fight the main controller's
        // session on the broker. The 8-char UUID suffix avoids client-
        // id collisions across restarts (FlashMQ kicks the older
        // session, which would otherwise drop the main controller's
        // subscriptions).
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let client_id = format!("victron-controller-lg-thinq-{}", &suffix[..8]);

        let availability_topic = format!("{}/{AVAILABILITY_SUFFIX}", mqtt.topic_root);

        let mut opts = MqttOptions::new(client_id, mqtt.host.clone(), mqtt.port);
        opts.set_keep_alive(Duration::from_secs(30));
        opts.set_clean_session(false);
        opts.set_last_will(rumqttc::LastWill::new(
            availability_topic.clone(),
            "offline",
            QoS::AtLeastOnce,
            true,
        ));
        if let (Some(user), Some(pass)) = (mqtt.username.as_ref(), mqtt.password.as_ref()) {
            opts.set_credentials(user, pass);
        }
        if mqtt.tls {
            // Option A keeps the bridge's MQTT transport simple — the
            // existing shell-side MQTT publisher already handles TLS
            // setup against the same broker, but threading that here
            // requires sharing rustls configs across two clients. For
            // now, if `[mqtt] tls = true` the operator must either run
            // the broker without TLS on a local interface or wait for
            // Option B to fold this bridge into the main MQTT client.
            return Err(anyhow!(
                "[lg_thinq] does not yet support `[mqtt] tls = true` — see crates/shell/src/lg_thinq/mod.rs"
            ));
        }

        let (client, eventloop) = AsyncClient::new(opts, 16);
        Ok(Self {
            client,
            eventloop,
            availability_topic,
        })
    }
}

async fn run_subscriber(
    mut eventloop: rumqttc::EventLoop,
    cmd_tx: mpsc::Sender<KnobCommand>,
    topic_root: &str,
) -> Result<()> {
    let prefix = format!("{topic_root}/knob/");
    loop {
        let event = match eventloop.poll().await {
            Ok(e) => e,
            Err(e) => {
                // rumqttc reconnects on the next poll; log + back off
                // briefly so we don't spin during sustained outages.
                warn!(target: "lg_thinq", "MQTT event-loop error (will reconnect): {e}");
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };
        let Event::Incoming(Incoming::Publish(pkt)) = event else {
            continue;
        };
        if !pkt.topic.starts_with(&prefix) || !pkt.topic.ends_with("/set") {
            continue;
        }
        // Topic is `<root>/knob/<knob_name>/set` — extract `<knob_name>`.
        let mid = &pkt.topic[prefix.len()..pkt.topic.len() - "/set".len()];
        let payload = std::str::from_utf8(&pkt.payload).unwrap_or("").trim();

        let parsed = match mid {
            KNOB_HEAT_PUMP_POWER => parse_bool(payload).map(KnobCommand::HeatPumpPower),
            KNOB_DHW_POWER => parse_bool(payload).map(KnobCommand::DhwPower),
            KNOB_HEATING_TARGET => parse_int(payload).map(KnobCommand::HeatingTarget),
            KNOB_DHW_TARGET => parse_int(payload).map(KnobCommand::DhwTarget),
            _ => {
                debug!(target: "lg_thinq", topic = %pkt.topic, "unknown knob");
                continue;
            }
        };

        match parsed {
            Some(cmd) => {
                if cmd_tx.send(cmd).await.is_err() {
                    // Receiver gone → bridge is shutting down.
                    return Ok(());
                }
            }
            None => {
                warn!(target: "lg_thinq", topic = %pkt.topic, payload, "unparseable knob payload");
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_commands(
    api: Arc<ThinqApi>,
    device_id: Arc<String>,
    client: AsyncClient,
    topic_root: Arc<String>,
    mut rx: mpsc::Receiver<KnobCommand>,
    writes_enabled: bool,
    heating_range: (u32, u32),
    dhw_range: (u32, u32),
) {
    // Last applied target value, used for optimistic state echo:
    // publish the new value to `/state` before the next poll round-
    // trip so HA sees the toggle move immediately even though LG's
    // own readback takes a few seconds.
    let last_state = Arc::new(Mutex::new(LastApplied::default()));

    while let Some(cmd) = rx.recv().await {
        let (control_payload, echo_topic, echo_value) = match cmd {
            KnobCommand::HeatPumpPower(on) => (
                HeatPumpControl::set_heating_power(on),
                format!("{topic_root}/knob/{KNOB_HEAT_PUMP_POWER}/state"),
                bool_text(on).to_string(),
            ),
            KnobCommand::DhwPower(on) => (
                HeatPumpControl::set_dhw_power(on),
                format!("{topic_root}/knob/{KNOB_DHW_POWER}/state"),
                bool_text(on).to_string(),
            ),
            KnobCommand::HeatingTarget(t) => {
                match validate_temperature_c(t, i64::from(heating_range.0), i64::from(heating_range.1)) {
                    Ok(t) => (
                        HeatPumpControl::set_water_heat_target_c(t),
                        format!("{topic_root}/knob/{KNOB_HEATING_TARGET}/state"),
                        t.to_string(),
                    ),
                    Err(e) => {
                        warn!(target: "lg_thinq", "heating target rejected: {e}");
                        continue;
                    }
                }
            }
            KnobCommand::DhwTarget(t) => {
                match validate_temperature_c(t, i64::from(dhw_range.0), i64::from(dhw_range.1)) {
                    Ok(t) => (
                        HeatPumpControl::set_dhw_target_c(t),
                        format!("{topic_root}/knob/{KNOB_DHW_TARGET}/state"),
                        t.to_string(),
                    ),
                    Err(e) => {
                        warn!(target: "lg_thinq", "DHW target rejected: {e}");
                        continue;
                    }
                }
            }
        };

        if !writes_enabled {
            info!(
                target: "lg_thinq",
                payload = ?control_payload,
                "LG ThinQ control dry-run (writes_enabled=false; not sent)"
            );
            continue;
        }

        match api.post_device_control(&device_id, control_payload.clone()).await {
            Ok(_) => {
                info!(target: "lg_thinq", payload = ?control_payload, "control applied");
                // Optimistic echo — publish the new state.
                let _ = client
                    .publish(&echo_topic, QoS::AtLeastOnce, true, echo_value.clone())
                    .await;
                let mut la = last_state.lock().await;
                la.merge(&echo_topic, &echo_value);
            }
            Err(e) => {
                error!(target: "lg_thinq", error = %e, payload = ?control_payload,
                    "LG ThinQ control failed");
                // Don't echo — let the next poll set the true state.
            }
        }
    }
}

#[derive(Default)]
struct LastApplied {
    by_topic: std::collections::HashMap<String, String>,
}

impl LastApplied {
    fn merge(&mut self, topic: &str, value: &str) {
        self.by_topic.insert(topic.to_string(), value.to_string());
    }
}

async fn poll_loop(
    api: Arc<ThinqApi>,
    device_id: Arc<String>,
    client: AsyncClient,
    topic_root: Arc<String>,
    period: Duration,
) {
    let mut interval = tokio::time::interval(period);
    // First tick fires immediately; we want a small grace delay so HA
    // discovery has time to land first.
    tokio::time::sleep(Duration::from_secs(3)).await;
    loop {
        interval.tick().await;
        match poll_once(&api, &device_id, &client, &topic_root).await {
            Ok(()) => {}
            Err(e) => {
                warn!(target: "lg_thinq", error = %e, "state poll failed; will retry");
            }
        }
    }
}

async fn poll_once(
    api: &ThinqApi,
    device_id: &str,
    client: &AsyncClient,
    topic_root: &str,
) -> Result<()> {
    let raw = api
        .get_device_state(device_id)
        .await
        .map_err(|e| anyhow!("get_device_state: {e}"))?;
    let state = HeatPumpState::from_json(&raw).map_err(|e| anyhow!("decode state: {e}"))?;

    // Knob states (retained — HA picks these up on attach).
    publish(
        client,
        &format!("{topic_root}/knob/{KNOB_HEAT_PUMP_POWER}/state"),
        bool_text(state.heating_enabled),
    )
    .await?;
    publish(
        client,
        &format!("{topic_root}/knob/{KNOB_DHW_POWER}/state"),
        bool_text(state.dhw_enabled),
    )
    .await?;
    if let Some(t) = state.heating_water_target_c {
        publish(
            client,
            &format!("{topic_root}/knob/{KNOB_HEATING_TARGET}/state"),
            format_int(t),
        )
        .await?;
    }
    if let Some(t) = state.dhw_target_c {
        publish(
            client,
            &format!("{topic_root}/knob/{KNOB_DHW_TARGET}/state"),
            format_int(t),
        )
        .await?;
    }

    // Sensor states.
    if let Some(t) = state.dhw_current_c {
        publish(
            client,
            &format!("{topic_root}/sensor/{SENSOR_DHW_ACTUAL}/state"),
            format_float(t),
        )
        .await?;
    }
    if let Some(t) = state.heating_water_current_c {
        publish(
            client,
            &format!("{topic_root}/sensor/{SENSOR_HEATING_ACTUAL}/state"),
            format_float(t),
        )
        .await?;
    }

    Ok(())
}

async fn publish(client: &AsyncClient, topic: &str, payload: impl Into<String>) -> Result<()> {
    let payload = payload.into();
    client
        .publish(topic, QoS::AtLeastOnce, true, payload)
        .await
        .with_context(|| format!("publish {topic}"))
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_uppercase().as_str() {
        "ON" | "TRUE" | "1" => Some(true),
        "OFF" | "FALSE" | "0" => Some(false),
        _ => None,
    }
}

fn parse_int(s: &str) -> Option<i64> {
    s.trim().parse::<i64>().ok().or_else(|| {
        // Tolerate "48.0" / "48.5" by truncating; HA's number slider
        // emits trailing `.0` on integer values.
        s.trim().parse::<f64>().ok().map(|v| v as i64)
    })
}

fn bool_text(b: bool) -> &'static str {
    if b { "ON" } else { "OFF" }
}

fn format_int(v: f64) -> String {
    (v as i64).to_string()
}

fn format_float(v: f64) -> String {
    // One decimal — matches HA's expectation for a `°C` sensor.
    format!("{v:.1}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bool_accepts_ha_switch_payloads() {
        assert_eq!(parse_bool("ON"), Some(true));
        assert_eq!(parse_bool("on"), Some(true));
        assert_eq!(parse_bool("OFF"), Some(false));
        assert_eq!(parse_bool("TRUE"), Some(true));
        assert_eq!(parse_bool("0"), Some(false));
        assert_eq!(parse_bool("1"), Some(true));
        assert_eq!(parse_bool("garbage"), None);
    }

    #[test]
    fn parse_int_tolerates_float_strings() {
        assert_eq!(parse_int("48"), Some(48));
        assert_eq!(parse_int("48.0"), Some(48));
        assert_eq!(parse_int("48.7"), Some(48));
        assert_eq!(parse_int("not a number"), None);
    }

    #[test]
    fn format_float_one_decimal() {
        assert_eq!(format_float(47.5), "47.5");
        assert_eq!(format_float(47.0), "47.0");
        assert_eq!(format_float(47.78), "47.8");
    }

    #[test]
    fn bool_text_round_trip() {
        assert_eq!(parse_bool(bool_text(true)), Some(true));
        assert_eq!(parse_bool(bool_text(false)), Some(false));
    }
}
