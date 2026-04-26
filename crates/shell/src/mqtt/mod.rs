//! MQTT publisher + subscriber.
//!
//! Three roles:
//!
//! - **Publisher** (`Publisher`): executes `Effect::Publish(...)` effects
//!   from the runtime and translates them into retained MQTT messages
//!   under `victron-controller/…`.
//! - **Subscriber** (`Subscriber`): listens on `victron-controller/knob/+/set`
//!   and `victron-controller/writes_enabled/set`, translates incoming
//!   messages into core `Event::Command`s and feeds them into the event
//!   channel.
//! - **Discovery** (`publish_ha_discovery`): one-shot at startup —
//!   publishes Home-Assistant MQTT-discovery config messages so HA
//!   sees the knob entities and derived sensors natively.
//!
//! All three share a single [`rumqttc::AsyncClient`] so MQTT connection
//! management (reconnects, keepalive, persistent session) happens in
//! one place.

mod discovery;
mod log_layer;
mod serialize;

pub use discovery::publish_ha_discovery;
pub use log_layer::{log_channel, spawn_log_publisher, LogRecord, MqttLogLayer};
pub use serialize::{
    decode_knob_set, decode_state_message, encode_publish_payload, matter_outdoor_temp_event,
    parse_matter_outdoor_temp, set_hardware_params, MatterOutdoorTempParse,
};

// PR-ev-soc-sensor: parsers re-exported for unit testing and any
// future caller that needs to parse the same wire formats out-of-band.
// `parse_discovery_state_topic` and `parse_ev_soc_state_value` are
// defined in this module (not `serialize`) because they're only used
// by the subscriber's inbound dispatch path and have no symmetric
// `encode_*` counterpart.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rumqttc::{
    AsyncClient, Event as MqttEvent, EventLoop, MqttOptions, Packet, QoS, TlsConfiguration,
    Transport,
};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use victron_controller_core::owner::Owner;
use victron_controller_core::types::{
    Command, Event, KnobId, KnobValue, PublishPayload, SensorId, SensorReading, TimerId,
    TimerStatus,
};

use crate::config::{EvConfig, MqttConfig, OutdoorTemperatureLocalConfig};
use crate::dashboard::SocHistoryStore;
use std::sync::Arc;

/// How long the bootstrap phase waits for retained `/state` messages
/// before switching to the normal subscription pattern.
const BOOTSTRAP_WINDOW: Duration = Duration::from_secs(2);

/// Wraps the async MQTT client + its event loop. Cheap to clone — the
/// client handle is an `Arc` internally.
#[derive(Debug, Clone)]
pub struct Publisher {
    client: AsyncClient,
    topic_root: String,
}

impl Publisher {
    /// Borrow the underlying MQTT client — used e.g. to publish
    /// one-shot HA discovery config at startup.
    #[must_use]
    pub fn client_handle(&self) -> AsyncClient {
        self.client.clone()
    }

    pub async fn publish(&self, payload: PublishPayload) {
        let Some((subtopic, body, retain)) = serialize::encode_publish_payload(&payload) else {
            debug!(?payload, "no mqtt encoding for payload");
            return;
        };
        let topic = format!("{}/{}", self.topic_root, subtopic);
        if let Err(e) = self
            .client
            .publish(&topic, QoS::AtLeastOnce, retain, body.as_bytes())
            .await
        {
            warn!(%topic, error = %e, "mqtt publish failed");
        }
    }
}

/// Connect to the broker and spawn the rumqttc event-loop task. Returns
/// a [`Publisher`] the runtime can clone + share, and an
/// [`Subscriber`] that wraps the `EventLoop` so someone drives it.
///
/// `outdoor_temp` (optional) wires a Matter MQTT bridge feeding
/// `SensorId::OutdoorTemperature` — see PR-matter-outdoor-temp.
///
/// `soc_history` is the in-memory ring that gets seeded from the
/// retained `<topic_root>/state/soc_history` payload during bootstrap
/// (PR-soc-history-persist).
#[allow(clippy::unused_async)]
pub async fn connect(
    config: &MqttConfig,
    outdoor_temp: &OutdoorTemperatureLocalConfig,
    ev: &EvConfig,
    soc_history: Arc<SocHistoryStore>,
) -> Result<Option<(Publisher, Subscriber)>> {
    if config.host.is_empty() {
        info!("mqtt disabled (no host configured)");
        return Ok(None);
    }

    let mut opts = MqttOptions::new(
        format!("victron-controller-{}", rand_suffix()),
        &config.host,
        config.port,
    );
    opts.set_keep_alive(Duration::from_secs(30));
    opts.set_clean_session(false);
    if let (Some(u), Some(p)) = (&config.username, &config.password) {
        opts.set_credentials(u, p);
    }
    if config.tls {
        let ca_path = config.ca_path.as_deref().ok_or_else(|| {
            anyhow::anyhow!("mqtt.tls=true requires mqtt.ca_path to point at a CA certificate")
        })?;
        let ca = std::fs::read(ca_path)
            .with_context(|| format!("read CA certificate from {ca_path}"))?;
        // A-68: validate CA bytes at parse time instead of waiting for
        // the first TLS handshake to fail cryptically. rumqttc's
        // TlsConfiguration::Simple accepts whatever bytes we hand it;
        // a typo in the config path pointing at a random file would
        // silently "configure TLS" and then fail at connect with an
        // opaque rustls error. We do a cheap PEM prefix check here —
        // not a full X.509 parse, but enough to catch "wrong file".
        let looks_like_pem = ca
            .windows(b"-----BEGIN CERTIFICATE-----".len())
            .any(|w| w == b"-----BEGIN CERTIFICATE-----");
        if !looks_like_pem {
            return Err(anyhow::anyhow!(
                "mqtt.ca_path {ca_path} does not contain a PEM-encoded \
                 certificate (missing '-----BEGIN CERTIFICATE-----' marker)"
            ));
        }
        opts.set_transport(Transport::Tls(TlsConfiguration::Simple {
            ca,
            alpn: None,
            client_auth: None,
        }));
        info!(%ca_path, "mqtt TLS enabled");
    }

    // 4096-slot request queue — sized to absorb startup HA discovery + retained bootstrap + observer-mode Publish(ActuatedPhase) bursts without backpressuring the runtime dispatch loop.
    let (client, event_loop) = AsyncClient::new(opts, 4096);
    // A-38: wording previously read "mqtt connected" but at this point
    // we've only constructed the rumqttc AsyncClient — no TCP handshake,
    // no CONNACK. "mqtt client configured" is the honest description.
    // The actual connect confirmation arrives asynchronously via the
    // EventLoop's `Event::Incoming(Packet::ConnAck)`; subscribers that
    // care log from there.
    info!(host = %config.host, port = config.port, "mqtt client configured (actual connect fires on first event-loop iteration)");
    let publisher = Publisher {
        client: client.clone(),
        topic_root: config.topic_root.clone(),
    };
    // Defensive validation: MQTT topics must not contain whitespace.
    // A copy-paste artefact ("LSJW74098PZ09  2927_mg" with two spaces)
    // silently broke the EV SoC subscription in the field — we
    // subscribed to a topic the broker had nothing on, never received
    // a discovery payload, and the sensor stayed Unknown forever
    // without any visible warning. Treat whitespace-bearing topics as
    // misconfiguration: warn loud at startup and refuse to subscribe.
    let ev_soc_topic_validated = validate_topic(ev.soc_topic.as_deref(), "ev.soc_topic");
    let ev_charge_target_topic_validated =
        validate_topic(ev.charge_target_topic.as_deref(), "ev.charge_target_topic");

    let subscriber = Subscriber {
        client,
        event_loop,
        topic_root: config.topic_root.clone(),
        matter_outdoor_topic: outdoor_temp.mqtt_topic.clone(),
        matter_outdoor_min_c: outdoor_temp.min_celsius,
        matter_outdoor_max_c: outdoor_temp.max_celsius,
        soc_history,
        ev_soc_discovery_topic: ev_soc_topic_validated,
        ev_soc_state_topic: None,
        ev_soc_value_field: None,
        ev_soc_last_parse_warn: None,
        ev_charge_target_discovery_topic: ev_charge_target_topic_validated,
        ev_charge_target_state_topic: None,
        ev_charge_target_value_field: None,
        ev_charge_target_last_parse_warn: None,
    };
    Ok(Some((publisher, subscriber)))
}

/// Reject MQTT topics containing whitespace (or empty strings, after
/// trimming). Returns `Some(topic)` when valid, `None` when missing
/// or malformed. Logs a `warn!` on rejection so the operator sees it
/// in the boot log instead of silently subscribing to a topic the
/// broker has nothing on.
#[must_use]
fn validate_topic(topic: Option<&str>, field_name: &str) -> Option<String> {
    let raw = topic?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        warn!(field = %field_name, "mqtt: topic is empty; bridge dormant");
        return None;
    }
    if trimmed.chars().any(char::is_whitespace) {
        warn!(
            field = %field_name,
            topic = %raw,
            "mqtt: topic contains whitespace (likely a copy-paste artefact); bridge dormant. \
             Fix the value in config.toml and restart.",
        );
        return None;
    }
    Some(trimmed.to_string())
}

// -----------------------------------------------------------------------------
// PR-ev-soc-sensor — parsers shared with unit tests
// -----------------------------------------------------------------------------

/// HA-discovery extract: state_topic + optional value_template field.
/// Some publishers (saic-python-mqtt-gateway for the SoC entity, but
/// not for `target_soc` which is plain numeric) emit JSON state bodies
/// and rely on a Jinja-style template at the discovery level to pluck
/// the value out. We don't run a full Jinja interpreter — we only
/// recognise the common shape `{{ value_json.<field> }}` (and HA's
/// short-form abbreviations `stat_t` / `val_tpl`).
#[derive(Debug, Clone, PartialEq)]
pub struct EvDiscovery {
    pub state_topic: String,
    /// JSON pointer-ish field name extracted from a
    /// `{{ value_json.<field> }}` template. None when no template, or
    /// when the template wasn't a recognisable shape.
    pub value_field: Option<String>,
}

#[must_use]
pub fn parse_discovery(payload: &[u8]) -> Option<EvDiscovery> {
    let v: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let obj = v.as_object()?;
    let state_topic = obj
        .get("state_topic")
        .or_else(|| obj.get("stat_t"))
        .and_then(|s| s.as_str())?
        .to_string();
    let value_field = obj
        .get("value_template")
        .or_else(|| obj.get("val_tpl"))
        .and_then(|s| s.as_str())
        .and_then(parse_value_template_field);
    Some(EvDiscovery { state_topic, value_field })
}

/// Backwards-compat alias used by tests + a few callers that don't
/// care about the template field.
#[must_use]
pub fn parse_discovery_state_topic(payload: &[u8]) -> Option<String> {
    parse_discovery(payload).map(|d| d.state_topic)
}

/// Match `{{ value_json.<field> }}` (and a tolerant tail). Returns the
/// `<field>` name. Bracket forms (`value_json["x"]`) are also accepted.
fn parse_value_template_field(template: &str) -> Option<String> {
    // Trim whitespace and the {{ }} delimiters.
    let t = template.trim();
    let inner = t.strip_prefix("{{")?.strip_suffix("}}")?.trim();
    // Match leading "value_json".
    let rest = inner.strip_prefix("value_json")?.trim_start();
    // Either ".field" or "['field']" / "[\"field\"]".
    if let Some(after_dot) = rest.strip_prefix('.') {
        let field: String = after_dot
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if field.is_empty() { None } else { Some(field) }
    } else if let Some(after_brk) = rest.strip_prefix('[') {
        let after_brk = after_brk.trim_start();
        let after_brk = after_brk.strip_prefix('\'').or_else(|| after_brk.strip_prefix('"'))?;
        let field: String = after_brk
            .chars()
            .take_while(|c| *c != '\'' && *c != '"')
            .collect();
        if field.is_empty() { None } else { Some(field) }
    } else {
        None
    }
}

/// Parse an EV SoC state-topic body. Accepts:
///   * plain decimal like `"42.5"`
///   * JSON object with the named `value_field` (extracted from the
///     discovery's `value_template`) — e.g. body `{"value": 42.5,
///     "timestamp": ...}` with `value_field = Some("value")` →
///     `Some(42.5)`.
///   * JSON object falling back to a `value` or `state` key when no
///     template was provided — covers the common HA convention.
///
/// Rejects:
///   * non-UTF-8 bodies,
///   * unparseable / non-finite floats,
///   * values outside the inclusive `[0.0, 100.0]` percentage range.
///
/// Returns `Some(value)` only on a clean, in-range reading.
#[must_use]
pub fn parse_ev_soc_state_value(payload: &[u8]) -> Option<f64> {
    parse_ev_soc_state_value_with_field(payload, None)
}

#[must_use]
pub fn parse_ev_soc_state_value_with_field(
    payload: &[u8],
    value_field: Option<&str>,
) -> Option<f64> {
    let s = std::str::from_utf8(payload).ok()?.trim();
    // Plain-number first: cheapest, covers the target_soc case.
    if let Ok(v) = s.parse::<f64>() {
        if v.is_finite() && (0.0..=100.0).contains(&v) {
            return Some(v);
        }
    }
    // JSON fallback. Try the templated field, else common defaults.
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    let try_key = |k: &str| -> Option<f64> {
        v.get(k).and_then(|x| x.as_f64()).filter(|n| n.is_finite() && (0.0..=100.0).contains(n))
    };
    if let Some(f) = value_field {
        return try_key(f);
    }
    try_key("value").or_else(|| try_key("state"))
}

/// PR-timers-section: wall-clock epoch-ms helper used for the one-shot
/// MqttBootstrap timer emit.
fn epoch_ms_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| {
            i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
        })
}

/// UUID-v4 suffix for MQTT clientId.
///
/// A-52: the prior PID⊕ns implementation had only 32 bits of entropy
/// and was deterministic given the start-time, so two controllers
/// restarted within a nanosecond of each other (or on hosts with
/// identical clocks, during a coordinated restart) could pick the
/// same clientId. Brokers with `clean_session=false` persistent
/// subscriptions treat a repeat clientId as a session-takeover and
/// kick the first connection offline — we saw this behavior in the
/// earlier field wedges before we had subscriber-reconnect. uuid::v4
/// gives 122 bits of entropy, collision probability effectively zero.
fn rand_suffix() -> String {
    // Strip dashes to keep the clientId compact; MQTT 3.1 limits
    // clientId to 23 chars for strict conformance and "victron-
    // controller-" prefix is 19 chars, leaving only 4. Brokers in
    // the field (Mosquitto, rumqttd, FlashMQ) all accept much longer
    // clientIds, so we send the full 32-hex-char v4.
    uuid::Uuid::new_v4().simple().to_string()
}

// -----------------------------------------------------------------------------
// Subscriber — owns the MQTT event loop, emits Commands to the runtime
// -----------------------------------------------------------------------------

pub struct Subscriber {
    client: AsyncClient,
    event_loop: EventLoop,
    topic_root: String,
    /// PR-matter-outdoor-temp: when `Some`, subscribe to this exact
    /// topic and feed its readings as `SensorId::OutdoorTemperature`.
    matter_outdoor_topic: Option<String>,
    matter_outdoor_min_c: f64,
    matter_outdoor_max_c: f64,
    /// PR-soc-history-persist: in-memory ring that gets restored from
    /// the retained `<topic_root>/state/soc_history` payload observed
    /// during the bootstrap window. Periodic re-publishes after
    /// bootstrap are ignored — we already have those samples in the
    /// deque locally.
    soc_history: Arc<SocHistoryStore>,
    /// PR-ev-soc-sensor: HA-discovery config topic for an external
    /// publisher (saic-python-mqtt-gateway today). When `Some`, the
    /// subscriber subscribes to the discovery topic, parses
    /// `state_topic` from the retained JSON, and subscribes to *that*
    /// for the actual SoC readings. `None` ⇒ the entire path is
    /// dormant.
    ev_soc_discovery_topic: Option<String>,
    /// PR-ev-soc-sensor: state_topic resolved from the most recent
    /// discovery payload. Carried in `Subscriber` (instead of the run-
    /// loop locals) so re-issuing subscriptions after a connection drop
    /// can refer to it.
    ev_soc_state_topic: Option<String>,
    /// PR-ev-soc-template-fix: optional value_template field name
    /// extracted from the discovery payload. When `Some(field)`, the
    /// state body is parsed as JSON and the value at `field` is used
    /// (covers saic-python-mqtt-gateway's JSON SoC body shape).
    ev_soc_value_field: Option<String>,
    /// PR-ev-soc-sensor: monotonic timestamp of the last
    /// `debug!`-logged value-parse failure on the EV SoC state topic.
    /// Used to rate-limit the noisy "garbage body" warnings to one
    /// emission per 60 s, mirroring the matter-outdoor-temp pattern.
    ev_soc_last_parse_warn: Option<Instant>,
    /// PR-auto-extended-charge: HA-discovery config topic for the EV's
    /// configured charge-target SoC. Same gateway as
    /// `ev_soc_discovery_topic`. `None` ⇒ dormant.
    ev_charge_target_discovery_topic: Option<String>,
    /// PR-auto-extended-charge: state_topic resolved from the most
    /// recent charge-target discovery payload.
    ev_charge_target_state_topic: Option<String>,
    /// Mirrors `ev_soc_value_field` — value_template field for the
    /// charge_target entity, when present.
    ev_charge_target_value_field: Option<String>,
    /// PR-auto-extended-charge: rate-limit handle, mirrors
    /// `ev_soc_last_parse_warn`.
    ev_charge_target_last_parse_warn: Option<Instant>,
}

impl std::fmt::Debug for Subscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `EventLoop` and `AsyncClient` don't implement Debug; omit them.
        f.debug_struct("Subscriber")
            .field("client", &"<AsyncClient>")
            .field("event_loop", &"<EventLoop>")
            .field("topic_root", &self.topic_root)
            .field("matter_outdoor_topic", &self.matter_outdoor_topic)
            .field("matter_outdoor_min_c", &self.matter_outdoor_min_c)
            .field("matter_outdoor_max_c", &self.matter_outdoor_max_c)
            .field("soc_history", &"<SocHistoryStore>")
            .field("ev_soc_discovery_topic", &self.ev_soc_discovery_topic)
            .field("ev_soc_state_topic", &self.ev_soc_state_topic)
            .field("ev_soc_value_field", &self.ev_soc_value_field)
            .field("ev_soc_last_parse_warn", &self.ev_soc_last_parse_warn)
            .field(
                "ev_charge_target_discovery_topic",
                &self.ev_charge_target_discovery_topic,
            )
            .field(
                "ev_charge_target_state_topic",
                &self.ev_charge_target_state_topic,
            )
            .field(
                "ev_charge_target_value_field",
                &self.ev_charge_target_value_field,
            )
            .field(
                "ev_charge_target_last_parse_warn",
                &self.ev_charge_target_last_parse_warn,
            )
            .finish()
    }
}

impl Subscriber {
    /// Drive the MQTT subscriber across two phases and the main loop.
    ///
    /// **Phase 1 (bootstrap)**: subscribe to retained `/state` topics,
    /// drain the event loop for [`BOOTSTRAP_WINDOW`], forward every
    /// parsed retained message as a System-owned `Event::Command` so
    /// the runtime seeds its knobs from MQTT instead of hard-coded
    /// safe-defaults.
    ///
    /// **Phase 2**: unsubscribe from `/state`, subscribe to `/set`,
    /// and loop forever — each inbound message becomes an HaMqtt-
    /// owned `Event::Command`. On connection error (network drop,
    /// broker restart) we re-subscribe from scratch rather than
    /// relying on rumqttc to replay subscriptions.
    pub async fn run(mut self, tx: mpsc::Sender<Event>) -> Result<()> {
        let soc_history_topic = format!("{}/state/soc_history", self.topic_root);
        let state_topics = [
            format!("{}/knob/+/state", self.topic_root),
            format!("{}/writes_enabled/state", self.topic_root),
            format!("{}/bookkeeping/+/state", self.topic_root),
            soc_history_topic.clone(),
        ];
        let set_topics = [
            format!("{}/knob/+/set", self.topic_root),
            format!("{}/writes_enabled/set", self.topic_root),
        ];

        // A-67: queue the AllowBatteryToCar=false reset BEFORE the
        // bootstrap event loop. SPEC §5.9 says "always boots false
        // regardless of retained". The post-bootstrap override below
        // covers the normal path, but if bootstrap crashes mid-way
        // (event-loop error, deserialize panic), the override never
        // fires and the runtime inherits whatever the retained
        // (possibly-true) value set. Queuing the reset first means
        // the runtime's first knob-update event is false; a later
        // retained-true message applies temporarily, then the
        // post-bootstrap override re-forces false. Belt-and-suspenders.
        let _ = tx
            .send(Event::Command {
                command: Command::Knob {
                    id: KnobId::AllowBatteryToCar,
                    value: KnobValue::Bool(false),
                },
                owner: Owner::System,
                at: Instant::now(),
            })
            .await;

        // Phase 1: bootstrap ---------------------------------------------------
        for t in &state_topics {
            self.client
                .subscribe(t, QoS::AtLeastOnce)
                .await
                .with_context(|| format!("bootstrap subscribe {t}"))?;
        }
        info!("mqtt bootstrap: collecting retained /state for {:?}", BOOTSTRAP_WINDOW);
        let deadline = Instant::now() + BOOTSTRAP_WINDOW;
        let mut applied = 0_usize;
        let mut applied_topics: HashSet<String> = HashSet::new();
        let mut duplicate_count = 0_usize;
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match timeout(remaining, self.event_loop.poll()).await {
                Ok(Ok(MqttEvent::Incoming(Packet::Publish(p)))) => {
                    // Skip a duplicate retained delivery within the bootstrap
                    // window. Broker/rumqttc redelivery amplification (A-71)
                    // can produce ~50× duplicates per retained topic. Our
                    // canonical retained value is one-per-topic by definition,
                    // so first-observed wins; subsequent copies are wasteful.
                    if !applied_topics.insert(p.topic.clone()) {
                        duplicate_count += 1;
                        continue;
                    }
                    // PR-soc-history-persist: the retained SoC-history
                    // payload is restored directly into the in-memory
                    // store; it does not flow through the runtime as
                    // an Event::Command.
                    if p.topic == soc_history_topic {
                        match std::str::from_utf8(&p.payload) {
                            Ok(s) => {
                                let now_ms = epoch_ms_now();
                                match self.soc_history.restore_from_wire(s, now_ms) {
                                    Some(n) => {
                                        info!(
                                            accepted = n,
                                            "soc_history restored from retained mqtt"
                                        );
                                        applied += 1;
                                    }
                                    None => {
                                        // restore_from_wire already
                                        // logged a warn; nothing to do.
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(
                                    error = %e,
                                    "soc_history retained payload is not valid utf-8; dropped",
                                );
                            }
                        }
                        continue;
                    }
                    if let Some(event) = serialize::decode_state_message(
                        &self.topic_root,
                        &p.topic,
                        &p.payload,
                    ) {
                        if tx.send(event).await.is_err() {
                            return Ok(());
                        }
                        applied += 1;
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    warn!(error = %e, "bootstrap event loop error; aborting bootstrap");
                    break;
                }
                Err(_) => break, // deadline reached
            }
        }
        info!(
            applied,
            unique_topics = applied_topics.len(),
            duplicates_suppressed = duplicate_count,
            "mqtt bootstrap complete; seeded knobs from retained state"
        );

        // Unconditional post-bootstrap override: SPEC §5.9 requires
        // `allow_battery_to_car` to boot `false` every single time,
        // regardless of retained value. This fires AFTER the bootstrap
        // in case the retained value was `true`.
        let _ = tx
            .send(Event::Command {
                command: Command::Knob {
                    id: KnobId::AllowBatteryToCar,
                    value: KnobValue::Bool(false),
                },
                owner: Owner::System,
                at: Instant::now(),
            })
            .await;

        // PR-timers-section: signal the one-shot MqttBootstrap timer
        // completion so the dashboard's timers section can reflect it.
        // No `next_fire` — the bootstrap only runs once per process.
        let _ = tx
            .send(Event::TimerState {
                id: TimerId::MqttBootstrap,
                last_fire_epoch_ms: epoch_ms_now(),
                next_fire_epoch_ms: None,
                status: TimerStatus::Idle,
                at: Instant::now(),
            })
            .await;

        for t in &state_topics {
            let _ = self.client.unsubscribe(t).await;
        }

        // Phase 2: main loop ---------------------------------------------------
        self.subscribe_set_topics(&set_topics).await?;
        self.subscribe_matter_outdoor().await;
        // PR-ev-soc-sensor: subscribe to the publisher's HA-discovery
        // config topic. The retained discovery payload arrives shortly
        // after subscribe; the inbound dispatch below resolves
        // `state_topic` from it and chains a second subscribe.
        self.subscribe_ev_soc_discovery().await;
        if let Some(state_topic) = self.ev_soc_state_topic.clone() {
            // Already-known state topic from a previous run cycle (e.g.
            // post-reconnect). Re-issue the subscribe so the broker
            // re-delivers retained values.
            self.subscribe_ev_soc_state(&state_topic).await;
        }
        // PR-auto-extended-charge: same two-stage pattern for the
        // charge-target topic.
        self.subscribe_ev_charge_target_discovery().await;
        if let Some(state_topic) = self.ev_charge_target_state_topic.clone() {
            self.subscribe_ev_charge_target_state(&state_topic).await;
        }

        // PR-matter-outdoor-temp: rate-limit the out-of-range warn to
        // once per 60 s. Genuine sensor failures stay visible without
        // flooding the log if a stuck sensor publishes every minute.
        let mut last_oor_warn: Option<Instant> = None;
        let oor_warn_period = Duration::from_secs(60);

        loop {
            let ev = match self.event_loop.poll().await {
                Ok(e) => e,
                Err(e) => {
                    warn!(error = %e, "mqtt event loop error; waiting 5s before re-subscribing");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    // rumqttc reconnects automatically, but doesn't replay
                    // subscriptions — so re-issue them.
                    if let Err(e2) = self.subscribe_set_topics(&set_topics).await {
                        warn!(error = %e2, "re-subscribe failed; continuing");
                    }
                    self.subscribe_matter_outdoor().await;
                    self.subscribe_ev_soc_discovery().await;
                    if let Some(t) = self.ev_soc_state_topic.clone() {
                        self.subscribe_ev_soc_state(&t).await;
                    }
                    self.subscribe_ev_charge_target_discovery().await;
                    if let Some(t) = self.ev_charge_target_state_topic.clone() {
                        self.subscribe_ev_charge_target_state(&t).await;
                    }
                    continue;
                }
            };
            match ev {
                MqttEvent::Incoming(Packet::ConnAck(_)) => {
                    debug!("mqtt ConnAck — re-subscribing");
                    if let Err(e) = self.subscribe_set_topics(&set_topics).await {
                        warn!(error = %e, "re-subscribe after ConnAck failed");
                    }
                    self.subscribe_matter_outdoor().await;
                    self.subscribe_ev_soc_discovery().await;
                    if let Some(t) = self.ev_soc_state_topic.clone() {
                        self.subscribe_ev_soc_state(&t).await;
                    }
                    self.subscribe_ev_charge_target_discovery().await;
                    if let Some(t) = self.ev_charge_target_state_topic.clone() {
                        self.subscribe_ev_charge_target_state(&t).await;
                    }
                }
                MqttEvent::Incoming(Packet::Publish(publish)) => {
                    // PR-matter-outdoor-temp: exact-match against the
                    // configured Matter outdoor-temperature topic before
                    // falling through to the knob/set decoder.
                    if let Some(topic) = self.matter_outdoor_topic.as_deref() {
                        if publish.topic == topic {
                            match serialize::parse_matter_outdoor_temp(
                                &publish.payload,
                                self.matter_outdoor_min_c,
                                self.matter_outdoor_max_c,
                            ) {
                                MatterOutdoorTempParse::Reading(c) => {
                                    let event = serialize::matter_outdoor_temp_event(
                                        c,
                                        Instant::now(),
                                    );
                                    if tx.send(event).await.is_err() {
                                        info!(
                                            "runtime receiver closed; mqtt subscriber exiting"
                                        );
                                        return Ok(());
                                    }
                                }
                                MatterOutdoorTempParse::Drop => {
                                    debug!(
                                        topic = %publish.topic,
                                        "matter outdoor temp body dropped (null/non-numeric/out-of-int16)"
                                    );
                                }
                                MatterOutdoorTempParse::OutOfRange(c) => {
                                    let now = Instant::now();
                                    let should_warn = last_oor_warn
                                        .is_none_or(|t| now.duration_since(t) >= oor_warn_period);
                                    if should_warn {
                                        warn!(
                                            celsius = c,
                                            min = self.matter_outdoor_min_c,
                                            max = self.matter_outdoor_max_c,
                                            topic = %publish.topic,
                                            "matter outdoor temp out of sanity range; dropped (rate-limited 60s)"
                                        );
                                        last_oor_warn = Some(now);
                                    }
                                }
                            }
                            continue;
                        }
                    }
                    // PR-ev-soc-sensor: discovery + state topic dispatch.
                    // Discovery payload is retained; the gateway may also
                    // republish it on its own restart with a new
                    // state_topic — handle that case by swapping the
                    // subscription.
                    if self
                        .ev_soc_discovery_topic
                        .as_deref()
                        .is_some_and(|t| t == publish.topic)
                    {
                        match parse_discovery(&publish.payload) {
                            Some(disco) => {
                                let new_state_topic = disco.state_topic;
                                let prev = self.ev_soc_state_topic.clone();
                                if prev.as_deref() != Some(new_state_topic.as_str()) {
                                    if let Some(old) = prev.as_deref() {
                                        if let Err(e) = self.client.unsubscribe(old).await {
                                            warn!(
                                                error = %e,
                                                old_topic = %old,
                                                "ev_soc: unsubscribe old state_topic failed"
                                            );
                                        }
                                        info!(
                                            old_topic = %old,
                                            new_topic = %new_state_topic,
                                            value_field = ?disco.value_field,
                                            "ev_soc: state_topic changed; re-subscribing",
                                        );
                                    } else {
                                        info!(
                                            new_topic = %new_state_topic,
                                            value_field = ?disco.value_field,
                                            "ev_soc: state_topic resolved from discovery",
                                        );
                                    }
                                    self.ev_soc_state_topic = Some(new_state_topic.clone());
                                    self.subscribe_ev_soc_state(&new_state_topic).await;
                                }
                                self.ev_soc_value_field = disco.value_field;
                            }
                            None => {
                                warn!(
                                    topic = %publish.topic,
                                    "ev_soc: discovery payload missing string `state_topic`; \
                                     no state subscription"
                                );
                            }
                        }
                        continue;
                    }
                    if self
                        .ev_soc_state_topic
                        .as_deref()
                        .is_some_and(|t| t == publish.topic)
                    {
                        match parse_ev_soc_state_value_with_field(
                            &publish.payload,
                            self.ev_soc_value_field.as_deref(),
                        ) {
                            Some(v) => {
                                let event = Event::Sensor(SensorReading {
                                    id: SensorId::EvSoc,
                                    value: v,
                                    at: Instant::now(),
                                });
                                // try_send drops on full instead of
                                // blocking the dispatch loop — we'd
                                // rather lose a single SoC sample than
                                // wedge the whole subscriber.
                                if let Err(e) = tx.try_send(event) {
                                    use tokio::sync::mpsc::error::TrySendError;
                                    match e {
                                        TrySendError::Full(_) => {
                                            warn!(
                                                "ev_soc: event channel full; dropped reading"
                                            );
                                        }
                                        TrySendError::Closed(_) => {
                                            info!(
                                                "runtime receiver closed; mqtt subscriber exiting"
                                            );
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                            None => {
                                let now = Instant::now();
                                let should_warn = self
                                    .ev_soc_last_parse_warn
                                    .is_none_or(|t| {
                                        now.duration_since(t) >= Duration::from_secs(60)
                                    });
                                if should_warn {
                                    let body_preview = match std::str::from_utf8(&publish.payload) {
                                        Ok(s) => {
                                            let s = s.trim();
                                            if s.len() > 80 {
                                                format!("{}…", &s[..80])
                                            } else {
                                                s.to_string()
                                            }
                                        }
                                        Err(_) => "<non-utf8>".to_string(),
                                    };
                                    warn!(
                                        topic = %publish.topic,
                                        body_len = publish.payload.len(),
                                        value_field = ?self.ev_soc_value_field,
                                        body_preview = %body_preview,
                                        "ev_soc: state body unparseable / out of range; \
                                         dropped (rate-limited 60s)"
                                    );
                                    self.ev_soc_last_parse_warn = Some(now);
                                }
                            }
                        }
                        continue;
                    }
                    // PR-auto-extended-charge: same two-stage discovery
                    // + state dispatch for the charge-target topic.
                    if self
                        .ev_charge_target_discovery_topic
                        .as_deref()
                        .is_some_and(|t| t == publish.topic)
                    {
                        match parse_discovery(&publish.payload) {
                            Some(disco) => {
                                let new_state_topic = disco.state_topic;
                                let prev = self.ev_charge_target_state_topic.clone();
                                if prev.as_deref() != Some(new_state_topic.as_str()) {
                                    if let Some(old) = prev.as_deref() {
                                        if let Err(e) = self.client.unsubscribe(old).await {
                                            warn!(
                                                error = %e,
                                                old_topic = %old,
                                                "ev_charge_target: unsubscribe old state_topic failed"
                                            );
                                        }
                                        info!(
                                            old_topic = %old,
                                            new_topic = %new_state_topic,
                                            value_field = ?disco.value_field,
                                            "ev_charge_target: state_topic changed; re-subscribing",
                                        );
                                    } else {
                                        info!(
                                            new_topic = %new_state_topic,
                                            value_field = ?disco.value_field,
                                            "ev_charge_target: state_topic resolved from discovery",
                                        );
                                    }
                                    self.ev_charge_target_state_topic =
                                        Some(new_state_topic.clone());
                                    self.subscribe_ev_charge_target_state(&new_state_topic).await;
                                }
                                self.ev_charge_target_value_field = disco.value_field;
                            }
                            None => {
                                warn!(
                                    topic = %publish.topic,
                                    "ev_charge_target: discovery payload missing string \
                                     `state_topic`; no state subscription"
                                );
                            }
                        }
                        continue;
                    }
                    if self
                        .ev_charge_target_state_topic
                        .as_deref()
                        .is_some_and(|t| t == publish.topic)
                    {
                        match parse_ev_soc_state_value_with_field(
                            &publish.payload,
                            self.ev_charge_target_value_field.as_deref(),
                        ) {
                            Some(v) => {
                                let event = Event::Sensor(SensorReading {
                                    id: SensorId::EvChargeTarget,
                                    value: v,
                                    at: Instant::now(),
                                });
                                if let Err(e) = tx.try_send(event) {
                                    use tokio::sync::mpsc::error::TrySendError;
                                    match e {
                                        TrySendError::Full(_) => {
                                            warn!(
                                                "ev_charge_target: event channel full; dropped reading"
                                            );
                                        }
                                        TrySendError::Closed(_) => {
                                            info!(
                                                "runtime receiver closed; mqtt subscriber exiting"
                                            );
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                            None => {
                                let now = Instant::now();
                                let should_warn = self
                                    .ev_charge_target_last_parse_warn
                                    .is_none_or(|t| {
                                        now.duration_since(t) >= Duration::from_secs(60)
                                    });
                                if should_warn {
                                    let body_preview = match std::str::from_utf8(&publish.payload) {
                                        Ok(s) => {
                                            let s = s.trim();
                                            if s.len() > 80 {
                                                format!("{}…", &s[..80])
                                            } else {
                                                s.to_string()
                                            }
                                        }
                                        Err(_) => "<non-utf8>".to_string(),
                                    };
                                    warn!(
                                        topic = %publish.topic,
                                        body_len = publish.payload.len(),
                                        value_field = ?self.ev_charge_target_value_field,
                                        body_preview = %body_preview,
                                        "ev_charge_target: state body unparseable / out of range; \
                                         dropped (rate-limited 60s)"
                                    );
                                    self.ev_charge_target_last_parse_warn = Some(now);
                                }
                            }
                        }
                        continue;
                    }
                    if let Some(event) = serialize::decode_knob_set(
                        &self.topic_root,
                        &publish.topic,
                        &publish.payload,
                    ) {
                        if tx.send(event).await.is_err() {
                            info!("runtime receiver closed; mqtt subscriber exiting");
                            return Ok(());
                        }
                    } else {
                        debug!(topic = %publish.topic, "unrouted mqtt message");
                    }
                }
                _ => {}
            }
        }
    }

    async fn subscribe_set_topics(&self, topics: &[String]) -> Result<()> {
        for t in topics {
            self.client
                .subscribe(t, QoS::AtLeastOnce)
                .await
                .with_context(|| format!("subscribe {t}"))?;
        }
        info!(?topics, "mqtt /set topics subscribed");
        Ok(())
    }

    /// PR-matter-outdoor-temp: subscribe (exact, no glob) to the
    /// configured Matter outdoor-temperature topic, if any. Logged
    /// once per (re)subscribe so an operator can see the bridge is
    /// live; non-fatal on failure (the OM poller is the safety net).
    async fn subscribe_matter_outdoor(&self) {
        let Some(topic) = self.matter_outdoor_topic.as_deref() else {
            return;
        };
        match self.client.subscribe(topic, QoS::AtLeastOnce).await {
            Ok(()) => {
                info!(%topic, "matter outdoor temperature MQTT bridge subscribed: {topic}");
            }
            Err(e) => {
                warn!(error = %e, %topic, "matter outdoor temperature MQTT subscribe failed");
            }
        }
    }

    /// PR-ev-soc-sensor: subscribe to the publisher's HA-discovery
    /// config topic (Stage A). Silent no-op when the bridge is
    /// unconfigured. Non-fatal on subscribe failure — the bridge is
    /// optional and the rest of the subscriber must keep running.
    async fn subscribe_ev_soc_discovery(&self) {
        let Some(topic) = self.ev_soc_discovery_topic.as_deref() else {
            return;
        };
        match self.client.subscribe(topic, QoS::AtLeastOnce).await {
            Ok(()) => {
                info!(%topic, "ev_soc discovery topic subscribed: {topic}");
            }
            Err(e) => {
                warn!(error = %e, %topic, "ev_soc discovery topic subscribe failed");
            }
        }
    }

    /// PR-ev-soc-sensor: subscribe to a resolved state topic (Stage B).
    async fn subscribe_ev_soc_state(&self, topic: &str) {
        match self.client.subscribe(topic, QoS::AtLeastOnce).await {
            Ok(()) => {
                info!(%topic, "ev_soc state topic subscribed: {topic}");
            }
            Err(e) => {
                warn!(error = %e, %topic, "ev_soc state topic subscribe failed");
            }
        }
    }

    /// PR-auto-extended-charge: subscribe to the publisher's HA-discovery
    /// config topic for the EV charge-target sensor.
    async fn subscribe_ev_charge_target_discovery(&self) {
        let Some(topic) = self.ev_charge_target_discovery_topic.as_deref() else {
            return;
        };
        match self.client.subscribe(topic, QoS::AtLeastOnce).await {
            Ok(()) => {
                info!(%topic, "ev_charge_target discovery topic subscribed: {topic}");
            }
            Err(e) => {
                warn!(error = %e, %topic, "ev_charge_target discovery topic subscribe failed");
            }
        }
    }

    /// PR-auto-extended-charge: subscribe to a resolved state topic.
    async fn subscribe_ev_charge_target_state(&self, topic: &str) {
        match self.client.subscribe(topic, QoS::AtLeastOnce).await {
            Ok(()) => {
                info!(%topic, "ev_charge_target state topic subscribed: {topic}");
            }
            Err(e) => {
                warn!(error = %e, %topic, "ev_charge_target state topic subscribe failed");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_discovery, parse_discovery_state_topic, parse_ev_soc_state_value,
        parse_ev_soc_state_value_with_field,
    };

    // ------------------------------------------------------------------
    // PR-ev-soc-sensor: discovery JSON → state_topic
    // ------------------------------------------------------------------

    #[test]
    fn parse_discovery_extracts_state_topic() {
        let body = br#"{
            "name": "MG SOC",
            "state_topic": "saic/LSJW/drivetrain/soc",
            "unit_of_measurement": "%",
            "device_class": "battery"
        }"#;
        assert_eq!(
            parse_discovery_state_topic(body).as_deref(),
            Some("saic/LSJW/drivetrain/soc"),
        );
    }

    #[test]
    fn parse_discovery_rejects_missing_state_topic() {
        let body = br#"{
            "name": "MG SOC",
            "unit_of_measurement": "%"
        }"#;
        assert!(parse_discovery_state_topic(body).is_none());
    }

    #[test]
    fn parse_discovery_rejects_non_string_state_topic() {
        let body = br#"{ "state_topic": 42 }"#;
        assert!(parse_discovery_state_topic(body).is_none());
    }

    #[test]
    fn parse_discovery_rejects_malformed_json() {
        assert!(parse_discovery_state_topic(b"not json").is_none());
        assert!(parse_discovery_state_topic(b"").is_none());
        assert!(parse_discovery_state_topic(b"{").is_none());
    }

    // ------------------------------------------------------------------
    // PR-ev-soc-sensor: state body → f64
    // ------------------------------------------------------------------

    #[test]
    fn parse_state_value_accepts_valid_float() {
        let v = parse_ev_soc_state_value(b"42.5").unwrap();
        assert!((v - 42.5).abs() < f64::EPSILON);
        // Trims surrounding whitespace, accepts integer-shaped bodies.
        assert!((parse_ev_soc_state_value(b" 0 ").unwrap() - 0.0).abs() < f64::EPSILON);
        assert!((parse_ev_soc_state_value(b"100").unwrap() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_state_value_rejects_nonfinite_or_out_of_range() {
        assert!(parse_ev_soc_state_value(b"NaN").is_none());
        assert!(parse_ev_soc_state_value(b"inf").is_none());
        assert!(parse_ev_soc_state_value(b"-inf").is_none());
        assert!(parse_ev_soc_state_value(b"-1").is_none());
        assert!(parse_ev_soc_state_value(b"100.1").is_none());
        assert!(parse_ev_soc_state_value(b"101").is_none());
    }

    #[test]
    fn parse_state_value_rejects_garbage() {
        assert!(parse_ev_soc_state_value(b"hello").is_none());
        assert!(parse_ev_soc_state_value(b"").is_none());
        assert!(parse_ev_soc_state_value(b"42.5%").is_none());
        // Non-UTF-8 bytes
        assert!(parse_ev_soc_state_value(&[0xff, 0xfe]).is_none());
    }

    // ------------------------------------------------------------------
    // PR-ev-soc-template-fix: HA value_template + JSON-body fallback.
    // ------------------------------------------------------------------

    #[test]
    fn parse_discovery_extracts_value_template_field() {
        let body = br#"{
            "state_topic": "saic/.../soc",
            "value_template": "{{ value_json.value }}"
        }"#;
        let d = parse_discovery(body).expect("ok");
        assert_eq!(d.state_topic, "saic/.../soc");
        assert_eq!(d.value_field.as_deref(), Some("value"));
    }

    #[test]
    fn parse_discovery_accepts_short_form_keys() {
        // HA's MQTT-discovery abbreviation list lets publishers use
        // `stat_t` / `val_tpl` instead of the long forms.
        let body = br#"{ "stat_t": "x/y/z", "val_tpl": "{{ value_json.soc }}" }"#;
        let d = parse_discovery(body).expect("ok");
        assert_eq!(d.state_topic, "x/y/z");
        assert_eq!(d.value_field.as_deref(), Some("soc"));
    }

    #[test]
    fn parse_discovery_handles_bracket_template_form() {
        let body = br#"{
            "state_topic": "x/y",
            "value_template": "{{ value_json['some_field'] }}"
        }"#;
        let d = parse_discovery(body).expect("ok");
        assert_eq!(d.value_field.as_deref(), Some("some_field"));
    }

    #[test]
    fn parse_discovery_no_template_returns_none_field() {
        let body = br#"{ "state_topic": "x/y" }"#;
        let d = parse_discovery(body).expect("ok");
        assert!(d.value_field.is_none());
    }

    #[test]
    fn parse_state_value_accepts_json_with_named_field() {
        let body = br#"{"value": 42.5, "timestamp": "2026-04-26T12:00:00Z"}"#;
        let v = parse_ev_soc_state_value_with_field(body, Some("value"));
        assert_eq!(v, Some(42.5));
    }

    #[test]
    fn parse_state_value_falls_back_to_json_value_or_state_when_no_template() {
        let body = br#"{"state": 88.0}"#;
        assert_eq!(parse_ev_soc_state_value(body), Some(88.0));
        let body = br#"{"value": 12.3}"#;
        assert_eq!(parse_ev_soc_state_value(body), Some(12.3));
    }

    #[test]
    fn parse_state_value_rejects_json_with_out_of_range_field() {
        let body = br#"{"value": 150.0}"#;
        assert!(parse_ev_soc_state_value_with_field(body, Some("value")).is_none());
    }

    #[test]
    fn parse_state_value_still_accepts_plain_number_when_template_present() {
        // Defensive: a publisher might emit plain numbers even when
        // the discovery declared a template. Plain number wins first
        // because it's the fast path; the field-named JSON branch is
        // only the fallback.
        let v = parse_ev_soc_state_value_with_field(b"50.0", Some("value"));
        assert_eq!(v, Some(50.0));
    }
}
