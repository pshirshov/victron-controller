//! Subscribes to the Victron D-Bus services and emits core [`Event`]s on a channel.
//!
//! Uses [`zbus`] in its tokio-backed mode. The Victron BusItem interface
//! emits a `PropertiesChanged`-shaped signal named `ItemsChanged` (with
//! `{path: {Value, Text}}` payload) on the service's root object path.
//! We subscribe to it for each service, parse the value, and dispatch.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, MatchRule, MessageStream, MessageType, Proxy};

use victron_controller_core::controllers::schedules::ScheduleSpec;
use victron_controller_core::types::{
    ActuatedReadback, Event, SensorId, SensorReading, TypedReading,
};

use crate::config::DbusServices;

/// Table mapping `(service, path)` to the core Event we should emit.
///
/// We keep this small and explicit rather than deriving it — there are
/// only ~20 paths and their semantics differ (scalar sensor vs. typed
/// vs. readback of an actuated).
#[derive(Debug, Clone)]
enum Route {
    Sensor(SensorId),
    GridSetpointReadback,
    CurrentLimitReadback,
    /// Partial update to Schedule N's Nth field. The subscriber
    /// accumulates all five fields before emitting a complete
    /// ScheduleSpec readback.
    ScheduleField { index: u8, field: ScheduleSpecField },
}

/// Which field of a ScheduleSpec this D-Bus path corresponds to.
#[derive(Debug, Clone, Copy)]
enum ScheduleSpecField {
    Start,
    Duration,
    Soc,
    Days,
    AllowDischarge,
}

/// Build the routing table from the configured bus names. Keyed by
/// `(service, path)` for O(1) lookup on each incoming event.
fn routing_table(s: &DbusServices) -> HashMap<(String, String), Route> {
    use SensorId::*;
    let mut r = HashMap::new();
    let add = |r: &mut HashMap<(String, String), Route>,
               svc: &str,
               path: &str,
               route: Route| {
        r.insert((svc.to_string(), path.to_string()), route);
    };

    // system
    add(&mut r, &s.system, "/Ac/Consumption/L1/Power", Route::Sensor(PowerConsumption));
    add(&mut r, &s.system, "/Ac/Consumption/L1/Current", Route::Sensor(ConsumptionCurrent));
    add(&mut r, &s.system, "/Ac/Grid/L1/Power", Route::Sensor(GridPower));

    // battery
    add(&mut r, &s.battery, "/Soc", Route::Sensor(BatterySoc));
    add(&mut r, &s.battery, "/Soh", Route::Sensor(BatterySoh));
    add(&mut r, &s.battery, "/InstalledCapacity", Route::Sensor(BatteryInstalledCapacity));
    add(&mut r, &s.battery, "/Dc/0/Power", Route::Sensor(BatteryDcPower));

    // MPPTs
    add(&mut r, &s.mppt_0, "/Yield/Power", Route::Sensor(MpptPower0));
    add(&mut r, &s.mppt_1, "/Yield/Power", Route::Sensor(MpptPower1));

    // Soltaro pvinverter
    add(&mut r, &s.pvinverter_soltaro, "/Ac/Power", Route::Sensor(SoltaroPower));

    // Grid meter
    add(&mut r, &s.grid, "/Ac/L1/Voltage", Route::Sensor(GridVoltage));
    add(&mut r, &s.grid, "/Ac/L1/Current", Route::Sensor(GridCurrent));

    // Vebus inverter
    add(&mut r, &s.vebus, "/Ac/Out/L1/P", Route::Sensor(OffgridPower));
    add(&mut r, &s.vebus, "/Ac/Out/L1/I", Route::Sensor(OffgridCurrent));
    add(&mut r, &s.vebus, "/Ac/ActiveIn/L1/I", Route::Sensor(VebusInputCurrent));
    add(&mut r, &s.vebus, "/Ac/In/1/CurrentLimit", Route::CurrentLimitReadback);

    // Evcharger (EV-branch ET112)
    add(&mut r, &s.evcharger, "/Ac/Power", Route::Sensor(EvchargerAcPower));
    add(&mut r, &s.evcharger, "/Ac/Current", Route::Sensor(EvchargerAcCurrent));

    // Settings
    add(
        &mut r,
        &s.settings,
        "/Settings/CGwacs/AcPowerSetPoint",
        Route::GridSetpointReadback,
    );
    add(&mut r, &s.settings, "/Settings/CGwacs/BatteryLife/State", Route::Sensor(EssState));

    // Schedule readback — 5 fields × 2 schedules. Each partial update
    // advances the subscriber's accumulator; a full ScheduleSpec is
    // emitted once all 5 fields have arrived.
    for index in 0..=1u8 {
        for (path_field, spec_field) in [
            ("Start", ScheduleSpecField::Start),
            ("Duration", ScheduleSpecField::Duration),
            ("Soc", ScheduleSpecField::Soc),
            ("Day", ScheduleSpecField::Days),
            ("AllowDischarge", ScheduleSpecField::AllowDischarge),
        ] {
            add(
                &mut r,
                &s.settings,
                &format!("/Settings/CGwacs/BatteryLife/Schedule/Charge/{index}/{path_field}"),
                Route::ScheduleField { index, field: spec_field },
            );
        }
    }

    r
}

/// Accumulator for partial ScheduleSpec updates. Each schedule's
/// fields trickle in one D-Bus signal at a time; we emit a complete
/// ScheduleSpec to the core every time any of them changes, populating
/// missing fields with sentinel zeros (which the target comparison
/// will fail to match — driving phase to stay in Commanded until all
/// fields converge).
#[derive(Debug, Clone, Copy, Default)]
struct SchedulePartial {
    start_s: Option<i32>,
    duration_s: Option<i32>,
    soc: Option<f64>,
    days: Option<i32>,
    discharge: Option<i32>,
}

impl SchedulePartial {
    fn apply(&mut self, field: ScheduleSpecField, value: f64) {
        match field {
            ScheduleSpecField::Start => self.start_s = Some(value as i32),
            ScheduleSpecField::Duration => self.duration_s = Some(value as i32),
            ScheduleSpecField::Soc => self.soc = Some(value),
            ScheduleSpecField::Days => self.days = Some(value as i32),
            ScheduleSpecField::AllowDischarge => self.discharge = Some(value as i32),
        }
    }

    /// Return a complete ScheduleSpec IFF all 5 fields have arrived at
    /// least once. Missing fields return None.
    fn as_spec(&self) -> Option<ScheduleSpec> {
        Some(ScheduleSpec {
            start_s: self.start_s?,
            duration_s: self.duration_s?,
            soc: self.soc?,
            days: self.days?,
            discharge: self.discharge?,
        })
    }
}

#[derive(Debug)]
pub struct Subscriber {
    conn: Connection,
    routes: Arc<HashMap<(String, String), Route>>,
    /// Accumulator for partial schedule field updates — one per
    /// schedule index (0, 1). Mutable across the run() loop.
    schedule_accumulators: [SchedulePartial; 2],
}

impl Subscriber {
    /// Connect to the Venus system bus.
    pub async fn connect(services: &DbusServices) -> Result<Self> {
        let conn = Connection::system()
            .await
            .context("connecting to the system D-Bus")?;
        let routes = Arc::new(routing_table(services));
        info!("D-Bus subscriber connected; {} paths routed", routes.len());
        Ok(Self {
            conn,
            routes,
            schedule_accumulators: [SchedulePartial::default(); 2],
        })
    }

    /// Start the subscriber loop. Each service is seeded with a
    /// `GetItems` call (so we bootstrap the world without waiting for
    /// the first value to tick), then we subscribe to its
    /// `ItemsChanged` signal and forward to `tx` for the lifetime of
    /// the task.
    ///
    /// Returns when `tx` is dropped or on an unrecoverable bus error.
    pub async fn run(mut self, tx: mpsc::Sender<Event>) -> Result<()> {
        // 1. Initial seed: call GetItems on every unique service.
        let services: std::collections::HashSet<String> = self
            .routes
            .keys()
            .map(|(s, _)| s.clone())
            .collect();
        for svc in &services {
            if let Err(e) = self.seed_service(svc, &tx).await {
                warn!(service = %svc, error = %e, "initial GetItems failed; will wait for signals");
            }
        }

        // 2. Subscribe to ItemsChanged across every Victron service.
        //    Venus emits these signals with member=`ItemsChanged` on
        //    interface=`com.victronenergy.BusItem` at path `/`.
        let rule = MatchRule::builder()
            .msg_type(MessageType::Signal)
            .interface("com.victronenergy.BusItem")
            .context("building MatchRule interface")?
            .member("ItemsChanged")
            .context("building MatchRule member")?
            .build();

        let mut stream = MessageStream::for_match_rule(rule, &self.conn, None)
            .await
            .context("subscribing to ItemsChanged")?;

        info!("D-Bus subscriber running");
        while let Some(msg) = stream.next().await {
            let Ok(msg) = msg else {
                continue;
            };
            let header = msg.header();
            let Some(svc) = header.sender().map(|s| s.to_string()) else {
                continue;
            };
            let path = header.path().map(|p| p.to_string()).unwrap_or_default();
            if !self.routes.keys().any(|(s, _)| s == &svc) {
                debug!(%svc, %path, "unrouted signal");
                continue;
            }
            let Ok(body) = msg.body().deserialize::<ItemsChangedBody>() else {
                continue;
            };
            for (child_path, child_value) in body.0 {
                let key = (svc.clone(), child_path);
                let Some(route) = self.routes.get(&key).cloned() else {
                    continue;
                };
                let Some(value) = extract_scalar(&child_value.value) else {
                    continue;
                };
                if let Some(event) = self.route_to_event(&route, value, Instant::now()) {
                    if tx.send(event).await.is_err() {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    /// Turn a routed (value, time) into an Event, possibly mutating the
    /// schedule accumulators in the process.
    fn route_to_event(&mut self, route: &Route, value: f64, at: Instant) -> Option<Event> {
        match route {
            Route::Sensor(id) => Some(Event::Sensor(SensorReading {
                id: *id,
                value,
                at,
            })),
            Route::GridSetpointReadback => {
                #[allow(clippy::cast_possible_truncation)]
                Some(Event::Readback(ActuatedReadback::GridSetpoint {
                    value: value as i32,
                    at,
                }))
            }
            Route::CurrentLimitReadback => {
                Some(Event::Readback(ActuatedReadback::InputCurrentLimit {
                    value,
                    at,
                }))
            }
            Route::ScheduleField { index, field } => {
                let idx = *index as usize;
                let acc = &mut self.schedule_accumulators[idx];
                acc.apply(*field, value);
                // Only emit once we have all 5 fields.
                let spec = acc.as_spec()?;
                Some(Event::Readback(if *index == 0 {
                    ActuatedReadback::Schedule0 { value: spec, at }
                } else {
                    ActuatedReadback::Schedule1 { value: spec, at }
                }))
            }
        }
    }

    /// Bootstrap: ask a service for all its items at once via GetItems.
    async fn seed_service(&mut self, service: &str, tx: &mpsc::Sender<Event>) -> Result<()> {
        let proxy = Proxy::new(&self.conn, service, "/", "com.victronenergy.BusItem")
            .await
            .context("building GetItems proxy")?;
        let items: HashMap<String, ItemEntry> = proxy
            .call("GetItems", &())
            .await
            .context("GetItems call")?;
        debug!(%service, count = items.len(), "seeded from GetItems");
        let at = Instant::now();
        for (path, entry) in items {
            let key = (service.to_string(), path);
            let Some(route) = self.routes.get(&key).cloned() else {
                continue;
            };
            let Some(value) = extract_scalar(&entry.value) else {
                continue;
            };
            if let Some(event) = self.route_to_event(&route, value, at) {
                if tx.send(event).await.is_err() {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

// --- wire types ---

/// A single entry in a GetItems / ItemsChanged map: `{Value, Text}`.
/// Only the Value is used for control; Text is the user-facing string.
#[derive(Debug, serde::Deserialize, zbus::zvariant::Type)]
struct ItemEntry {
    #[serde(rename = "Value")]
    value: OwnedValue,
    #[serde(rename = "Text")]
    #[allow(dead_code)]
    text: String,
}

/// Top-level body of an ItemsChanged signal:
/// `a{s(a{sv})}` — map path → {Value, Text}.
#[derive(Debug, serde::Deserialize, zbus::zvariant::Type)]
struct ItemsChangedBody(HashMap<String, ItemEntry>);

// --- value extraction ---

/// Pull an `f64` out of a zvariant value, coercing across the integer
/// and floating types Venus emits. Returns `None` for unexpected
/// shapes (e.g. arrays, dicts).
fn extract_scalar(v: &Value<'_>) -> Option<f64> {
    // zbus 4 Value has F64 but no F32 variant at the top level (floats
    // are f64 on the wire). Integer variants vary by width.
    match v {
        Value::F64(f) => Some(*f),
        Value::I32(n) => Some(f64::from(*n)),
        Value::U32(n) => Some(f64::from(*n)),
        Value::I64(n) => {
            #[allow(clippy::cast_precision_loss)]
            Some(*n as f64)
        }
        Value::U64(n) => {
            #[allow(clippy::cast_precision_loss)]
            Some(*n as f64)
        }
        Value::I16(n) => Some(f64::from(*n)),
        Value::U16(n) => Some(f64::from(*n)),
        Value::U8(n) => Some(f64::from(*n)),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

// Silence unused-variable warning for the TypedReading variant that
// the subscriber doesn't emit (it comes from the myenergi poller
// instead).
const _: fn() = || {
    let _ = TypedReading::Eddi {
        mode: victron_controller_core::myenergi::EddiMode::Stopped,
        at: Instant::now(),
    };
};
