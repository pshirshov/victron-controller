//! Subscribes to the Victron D-Bus services and emits core [`Event`]s on a channel.
//!
//! Uses [`zbus`] in its tokio-backed mode. The Victron BusItem interface
//! emits a `PropertiesChanged`-shaped signal named `ItemsChanged` (with
//! `{path: {Value, Text}}` payload) on the service's root object path.
//! We subscribe to it for each service, parse the value, and dispatch.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, MatchRule, MessageStream, MessageType, Proxy};

use victron_controller_core::controllers::schedules::ScheduleSpec;
use victron_controller_core::types::{
    ActuatedReadback, Event, SensorId, SensorReading, TypedReading,
};

use crate::config::DbusServices;

/// Cadence of the periodic `GetItems` re-seed against every Victron
/// service. Victron emits `ItemsChanged` only on value changes, so
/// without this poll stable values would time out of the freshness
/// window. 5 s keeps sub-tick freshness without overwhelming the
/// Venus D-Bus broker (PR-URGENT-20: 500 ms × 9 services ≈ 18
/// GetItems/sec appeared to trigger broker connection eviction
/// after ~20 s; 5 s is 10× gentler). Freshness window for local
/// D-Bus sensors is tuned together with this (see
/// `ControllerParams::freshness_local_dbus`).
/// Exposed so the dashboard can display it.
pub const DBUS_POLL_PERIOD: std::time::Duration = std::time::Duration::from_secs(5);

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
    /// Routing table from (service, path) → Route. Derived from the
    /// `DbusServices` handed to `new` once up front and reused across
    /// every reconnect attempt.
    routes: Arc<HashMap<(String, String), Route>>,
    /// Unique set of service well-known names, derived from `routes`.
    /// Cached to avoid rebuilding on every reconnect.
    service_set: HashSet<String>,
    /// Accumulator for partial schedule field updates — one per
    /// schedule index (0, 1). Persistent across reconnects so a
    /// mid-accumulation blip doesn't discard fields already received.
    schedule_accumulators: [SchedulePartial; 2],
    /// Poll-tick count since the last heartbeat emission. Reset on
    /// every heartbeat. Persistent across reconnects so heartbeats
    /// remain continuous through transient bus hiccups.
    poll_ticks_since_last_heartbeat: u32,
    /// Raw signal count since the last heartbeat: every `Ok(msg)` from
    /// the ItemsChanged stream, before any filtering. Measures stream
    /// activity / bus health.
    raw_signals_since_last_heartbeat: u32,
    /// Routed signal count since the last heartbeat: signals whose
    /// sender resolved to a known service AND whose path matched a
    /// route in the routing table. Measures delivered readings.
    routed_signals_since_last_heartbeat: u32,
    /// Subscriber start time — used by the heartbeat to log
    /// since-start age for correlating "wedged after ~20 s" reports
    /// against absolute wall time.
    started_at: Instant,
    /// Monotonic time of the most recent successfully-routed
    /// ItemsChanged signal. `None` until the first one arrives.
    /// Used by the heartbeat to flag a silent stream even while the
    /// poll arm is still ticking, and to gate reconnect decisions.
    last_signal_at: Option<Instant>,
    /// Monotonic time of the most recent poll tick in which *at least
    /// one* `GetItems` succeeded. `None` until the first such tick.
    /// Used by the heartbeat to flag broker-side stalls, and to gate
    /// reconnect decisions.
    last_successful_poll_at: Option<Instant>,
}

/// Minimum gap between periodic-`GetItems` failure warnings for a given
/// service. Keeps the log readable during sustained outages.
const RESEED_WARN_THROTTLE: Duration = Duration::from_secs(30);
/// Consecutive failure count at which a single ERROR-level log escalation
/// fires (on top of the rate-limited WARN). At the 5 s poll cadence
/// this is 25 s — well past the 15 s freshness deadline.
const RESEED_ESCALATE_AFTER: u32 = 5;
/// Interval at which the subscriber emits a liveness heartbeat INFO log
/// summarising poll tick + signal counters since the last heartbeat.
/// PR-URGENT-20: shortened from 60 s to 20 s for faster field-debug
/// feedback while we chase the 20-s wedge. Turn back up once stable.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
/// Upper bound on a single Venus GetItems reply. Healthy responses
/// are <50 ms; 2 s is 40x headroom. On timeout, the poll arm marks
/// this service as failed (via `fail_counts`) and continues to the
/// next one — one hung service can no longer starve the whole
/// subscriber `select!` loop (PR-URGENT-19).
const GET_ITEMS_TIMEOUT: Duration = Duration::from_secs(2);
/// Initial reconnect backoff after a session ends. Doubles up to
/// [`RECONNECT_BACKOFF_MAX`] across successive failures; resets to this
/// value after a successful reconnect (= the next session running for
/// at least one heartbeat).
const RECONNECT_BACKOFF_INITIAL: Duration = Duration::from_secs(1);
/// Cap on the exponential reconnect backoff. 30 s balances "notice we
/// are down within a minute" against "don't hammer a recovering broker".
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(30);
/// A session lasting this long is considered "healthy"; on its
/// failure, the reconnect backoff resets to [`RECONNECT_BACKOFF_INITIAL`]
/// rather than continuing to grow. Prevents a stable hour-long
/// subscriber from eating a 30 s backoff after a single disconnect.
const HEALTHY_SESSION_THRESHOLD: Duration = Duration::from_secs(60);
/// Dual-silence threshold driving a reconnect: if the heartbeat fires
/// and both `since_last_signal_s > SILENCE_RECONNECT_THRESHOLD` AND
/// `since_last_poll_success_s > SILENCE_RECONNECT_THRESHOLD`, the
/// session has no evidence the bus is alive and we return Err to the
/// outer loop to reconnect. Must be > HEARTBEAT_INTERVAL so a single
/// transient hiccup doesn't trip it.
const SILENCE_RECONNECT_THRESHOLD: Duration = Duration::from_secs(30);

impl Subscriber {
    /// Build the subscriber config. Pure — no I/O. The actual D-Bus
    /// connection is opened lazily inside [`run`] so reconnects are
    /// symmetric with the initial connect.
    pub fn new(services: &DbusServices) -> Self {
        let routes = Arc::new(routing_table(services));
        let service_set: HashSet<String> =
            routes.keys().map(|(s, _)| s.clone()).collect();
        info!(
            paths = routes.len(),
            services = service_set.len(),
            "D-Bus subscriber configured"
        );
        Self {
            routes,
            service_set,
            schedule_accumulators: [SchedulePartial::default(); 2],
            poll_ticks_since_last_heartbeat: 0,
            raw_signals_since_last_heartbeat: 0,
            routed_signals_since_last_heartbeat: 0,
            started_at: Instant::now(),
            last_signal_at: None,
            last_successful_poll_at: None,
        }
    }

    /// Outer reconnect loop. Repeatedly opens a fresh D-Bus session and
    /// runs it via [`connect_and_serve`] until the channel is dropped
    /// or a clean shutdown is signalled. Individual session failures
    /// (connection drop, broker silence, stream-end) are treated as
    /// transient: they log, wait out an exponential backoff capped at
    /// [`RECONNECT_BACKOFF_MAX`], and reconnect.
    ///
    /// Returns `Ok(())` only on clean shutdown (receiver dropped from
    /// inside a session). No path here terminates the whole binary on
    /// a D-Bus hiccup.
    pub async fn run(mut self, tx: mpsc::Sender<Event>) -> Result<()> {
        let mut backoff = RECONNECT_BACKOFF_INITIAL;
        let mut attempt: u32 = 0;
        loop {
            attempt = attempt.saturating_add(1);
            info!(
                attempt,
                backoff_ms = backoff.as_millis() as u64,
                "D-Bus subscriber: connecting"
            );
            let session_start = Instant::now();
            match self.connect_and_serve(&tx, attempt).await {
                Ok(()) => {
                    info!("D-Bus subscriber exiting cleanly");
                    return Ok(());
                }
                Err(e) => {
                    // Reset backoff when the session lasted long enough
                    // to be considered "healthy", so a clean hour-long
                    // session dropped by the broker reconnects at 1 s
                    // rather than the capped 30 s.
                    let session_age = session_start.elapsed();
                    if session_age > HEALTHY_SESSION_THRESHOLD {
                        backoff = RECONNECT_BACKOFF_INITIAL;
                    }
                    warn!(
                        attempt,
                        session_age_s = session_age.as_secs(),
                        backoff_ms = backoff.as_millis() as u64,
                        error = %format!("{e:#}"),
                        "D-Bus subscriber session ended; reconnecting"
                    );
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(RECONNECT_BACKOFF_MAX);
                }
            }
        }
    }

    /// One session: open a connection, seed, subscribe, and pump events
    /// until an unrecoverable per-session condition is hit (then return
    /// `Err` so the outer loop reconnects) or the receiver is dropped
    /// (then return `Ok(())` for clean shutdown).
    ///
    /// Per-session state (connection, owner map, match stream,
    /// per-service fail counts + warn throttles) lives in locals here;
    /// cross-session state (counters, clocks, schedule accumulators)
    /// stays on `self` so the heartbeat / schedule readback remain
    /// continuous across reconnect.
    async fn connect_and_serve(
        &mut self,
        tx: &mpsc::Sender<Event>,
        attempt: u32,
    ) -> Result<()> {
        let conn = Connection::system()
            .await
            .context("connecting to the system D-Bus")?;

        // 1. Initial seed: call GetItems on every unique service.
        for svc in &self.service_set {
            if let Err(e) = seed_service(
                &conn,
                &self.routes,
                &mut self.schedule_accumulators,
                svc,
                tx,
            )
            .await
            {
                warn!(
                    service = %svc,
                    error = %format!("{e:#}"),
                    "initial GetItems failed; will wait for signals"
                );
            }
        }

        // Build unique-name → well-known-name map. D-Bus signal
        // headers carry the sender's *unique* bus name (e.g. `:1.42`),
        // never the well-known name our routes are keyed by. Resolve
        // each service's current owner once up front.
        let mut owner_to_service: HashMap<String, String> = HashMap::new();
        {
            let dbus_proxy = Proxy::new(
                &conn,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
            )
            .await
            .context("building org.freedesktop.DBus proxy")?;
            for svc in &self.service_set {
                match dbus_proxy.call::<_, _, String>("GetNameOwner", &(svc.as_str())).await {
                    Ok(unique) => {
                        debug!(%svc, %unique, "resolved unique name");
                        owner_to_service.insert(unique, svc.clone());
                    }
                    Err(e) => warn!(
                        service = %svc,
                        error = %format!("{e:#}"),
                        "GetNameOwner failed; signals from this service will be dropped"
                    ),
                }
            }
        }
        info!(
            attempt,
            mapped = owner_to_service.len(),
            total = self.service_set.len(),
            "D-Bus subscriber connected; resolved unique bus names"
        );

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

        let mut stream = MessageStream::for_match_rule(rule, &conn, None)
            .await
            .context("subscribing to ItemsChanged")?;

        // Periodic re-seed ticker. Victron's `ItemsChanged` signals only
        // fire on value *changes*, so stable values (battery SoC at night,
        // ESS state, idle MPPTs, zero vebus current) never emit after the
        // initial GetItems — and our freshness window would mark them
        // Stale, making them unusable for control decisions. A periodic
        // `GetItems` poll against every service keeps everything fresh,
        // and signals still provide sub-tick reactivity for fast-moving
        // paths like grid power. See `DBUS_POLL_PERIOD` for cadence.
        let poll_period = DBUS_POLL_PERIOD;
        let mut poll = tokio::time::interval(poll_period);
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // First tick fires immediately; skip it — we already seeded above.
        poll.tick().await;

        info!(
            poll_period_s = poll_period.as_secs(),
            "D-Bus subscriber running"
        );

        // Heartbeat ticker. Independent of poll.tick() so the
        // subscriber still emits liveness logs at a steady cadence
        // even if the poll arm is starved by a busy signal stream.
        let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
        // First tick is immediate; skip it so we don't log
        // "0 ticks, 0 signals" at startup.
        heartbeat.tick().await;

        // Per-session failure tracking. Reset on every reconnect so
        // operators get fresh warn signals each session.
        let mut fail_counts: HashMap<String, u32> = HashMap::new();
        let mut last_warn: HashMap<String, Instant> = HashMap::new();
        // Start of this session; gates the dual-silence reconnect test
        // so we don't trip it on a session that simply hasn't run long
        // enough to have seen any activity yet.
        let session_started_at = Instant::now();

        loop {
            tokio::select! {
                result = stream.next() => {
                    match result {
                        Some(Ok(msg)) => {
                            self.raw_signals_since_last_heartbeat =
                                self.raw_signals_since_last_heartbeat.saturating_add(1);
                            let header = msg.header();
                            // Sender is a unique bus name like `:1.42`; translate.
                            let Some(sender) = header.sender().map(|s| s.to_string()) else {
                                continue;
                            };
                            let Some(svc) = owner_to_service.get(&sender).cloned() else {
                                debug!(%sender, "signal from unmapped sender");
                                continue;
                            };
                            let Ok(body) = msg.body().deserialize::<ItemsChangedBody>() else {
                                continue;
                            };
                            for (child_path, child_value) in body.0 {
                                let key = (svc.clone(), child_path);
                                let Some(route) = self.routes.get(&key).cloned() else {
                                    continue;
                                };
                                let Some(v) = child_value.value() else { continue };
                                let Some(value) = extract_scalar(v) else {
                                    continue;
                                };
                                self.routed_signals_since_last_heartbeat =
                                    self.routed_signals_since_last_heartbeat.saturating_add(1);
                                let now = Instant::now();
                                self.last_signal_at = Some(now);
                                if let Some(event) = self.route_to_event(&route, value, now) {
                                    if tx.send(event).await.is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            warn!(error = %e, "zbus ItemsChanged stream yielded error");
                        }
                        None => {
                            // Stream-end is our strongest signal the broker
                            // has dropped us. Return Err so the outer loop
                            // reconnects rather than terminating the task.
                            return Err(anyhow!(
                                "ItemsChanged stream ended — broker likely dropped us"
                            ));
                        }
                    }
                }
                _ = poll.tick() => {
                    self.poll_ticks_since_last_heartbeat =
                        self.poll_ticks_since_last_heartbeat.saturating_add(1);
                    let mut any_success = false;
                    for svc in &self.service_set {
                        match seed_service(
                            &conn,
                            &self.routes,
                            &mut self.schedule_accumulators,
                            svc,
                            tx,
                        )
                        .await
                        {
                            Ok(()) => {
                                any_success = true;
                                // Success resets both failure tracking
                                // and the warn-throttle, so the next
                                // failure warns immediately.
                                fail_counts.remove(svc);
                                last_warn.remove(svc);
                            }
                            Err(e) => {
                                let count = fail_counts
                                    .entry(svc.clone())
                                    .or_insert(0);
                                *count += 1;
                                let count_now = *count;
                                let now = Instant::now();
                                let should_warn = last_warn
                                    .get(svc)
                                    .is_none_or(|t| {
                                        now.duration_since(*t) >= RESEED_WARN_THROTTLE
                                    });
                                if should_warn {
                                    warn!(
                                        service = %svc,
                                        count = count_now,
                                        error = %format!("{e:#}"),
                                        "periodic GetItems failed"
                                    );
                                    last_warn.insert(svc.clone(), now);
                                }
                                if count_now == RESEED_ESCALATE_AFTER {
                                    error!(
                                        service = %svc,
                                        "periodic GetItems failing for {RESEED_ESCALATE_AFTER}+ \
                                         consecutive ticks; sensor freshness unreliable"
                                    );
                                }
                            }
                        }
                    }
                    if any_success {
                        self.last_successful_poll_at = Some(Instant::now());
                    }
                }
                _ = heartbeat.tick() => {
                    let poll_ticks =
                        std::mem::take(&mut self.poll_ticks_since_last_heartbeat);
                    let raw_signals =
                        std::mem::take(&mut self.raw_signals_since_last_heartbeat);
                    let routed_signals =
                        std::mem::take(&mut self.routed_signals_since_last_heartbeat);
                    let now = Instant::now();
                    let since_start_s = now.duration_since(self.started_at).as_secs();
                    // `-1` sentinel for "never yet"; avoids std::u64::MAX
                    // showing up as a nonsense age in logs.
                    let since_last_signal_s: i64 = self.last_signal_at.map_or(-1, |t| {
                        i64::try_from(now.duration_since(t).as_secs()).unwrap_or(i64::MAX)
                    });
                    let since_last_poll_success_s: i64 =
                        self.last_successful_poll_at.map_or(-1, |t| {
                            i64::try_from(now.duration_since(t).as_secs()).unwrap_or(i64::MAX)
                        });
                    info!(
                        poll_ticks,
                        raw_signals,
                        routed_signals,
                        since_start_s,
                        since_last_signal_s,
                        since_last_poll_success_s,
                        "dbus subscriber heartbeat"
                    );

                    // Dual-silence reconnect trigger: both signal stream
                    // and poll path have been quiet for longer than
                    // SILENCE_RECONNECT_THRESHOLD. Gate on session age
                    // so a freshly-reconnected session isn't killed
                    // before it has had a chance to receive anything.
                    let session_age = now.duration_since(session_started_at);
                    if session_age > SILENCE_RECONNECT_THRESHOLD {
                        let signal_silent = self
                            .last_signal_at
                            .is_none_or(|t| now.duration_since(t) > SILENCE_RECONNECT_THRESHOLD);
                        let poll_silent = self
                            .last_successful_poll_at
                            .is_none_or(|t| now.duration_since(t) > SILENCE_RECONNECT_THRESHOLD);
                        if signal_silent && poll_silent {
                            return Err(anyhow!(
                                "no ItemsChanged signals and no successful GetItems in \
                                 the last {}s — reconnecting",
                                SILENCE_RECONNECT_THRESHOLD.as_secs()
                            ));
                        }
                    }
                }
                else => break,
            }
        }
        Ok(())
    }

    /// Turn a routed (value, time) into an Event, possibly mutating the
    /// schedule accumulators in the process. Thin wrapper around the
    /// free `route_to_event` helper; the same logic is invoked from
    /// `seed_service` (which cannot take `&mut self`).
    fn route_to_event(&mut self, route: &Route, value: f64, at: Instant) -> Option<Event> {
        route_to_event(route, value, at, &mut self.schedule_accumulators)
    }
}

/// Core routing logic factored out of the impl so both `seed_service`
/// (free fn) and `Subscriber::route_to_event` can share it. The schedule
/// accumulator is passed in by `&mut` so callers can own it in different
/// places (on `Self` for the signal arm, via caller for seed).
fn route_to_event(
    route: &Route,
    value: f64,
    at: Instant,
    schedule_accumulators: &mut [SchedulePartial; 2],
) -> Option<Event> {
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
            let acc = &mut schedule_accumulators[idx];
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
/// Free function (not a method) because it runs with the per-session
/// `Connection` owned by `connect_and_serve`, not by `Self`.
async fn seed_service(
    conn: &Connection,
    routes: &HashMap<(String, String), Route>,
    schedule_accumulators: &mut [SchedulePartial; 2],
    service: &str,
    tx: &mpsc::Sender<Event>,
) -> Result<()> {
    let proxy = Proxy::new(conn, service, "/", "com.victronenergy.BusItem")
        .await
        .context("building GetItems proxy")?;
    // Bound the wait. A healthy Venus returns GetItems in <50 ms; a
    // hung service would otherwise park this whole select arm and
    // starve both the signal and heartbeat arms (PR-URGENT-19).
    let items: HashMap<String, ItemEntry> = tokio::time::timeout(
        GET_ITEMS_TIMEOUT,
        proxy.call("GetItems", &()),
    )
    .await
    .with_context(|| format!("GetItems timed out on {service}"))?
    .with_context(|| format!("GetItems call on {service}"))?;
    debug!(%service, count = items.len(), "seeded from GetItems");
    let at = Instant::now();
    for (path, entry) in items {
        let key = (service.to_string(), path);
        let Some(route) = routes.get(&key).cloned() else {
            continue;
        };
        let Some(v) = entry.value() else { continue };
        let Some(value) = extract_scalar(v) else {
            continue;
        };
        if let Some(event) = route_to_event(&route, value, at, schedule_accumulators) {
            if tx.send(event).await.is_err() {
                return Ok(());
            }
        }
    }
    Ok(())
}

// --- wire types ---

/// A single entry in a GetItems / ItemsChanged map.
///
/// Venus emits `a{sv}` here — a dict-of-variants with keys `"Value"` and
/// `"Text"` (and occasionally others like `"Valid"`, `"Min"`, `"Max"`).
/// We keep only the Value; Text is user-facing and unused for control.
///
/// Earlier the type was `struct { Value, Text }` which deserialises as
/// `(vs)`, but the wire format is `a{sv}`, so zbus rightly refused with
/// `Signature mismatch: got a{sa{sv}}, expected a{s(vs)}`.
#[derive(Debug, serde::Deserialize, zbus::zvariant::Type)]
#[zvariant(signature = "a{sv}")]
struct ItemEntry(HashMap<String, OwnedValue>);

impl ItemEntry {
    fn value(&self) -> Option<&OwnedValue> {
        self.0.get("Value")
    }
}

/// Top-level body of an ItemsChanged signal / GetItems reply:
/// `a{sa{sv}}` — path → (Value, Text, ...).
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
        // Guard admits only finite, non-subnormal floats (plus exact
        // zero); NaN/±Inf/subnormals fall through to the wildcard
        // `_ => None` below (sensor dropout, not data).
        Value::F64(f) if f.is_finite() && (*f == 0.0 || f.is_normal()) => Some(*f),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_scalar_rejects_nonfinite_and_bool() {
        assert_eq!(extract_scalar(&Value::F64(f64::NAN)), None);
        assert_eq!(extract_scalar(&Value::F64(f64::INFINITY)), None);
        assert_eq!(extract_scalar(&Value::F64(f64::NEG_INFINITY)), None);
        assert_eq!(extract_scalar(&Value::F64(-1.5)), Some(-1.5));
        assert_eq!(extract_scalar(&Value::Bool(true)), None);
        assert_eq!(extract_scalar(&Value::Bool(false)), None);
        assert_eq!(
            extract_scalar(&Value::F64(f64::MIN_POSITIVE / 2.0)),
            None,
            "subnormal rejected"
        );
    }
}
