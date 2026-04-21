//! The shell's event loop. Owns the [`World`] and calls [`process`]
//! on every event. Effects are dispatched to their executors
//! (D-Bus writer for `WriteDbus`, future MQTT publisher for `Publish`,
//! etc).
//!
//! There is exactly ONE writer task per actuator bus (currently just
//! D-Bus); that task owns the bus connection and serialises writes,
//! so the core doesn't need to think about concurrency.

use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, info, trace};

use victron_controller_core::types::{Effect, Event};
use victron_controller_core::{process, Topology, World};

use crate::clock::RealClock;
use crate::dbus::Writer;
use crate::mqtt::Publisher as MqttPublisher;
use crate::myenergi::Writer as MyenergiWriter;

#[derive(Debug)]
pub struct Runtime {
    world: World,
    topology: Topology,
    clock: RealClock,
    writer: Writer,
    myenergi: MyenergiWriter,
    mqtt: Option<MqttPublisher>,
}

impl Runtime {
    pub fn new(
        writer: Writer,
        myenergi: MyenergiWriter,
        mqtt: Option<MqttPublisher>,
        topology: Topology,
        now: Instant,
    ) -> Self {
        Self {
            world: World::fresh_boot(now),
            topology,
            clock: RealClock,
            writer,
            myenergi,
            mqtt,
        }
    }

    /// Consume events from `rx` until it closes. Sends a `Tick` every
    /// `tick_period` on its own timer so freshness decay and periodic
    /// controller re-evaluation keep running even without external
    /// events.
    pub async fn run(
        mut self,
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
            let effects = process(&event, &mut self.world, &self.clock, &self.topology);
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
                // block the event loop.
                let my = self.myenergi.clone();
                tokio::spawn(async move {
                    my.execute(action).await;
                });
            }
            Effect::Publish(payload) => {
                if let Some(mqtt) = &self.mqtt {
                    mqtt.publish(payload).await;
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
