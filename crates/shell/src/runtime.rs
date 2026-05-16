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
use tokio::sync::{mpsc, Mutex, Notify};
use tracing::{debug, info, trace, warn};

use victron_controller_core::types::{check_staleness_invariant, Effect, Event, SensorId, TypedReading};
use victron_controller_core::{process, Topology, World};

use crate::clock::RealClock;
use crate::dashboard::SnapshotBroadcast;
use crate::dashboard::convert::world_to_snapshot;
use crate::dbus::Writer;
use crate::mqtt::Publisher as MqttPublisher;
use crate::dashboard::convert::MetaContext;
use crate::myenergi::Writer as MyenergiWriter;
use crate::lg_thinq::Writer as LgThinqWriter;

#[derive(Debug)]
pub struct Runtime {
    world: Arc<Mutex<World>>,
    topology: Topology,
    clock: RealClock,
    writer: Writer,
    myenergi: MyenergiWriter,
    /// `None` when `[lg_thinq]` is not configured.
    lg_thinq: Option<LgThinqWriter>,
    mqtt: Option<MqttPublisher>,
    snapshot_stream: Arc<SnapshotBroadcast>,
    meta: MetaContext,
    /// PR2: pinged after the runtime applies a `WeatherCloudForecast`
    /// to `world`. The baseline scheduler waits on this notify in
    /// parallel with its periodic cadence so a fresh cloud forecast
    /// triggers an immediate baseline re-emission instead of waiting
    /// up to one full cadence (typically 1 h) — the startup race that
    /// otherwise leaves the dashboard on an unmodulated, no-cloud
    /// baseline snapshot.
    cloud_forecast_arrived: Arc<Notify>,
}

impl Runtime {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world: Arc<Mutex<World>>,
        writer: Writer,
        myenergi: MyenergiWriter,
        lg_thinq: Option<LgThinqWriter>,
        mqtt: Option<MqttPublisher>,
        topology: Topology,
        snapshot_stream: Arc<SnapshotBroadcast>,
        meta: MetaContext,
        cloud_forecast_arrived: Arc<Notify>,
    ) -> Self {
        // Belt-and-braces against constant edits that bypass the unit
        // test in `crates/core/src/types.rs`. PR-staleness-floor (M-UX-1).
        for &id in SensorId::ALL {
            if let Err(msg) = check_staleness_invariant(id) {
                panic!("{msg}");
            }
        }
        // PR-tz-from-victron: the clock and the core's `Topology` share
        // the same `TzHandle` so a D-Bus `/Settings/System/TimeZone`
        // update lands in `RealClock::naive()` immediately.
        let clock = RealClock::new(topology.tz_handle.clone());
        Self {
            world,
            topology,
            clock,
            writer,
            myenergi,
            lg_thinq,
            mqtt,
            snapshot_stream,
            meta,
            cloud_forecast_arrived,
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
            // PR2: detect cloud-forecast events BEFORE process() consumes
            // the event by value. Used to ping the baseline scheduler
            // once the world update has actually landed.
            let is_cloud_forecast = matches!(
                event,
                Event::TypedSensor(TypedReading::WeatherCloudForecast { .. }),
            );
            let (effects, snapshot) = {
                let mut world = self.world.lock().await;
                let effects = process(&event, &mut world, &self.clock, &self.topology);
                let snapshot = world_to_snapshot(&world, &self.meta);
                (effects, snapshot)
            };
            if is_cloud_forecast {
                // World has the fresh cloud array now — wake the
                // baseline scheduler so it re-emits with modulation
                // applied, instead of waiting up to one cadence
                // boundary.
                self.cloud_forecast_arrived.notify_one();
            }
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
            Effect::WriteDbusPinned { service, path, value } => {
                // PR-pinned-registers: drift-correction write. Goes
                // through the same chokepoint as the regular `write`,
                // so the `[dbus] writes_enabled` dry-run gate fires here
                // too.
                self.writer.write_pinned(&service, &path, value).await;
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
            Effect::CallLgThinq(action) => {
                // Mirror the myenergi dispatch: spawn with a 20 s
                // timeout to avoid blocking the event loop on a slow
                // LG ThinQ Cloud round-trip.
                if let Some(lg) = self.lg_thinq.clone() {
                    tokio::spawn(async move {
                        match tokio::time::timeout(
                            Duration::from_secs(20),
                            lg.execute(action),
                        )
                        .await
                        {
                            Ok(()) => {}
                            Err(_) => warn!(
                                ?action,
                                "lg_thinq call stuck >20s; dropping"
                            ),
                        }
                    });
                } else {
                    trace!(?action, "CallLgThinq dropped (lg_thinq not configured)");
                }
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
