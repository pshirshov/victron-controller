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
pub use serialize::{decode_knob_set, encode_publish_payload};

use std::time::Duration;

use anyhow::{Context, Result};
use rumqttc::{AsyncClient, Event as MqttEvent, EventLoop, MqttOptions, Packet, QoS};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use victron_controller_core::types::{Event, PublishPayload};

use crate::config::MqttConfig;

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
    /// Subscribe to command topics, then drive the event loop forever,
    /// forwarding Command events into `tx`.
    pub async fn run(mut self, tx: mpsc::Sender<Event>) -> Result<()> {
        let knob_topic = format!("{}/knob/+/set", self.topic_root);
        let kill_topic = format!("{}/writes_enabled/set", self.topic_root);
        self.client
            .subscribe(&knob_topic, QoS::AtLeastOnce)
            .await
            .with_context(|| format!("subscribe {knob_topic}"))?;
        self.client
            .subscribe(&kill_topic, QoS::AtLeastOnce)
            .await
            .with_context(|| format!("subscribe {kill_topic}"))?;
        info!(%knob_topic, %kill_topic, "mqtt subscribed");

        loop {
            let ev = match self.event_loop.poll().await {
                Ok(e) => e,
                Err(e) => {
                    warn!(error = %e, "mqtt event loop error; reconnecting in 5s");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };
            let MqttEvent::Incoming(Packet::Publish(publish)) = ev else {
                continue;
            };
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
    }
}
