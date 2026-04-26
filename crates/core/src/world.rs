//! The single top-level state container for the pure core. See SPEC §2.3.6.

use chrono::{NaiveDate, NaiveDateTime};
use std::time::Instant;

use crate::controllers::schedules::ScheduleSpec;
use crate::knobs::Knobs;
use crate::myenergi::{EddiMode, ZappiMode, ZappiState};
use crate::tass::{Actual, Actuated};
use crate::types::{BookkeepingId, Decision, ForecastProvider, SensorId, TimerId, TimerStatus};

/// All scalar sensor readings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sensors {
    pub battery_soc: Actual<f64>,
    pub battery_soh: Actual<f64>,
    pub battery_installed_capacity: Actual<f64>,
    pub battery_dc_power: Actual<f64>,
    pub mppt_power_0: Actual<f64>,
    pub mppt_power_1: Actual<f64>,
    pub soltaro_power: Actual<f64>,
    pub power_consumption: Actual<f64>,
    pub grid_power: Actual<f64>,
    pub grid_voltage: Actual<f64>,
    pub grid_current: Actual<f64>,
    pub consumption_current: Actual<f64>,
    pub offgrid_power: Actual<f64>,
    pub offgrid_current: Actual<f64>,
    pub vebus_input_current: Actual<f64>,
    pub evcharger_ac_power: Actual<f64>,
    pub evcharger_ac_current: Actual<f64>,
    pub ess_state: Actual<f64>,
    pub outdoor_temperature: Actual<f64>,
    /// Cumulative energy delivered to the EV in the current Zappi
    /// session (kWh). Surfaced via the dashboard `Sensors` row;
    /// driven by the myenergi cloud poller. See PR-session-kwh-sensor.
    pub session_kwh: Actual<f64>,
}

impl Sensors {
    /// Read the `Actual<f64>` for a given sensor id. Single source of
    /// truth for the `SensorId → world.sensors.<field>` mapping; mirrors
    /// the per-arm match in `apply_sensor_reading` (process.rs) and the
    /// freshness-decay loop in `apply_tick`. Used by
    /// `SensorBroadcastCore` to publish every sensor uniformly.
    #[must_use]
    pub fn by_id(&self, id: SensorId) -> Actual<f64> {
        match id {
            SensorId::BatterySoc => self.battery_soc,
            SensorId::BatterySoh => self.battery_soh,
            SensorId::BatteryInstalledCapacity => self.battery_installed_capacity,
            SensorId::BatteryDcPower => self.battery_dc_power,
            SensorId::MpptPower0 => self.mppt_power_0,
            SensorId::MpptPower1 => self.mppt_power_1,
            SensorId::SoltaroPower => self.soltaro_power,
            SensorId::PowerConsumption => self.power_consumption,
            SensorId::GridPower => self.grid_power,
            SensorId::GridVoltage => self.grid_voltage,
            SensorId::GridCurrent => self.grid_current,
            SensorId::ConsumptionCurrent => self.consumption_current,
            SensorId::OffgridPower => self.offgrid_power,
            SensorId::OffgridCurrent => self.offgrid_current,
            SensorId::VebusInputCurrent => self.vebus_input_current,
            SensorId::EvchargerAcPower => self.evcharger_ac_power,
            SensorId::EvchargerAcCurrent => self.evcharger_ac_current,
            SensorId::EssState => self.ess_state,
            SensorId::OutdoorTemperature => self.outdoor_temperature,
            SensorId::SessionKwh => self.session_kwh,
            // PR-actuated-as-sensors: the actuated-mirror sensor
            // variants don't have dedicated storage on `Sensors`. Their
            // storage of truth is `world.<entity>.actual`; the post-
            // update hook in `apply_sensor_reading` writes there. The
            // `SensorId` exists purely for cadence/routing — these
            // variants intentionally return `Unknown` here, and
            // `SensorBroadcastCore` filters them via
            // `id.actuated_id().is_some()` so the dashboard / HA
            // discovery don't publish `unavailable` for slots whose
            // values are already surfaced via the dedicated `Actuated`
            // table.
            SensorId::GridSetpointActual
            | SensorId::InputCurrentLimitActual
            | SensorId::Schedule0StartActual
            | SensorId::Schedule0DurationActual
            | SensorId::Schedule0SocActual
            | SensorId::Schedule0DaysActual
            | SensorId::Schedule0AllowDischargeActual
            | SensorId::Schedule1StartActual
            | SensorId::Schedule1DurationActual
            | SensorId::Schedule1SocActual
            | SensorId::Schedule1DaysActual
            | SensorId::Schedule1AllowDischargeActual => Actual::unknown(self.battery_soc.since),
        }
    }

    #[must_use]
    pub fn unknown(now: Instant) -> Self {
        Self {
            battery_soc: Actual::unknown(now),
            battery_soh: Actual::unknown(now),
            battery_installed_capacity: Actual::unknown(now),
            battery_dc_power: Actual::unknown(now),
            mppt_power_0: Actual::unknown(now),
            mppt_power_1: Actual::unknown(now),
            soltaro_power: Actual::unknown(now),
            power_consumption: Actual::unknown(now),
            grid_power: Actual::unknown(now),
            grid_voltage: Actual::unknown(now),
            grid_current: Actual::unknown(now),
            consumption_current: Actual::unknown(now),
            offgrid_power: Actual::unknown(now),
            offgrid_current: Actual::unknown(now),
            vebus_input_current: Actual::unknown(now),
            evcharger_ac_power: Actual::unknown(now),
            evcharger_ac_current: Actual::unknown(now),
            ess_state: Actual::unknown(now),
            outdoor_temperature: Actual::unknown(now),
            session_kwh: Actual::unknown(now),
        }
    }
}

/// Per-provider forecast snapshot.
///
/// PR-soc-chart-solar: `hourly_kwh` carries per-hour estimates starting
/// at midnight LOCAL today, length 48 (24 today + 24 tomorrow), kWh
/// per hour. Empty when the provider didn't return hourly data
/// (legacy / quota / partial response). The pre-existing daily totals
/// (`today_kwh` / `tomorrow_kwh`) are unaffected and still drive
/// `forecast_fusion::fused_today_kwh` for the weather_soc planner.
#[derive(Debug, Clone, PartialEq)]
pub struct ForecastSnapshot {
    pub today_kwh: f64,
    pub tomorrow_kwh: f64,
    pub fetched_at: Instant,
    /// Hourly energy estimates starting at midnight LOCAL time today.
    /// Length 48 = 24 today + 24 tomorrow. kWh per hour. Empty when the
    /// provider didn't supply hourly data.
    pub hourly_kwh: Vec<f64>,
}

/// Non-scalar sensor state.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedSensors {
    pub zappi_state: Actual<ZappiState>,
    pub eddi_mode: Actual<EddiMode>,
    pub forecast_solcast: Option<ForecastSnapshot>,
    pub forecast_forecast_solar: Option<ForecastSnapshot>,
    pub forecast_open_meteo: Option<ForecastSnapshot>,
}

impl TypedSensors {
    #[must_use]
    pub fn unknown(now: Instant) -> Self {
        Self {
            zappi_state: Actual::unknown(now),
            eddi_mode: Actual::unknown(now),
            forecast_solcast: None,
            forecast_forecast_solar: None,
            forecast_open_meteo: None,
        }
    }

    #[must_use]
    pub fn forecast(&self, p: ForecastProvider) -> Option<&ForecastSnapshot> {
        match p {
            ForecastProvider::Solcast => self.forecast_solcast.as_ref(),
            ForecastProvider::ForecastSolar => self.forecast_forecast_solar.as_ref(),
            ForecastProvider::OpenMeteo => self.forecast_open_meteo.as_ref(),
        }
    }
}

/// Cross-cutting bookkeeping. Persisted to retained MQTT when changed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bookkeeping {
    pub next_full_charge: Option<NaiveDateTime>,
    pub above_soc_date: Option<NaiveDate>,
    pub prev_ess_state: Option<i32>,
    pub charge_to_full_required: bool,
    pub soc_end_of_day_target: f64,
    /// Effective export SoC threshold (charge-to-full overrides when active).
    pub effective_export_soc_threshold: f64,
    pub battery_selected_soc_target: f64,
    /// Last Eddi mode transition time (used by Eddi controller's dwell check).
    pub eddi_last_transition_at: Option<Instant>,
    /// True if today's weather_soc decided the night charge should
    /// extend through NightExtended (05:00-08:00). Set by `run_weather_soc`
    /// at 01:55; reset to false on every calendar-day rollover (by
    /// `apply_tick`). Read by `run_schedules` as the ONLY signal driving
    /// `charge_battery_extended` (combined with a manual override knob).
    pub charge_battery_extended_today: bool,
    /// Calendar date `charge_battery_extended_today` was last set for, so
    /// the tick-level reset knows when to clear.
    pub charge_battery_extended_today_date: Option<NaiveDate>,
    /// A-21: last calendar date `run_weather_soc` fired its knob proposals.
    /// Prevents the 60-tick flood in the 01:55:00–01:55:59 window.
    /// Stamped only on successful knob application (γ-hold permitting);
    /// not persisted to retained MQTT today, so a reboot inside the 01:55
    /// minute may re-fire — accepted tradeoff.
    pub last_weather_soc_run_date: Option<NaiveDate>,
    /// PR-gamma-hold-redesign: per-tick weather_soc derivations. The
    /// planner writes its current proposal here every tick; the
    /// setpoint / current-limit / schedules controllers read from
    /// these slots when the matching `*_mode = Weather`. Replaces the
    /// previous "planner clobbers user-owned knobs" model. See
    /// `process::effective_*` helpers.
    pub weather_soc_export_soc_threshold: f64,
    pub weather_soc_discharge_soc_target: f64,
    pub weather_soc_battery_soc_target: f64,
    pub weather_soc_disable_night_grid_discharge: bool,
}

impl Bookkeeping {
    #[must_use]
    pub const fn fresh_boot() -> Self {
        Self {
            next_full_charge: None,
            above_soc_date: None,
            prev_ess_state: None,
            charge_to_full_required: false,
            soc_end_of_day_target: 80.0,
            effective_export_soc_threshold: 80.0,
            battery_selected_soc_target: 80.0,
            eddi_last_transition_at: None,
            charge_battery_extended_today: false,
            charge_battery_extended_today_date: None,
            last_weather_soc_run_date: None,
            // PR-gamma-hold-redesign: match Knobs::safe_defaults so a
            // boot before the first weather_soc tick still hands the
            // controllers a sane value when `*_mode = Weather`.
            weather_soc_export_soc_threshold: 80.0,
            weather_soc_discharge_soc_target: 80.0,
            weather_soc_battery_soc_target: 80.0,
            weather_soc_disable_night_grid_discharge: false,
        }
    }
}

/// Pure per-tick derivations. Owned by derivation cores (see
/// `core_dag::cores`), not by `Bookkeeping`. Recomputed every tick from
/// sensors; never retained on MQTT. Consumed by actuator cores that
/// declare a `depends_on` edge on the producing derivation core.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DerivedState {
    /// Written by `ZappiActiveCore` at the top of every tick; read by
    /// `SetpointCore`, `CurrentLimitCore`, and `SchedulesCore`.
    pub zappi_active: bool,
}

/// Name/value pair surfaced in the per-core `last_inputs`/`last_outputs`
/// lists on `CoreState`. PR-core-io-popups.
///
/// Distinct from `crate::types::DecisionFactor` even though the fields
/// are identical — the wire types are decoupled (see `dashboard.baboon`)
/// so a future change to either layer can't cross-pollute the other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreFactor {
    pub id: String,
    pub value: String,
}

/// Per-core observability snapshot, populated by `CoreRegistry::run_all`
/// after each core runs. Pure observability — no controller reads this.
/// PR-tass-dag-view.
///
/// `last_run_outcome` is currently always `"success"` because every
/// production core runs unconditionally per tick (see
/// `CoreRegistry::run_all`); we treat "ran without panicking" as success.
/// The field exists so future cores that conditionally early-return or
/// fail open can surface that to the dashboard without another wire bump.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreState {
    pub id: String,
    pub depends_on: Vec<String>,
    pub last_run_outcome: String,
    /// For derivation cores, the stringified derived value
    /// (e.g. `"true"` / `"false"` for `ZappiActiveCore`). `None` for
    /// actuator cores whose effect is on Decisions/Actuated rather
    /// than a single payload.
    pub last_payload: Option<String>,
    /// Live values the core read on the most recent tick. Empty for
    /// cores that have nothing meaningful to surface (e.g.
    /// `SensorBroadcastCore`, which is pure observability). PR-core-io-popups.
    pub last_inputs: Vec<CoreFactor>,
    /// Live values the core wrote on the most recent tick. Empty when
    /// the core's output is a Decision rather than a discrete value, or
    /// when the previous output isn't cleanly recoverable from the
    /// post-run world (in which case the inputs alone are surfaced).
    /// PR-core-io-popups.
    pub last_outputs: Vec<CoreFactor>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CoresState {
    pub cores: Vec<CoreState>,
    /// Canonical Kahn output order (matches `CoreRegistry::order`).
    pub topo_order: Vec<String>,
}

/// Latest "why?" explanation per controller. Overwritten on every
/// evaluation, so the snapshot always matches the live target state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Decisions {
    pub grid_setpoint: Option<Decision>,
    pub input_current_limit: Option<Decision>,
    pub schedule_0: Option<Decision>,
    pub schedule_1: Option<Decision>,
    pub zappi_mode: Option<Decision>,
    pub eddi_mode: Option<Decision>,
    pub weather_soc: Option<Decision>,
}

/// PR-ha-discovery-expand: per-sensor + per-bookkeeping last-published
/// snapshot, used by `SensorBroadcastCore` to skip republishing
/// identical values every tick. Without this, ~28 retained MQTT
/// publishes/s would hit FlashMQ's republish ceiling and saturate the
/// rumqttc request queue.
///
/// Equality semantics:
/// - Sensors compare both `value` (with bit-exact `f64::to_bits` to
///   the encoded WIRE BODY (PR-ha-discovery-D03/D04). This collapses two
///   cases that bit-equality misses: numeric formatter rounding (e.g.
///   `42.0001` and `42.0002` both render as `"42"`) and the
///   `(Fresh, None)` vs `(Stale, None)` pair which both encode to
///   `"unavailable"`. Invariant: "publish iff the wire body changes".
/// - Numeric bookkeeping compares bit-exact f64.
/// - Bool bookkeeping compares directly.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PublishedCache {
    pub sensors: std::collections::HashMap<SensorId, String>,
    pub bookkeeping_numeric: std::collections::HashMap<BookkeepingId, u64>,
    pub bookkeeping_bool: std::collections::HashMap<BookkeepingId, bool>,
}

/// One per-timer entry mirroring the wire `Timer` shape. PR-timers-section.
#[derive(Debug, Clone, PartialEq)]
pub struct TimerEntry {
    /// Expected period between fires. `0` for one-shot timers.
    pub period: std::time::Duration,
    /// Wall-clock epoch-ms of the last fire (`None` until the first fire).
    pub last_fire_epoch_ms: Option<i64>,
    /// Wall-clock epoch-ms of the projected next fire (`None` for
    /// one-shot timers that have already completed).
    pub next_fire_epoch_ms: Option<i64>,
    /// Current status.
    pub status: TimerStatus,
}

/// Per-timer observability snapshot. Updated by the shell via
/// `Event::TimerState`; consumed by `dashboard::convert` to populate the
/// `Timers` section of the wire snapshot.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timers {
    pub entries: std::collections::HashMap<TimerId, TimerEntry>,
}

/// Top-level world state.
#[derive(Debug, Clone, PartialEq)]
pub struct World {
    // Actuated
    pub grid_setpoint: Actuated<i32>,
    pub input_current_limit: Actuated<f64>,
    pub zappi_mode: Actuated<ZappiMode>,
    pub eddi_mode: Actuated<EddiMode>,
    pub schedule_0: Actuated<ScheduleSpec>,
    pub schedule_1: Actuated<ScheduleSpec>,

    // PR-gamma-hold-redesign: knobs are user-owned plain values; γ-hold
    // and per-knob provenance are gone. Source-of-truth dispatch on the
    // four weather_soc-driven outputs is via the `*_mode` selectors plus
    // `bookkeeping.weather_soc_*` slots.
    pub knobs: Knobs,

    // Sensors
    pub sensors: Sensors,
    pub typed_sensors: TypedSensors,

    // Derived / cross-controller
    pub bookkeeping: Bookkeeping,

    /// Per-tick derivations, written by derivation cores at the top of
    /// each tick. See [`DerivedState`].
    pub derived: DerivedState,

    /// Latest human-readable explanation for each controller. See
    /// SPEC §5.12 and `types::Decision`.
    pub decisions: Decisions,

    /// Per-tick observability snapshot of the TASS core DAG. Populated
    /// by `CoreRegistry::run_all` after each core runs; consumed by
    /// `dashboard::convert::world_to_snapshot`. PR-tass-dag-view.
    pub cores_state: CoresState,

    /// PR-ha-discovery-expand: per-id last-published snapshot driving
    /// publish-on-change for the `SensorBroadcastCore`. Pure
    /// observability; no controller reads from this.
    pub published_cache: PublishedCache,

    /// PR-timers-section: per-timer observability snapshot. Updated by
    /// the shell via `Event::TimerState`; pure observability.
    pub timers: Timers,

    /// PR-tz-from-victron: the Victron-supplied display timezone (IANA
    /// name, e.g. `"Europe/London"`). Updated by `apply_event` on every
    /// successful `Event::Timezone`. Defaults to `"Etc/UTC"` so a
    /// fresh-boot controller has a sensible value before the first
    /// D-Bus reading lands.
    pub timezone: String,

    /// Monotonic timestamp of the most recent successful `Event::Timezone`
    /// observation. `None` until the first reading lands; the dashboard
    /// uses it to mark the synthetic `system.timezone` row Stale once
    /// the freshness window lapses.
    pub timezone_updated_at: Option<Instant>,
}

impl World {
    /// Fresh-boot world: all sensors `Unknown`, all actuated entities
    /// `Unset`, knobs at [`Knobs::safe_defaults`], bookkeeping empty.
    #[must_use]
    pub fn fresh_boot(now: Instant) -> Self {
        Self {
            grid_setpoint: Actuated::new(now),
            input_current_limit: Actuated::new(now),
            zappi_mode: Actuated::new(now),
            eddi_mode: Actuated::new(now),
            schedule_0: Actuated::new(now),
            schedule_1: Actuated::new(now),
            knobs: Knobs::safe_defaults(),
            sensors: Sensors::unknown(now),
            typed_sensors: TypedSensors::unknown(now),
            bookkeeping: Bookkeeping::fresh_boot(),
            derived: DerivedState::default(),
            decisions: Decisions::default(),
            cores_state: CoresState::default(),
            published_cache: PublishedCache::default(),
            timers: Timers::default(),
            // PR-tz-from-victron: default UTC until the first D-Bus
            // `/Settings/System/TimeZone` reading lands.
            timezone: "Etc/UTC".to_string(),
            timezone_updated_at: None,
        }
    }
}
