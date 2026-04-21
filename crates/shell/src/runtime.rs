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
use tracing::{debug, info, trace, warn};

use victron_controller_core::types::{Effect, Event, PublishPayload};
use victron_controller_core::{process, Topology, World};

use crate::clock::RealClock;
use crate::dbus::Writer;

#[derive(Debug)]
pub struct Runtime {
    world: World,
    topology: Topology,
    clock: RealClock,
    writer: Writer,
}

impl Runtime {
    pub fn new(writer: Writer, topology: Topology, now: Instant) -> Self {
        Self {
            world: World::fresh_boot(now),
            topology,
            clock: RealClock,
            writer,
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
                // Wired in a later commit — for now just log.
                warn!(?action, "CallMyenergi effect dropped (no myenergi client yet)");
            }
            Effect::Publish(payload) => {
                // Wired in a later commit (MQTT publisher).
                match payload {
                    PublishPayload::Knob { id, value } => {
                        debug!(?id, ?value, "Publish(Knob) dropped (no MQTT yet)");
                    }
                    PublishPayload::ActuatedPhase { id, phase } => {
                        debug!(?id, ?phase, "Publish(ActuatedPhase) dropped (no MQTT yet)");
                    }
                    PublishPayload::KillSwitch(v) => {
                        debug!(v, "Publish(KillSwitch) dropped (no MQTT yet)");
                    }
                    PublishPayload::Bookkeeping(k, v) => {
                        debug!(?k, ?v, "Publish(Bookkeeping) dropped (no MQTT yet)");
                    }
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
