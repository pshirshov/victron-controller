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
    Command, Event, KnobId, KnobValue, PublishPayload, TimerId, TimerStatus,
};

use crate::config::{MqttConfig, OutdoorTemperatureLocalConfig};

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
#[allow(clippy::unused_async)]
pub async fn connect(
    config: &MqttConfig,
    outdoor_temp: &OutdoorTemperatureLocalConfig,
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
    let subscriber = Subscriber {
        client,
        event_loop,
        topic_root: config.topic_root.clone(),
        matter_outdoor_topic: outdoor_temp.mqtt_topic.clone(),
        matter_outdoor_min_c: outdoor_temp.min_celsius,
        matter_outdoor_max_c: outdoor_temp.max_celsius,
    };
    Ok(Some((publisher, subscriber)))
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
        let state_topics = [
            format!("{}/knob/+/state", self.topic_root),
            format!("{}/writes_enabled/state", self.topic_root),
            format!("{}/bookkeeping/+/state", self.topic_root),
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
}
