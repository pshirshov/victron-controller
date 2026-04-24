//! The shell's event loop. Owns the [`World`] and calls [`process`]
//! on every event. Effects are dispatched to their executors
//! (D-Bus writer for `WriteDbus`, future MQTT publisher for `Publish`,
//! etc).
//!
//! There is exactly ONE writer task per actuator bus (currently just
//! D-Bus); that task owns the bus connection and serialises writes,
//! so the core doesn't need to think about concurrency.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, trace, warn};

use victron_controller_core::types::{Effect, Event};
use victron_controller_core::{process, Topology, World};

use crate::clock::RealClock;
use crate::dashboard::SnapshotBroadcast;
use crate::dashboard::convert::world_to_snapshot;
use crate::dbus::Writer;
use crate::mqtt::Publisher as MqttPublisher;
use crate::dashboard::convert::MetaContext;
use crate::myenergi::Writer as MyenergiWriter;

#[derive(Debug)]
pub struct Runtime {
    world: Arc<Mutex<World>>,
    topology: Topology,
    clock: RealClock,
    writer: Writer,
    myenergi: MyenergiWriter,
    mqtt: Option<MqttPublisher>,
    snapshot_stream: Arc<SnapshotBroadcast>,
    meta: MetaContext,
}

impl Runtime {
    pub fn new(
        world: Arc<Mutex<World>>,
        writer: Writer,
        myenergi: MyenergiWriter,
        mqtt: Option<MqttPublisher>,
        topology: Topology,
        snapshot_stream: Arc<SnapshotBroadcast>,
        meta: MetaContext,
    ) -> Self {
        Self {
            world,
            topology,
            clock: RealClock,
            writer,
            myenergi,
            mqtt,
            snapshot_stream,
            meta,
        }
    }

    /// Consume events from `rx` until it closes. Sends a `Tick` every
    /// `tick_period` on its own timer so freshness decay and periodic
    /// controller re-evaluation keep running even without external
    /// events.
    pub async fn run(
        self,
        mut rx: mpsc::Receiver<Event>,
        tick_period: Duration,
    ) -> Result<()> {
        let mut tick = tokio::time::interval(tick_period);
        info!("runtime event loop started");
        loop {
            let event = tokio::select! {
                maybe_event = rx.recv() => match maybe_event {
                    Some(e) => e,
                    None => {
                        info!("event channel closed; shutting down runtime");
                        return Ok(());
                    }
                },
                _ = tick.tick() => Event::Tick { at: Instant::now() },
            };
            trace!(?event, "process");
            let (effects, snapshot) = {
                let mut world = self.world.lock().await;
                let effects = process(&event, &mut world, &self.clock, &self.topology);
                let snapshot = world_to_snapshot(&world, &self.meta);
                (effects, snapshot)
            };
            // Fan the fresh snapshot out to every WebSocket client.
            self.snapshot_stream.send(snapshot);
            for e in effects {
                self.dispatch(e).await;
            }
        }
    }

    async fn dispatch(&self, effect: Effect) {
        match effect {
            Effect::WriteDbus { target, value } => {
                self.writer.write(target, value).await;
            }
            Effect::CallMyenergi(action) => {
                // Spawn so a slow HTTP call (myenergi cloud) doesn't
                // block the event loop. A-60: wrap the spawned work in
                // a 20 s timeout — reqwest's default is 15 s but the
                // runtime cannot enforce that across multiple in-flight
                // spawns, and without an upper bound a stuck mode-change
                // could accumulate spawns (observed as "last-writer-
                // wins" races across tokio tasks).
                let my = self.myenergi.clone();
                tokio::spawn(async move {
                    match tokio::time::timeout(
                        Duration::from_secs(20),
                        my.execute(action),
                    )
                    .await
                    {
                        Ok(()) => {}
                        Err(_) => warn!(
                            ?action,
                            "myenergi call stuck >20s; dropping"
                        ),
                    }
                });
            }
            Effect::Publish(payload) => {
                if let Some(mqtt) = &self.mqtt {
                    // Never block the dispatch loop longer than 1s on a single
                    // publish. If rumqttc's request queue is saturated (HA
                    // discovery burst, observer-mode ActuatedPhase spam, broker
                    // stall), drop the publish rather than wedge the runtime.
                    match tokio::time::timeout(
                        Duration::from_secs(1),
                        mqtt.publish(payload),
                    )
                    .await
                    {
                        Ok(()) => {}
                        Err(_) => {
                            warn!(?payload, "mqtt publish stuck >1s; dropping");
                        }
                    }
                } else {
                    debug!(?payload, "Publish dropped (no MQTT broker configured)");
                }
            }
            Effect::Log {
                level,
                source,
                message,
            } => match level {
                victron_controller_core::types::LogLevel::Error => {
                    tracing::error!(source, message);
                }
                victron_controller_core::types::LogLevel::Warn => {
                    tracing::warn!(source, message);
                }
                victron_controller_core::types::LogLevel::Info => {
                    tracing::info!(source, message);
                }
                victron_controller_core::types::LogLevel::Debug => {
                    tracing::debug!(source, message);
                }
                victron_controller_core::types::LogLevel::Trace => {
                    tracing::trace!(source, message);
                }
            },
        }
    }
}
