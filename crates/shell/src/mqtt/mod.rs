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
mod serialize;

pub use discovery::publish_ha_discovery;
pub use serialize::{decode_knob_set, decode_state_message, encode_publish_payload};

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rumqttc::{AsyncClient, Event as MqttEvent, EventLoop, MqttOptions, Packet, QoS};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, info, warn};

use victron_controller_core::types::{Event, PublishPayload};

use crate::config::MqttConfig;

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
#[allow(clippy::unused_async)]
pub async fn connect(config: &MqttConfig) -> Result<Option<(Publisher, Subscriber)>> {
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
    // TLS setup would go here via opts.set_transport(...). Deferred
    // until the user enables it in config.

    let (client, event_loop) = AsyncClient::new(opts, 64);
    info!(host = %config.host, port = config.port, "mqtt connected (session will establish on first loop iteration)");
    let publisher = Publisher {
        client: client.clone(),
        topic_root: config.topic_root.clone(),
    };
    let subscriber = Subscriber {
        client,
        event_loop,
        topic_root: config.topic_root.clone(),
    };
    Ok(Some((publisher, subscriber)))
}

/// Simple 8-char suffix derived from PID + ns-since-epoch so each run
/// of the binary has a distinct clientId.
fn rand_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = u128::from(std::process::id());
    #[allow(clippy::cast_possible_truncation)]
    let xored = (pid ^ n) as u32;
    format!("{xored:08x}")
}

// -----------------------------------------------------------------------------
// Subscriber — owns the MQTT event loop, emits Commands to the runtime
// -----------------------------------------------------------------------------

pub struct Subscriber {
    client: AsyncClient,
    event_loop: EventLoop,
    topic_root: String,
}

impl std::fmt::Debug for Subscriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `EventLoop` and `AsyncClient` don't implement Debug; omit them.
        f.debug_struct("Subscriber")
            .field("client", &"<AsyncClient>")
            .field("event_loop", &"<EventLoop>")
            .field("topic_root", &self.topic_root)
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
        ];
        let set_topics = [
            format!("{}/knob/+/set", self.topic_root),
            format!("{}/writes_enabled/set", self.topic_root),
        ];

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
        while Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match timeout(remaining, self.event_loop.poll()).await {
                Ok(Ok(MqttEvent::Incoming(Packet::Publish(p)))) => {
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
        info!(applied, "mqtt bootstrap complete; seeded knobs from retained state");

        for t in &state_topics {
            let _ = self.client.unsubscribe(t).await;
        }

        // Phase 2: main loop ---------------------------------------------------
        self.subscribe_set_topics(&set_topics).await?;

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
                    continue;
                }
            };
            match ev {
                MqttEvent::Incoming(Packet::ConnAck(_)) => {
                    debug!("mqtt ConnAck — re-subscribing");
                    if let Err(e) = self.subscribe_set_topics(&set_topics).await {
                        warn!(error = %e, "re-subscribe after ConnAck failed");
                    }
                }
                MqttEvent::Incoming(Packet::Publish(publish)) => {
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
}
