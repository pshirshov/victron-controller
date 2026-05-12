//! The single top-level state container for the pure core. See SPEC §2.3.6.

use chrono::{NaiveDate, NaiveDateTime};
use std::time::Instant;

use crate::controllers::schedules::ScheduleSpec;
use crate::knobs::Knobs;
use crate::myenergi::{EddiMode, ZappiMode, ZappiState};
use crate::tass::{Actual, Actuated};
use crate::types::{
    BookkeepingId, ControllerObservableId, Decision, ForecastProvider, PinnedRegisterEntity,
    SensorId, TimerId, TimerStatus, ZappiDrainBranch,
};

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
    /// PR-ev-soc-sensor: EV state-of-charge percentage. Pushed in by
    /// an external MQTT publisher (saic-python-mqtt-gateway today)
    /// after the shell-side subscriber resolves the publisher's HA-
    /// discovery `state_topic`. Read by the daily 04:30 auto-extended-
    /// charge evaluation in `process::maybe_evaluate_auto_extended`;
    /// pure observability for the rest of the controllers.
    pub ev_soc: Actual<f64>,
    /// PR-auto-extended-charge: EV configured charge-target percentage,
    /// sourced from the same gateway as `ev_soc`. Read by the 04:30
    /// auto-extended-charge evaluation; pure observability otherwise.
    pub ev_charge_target: Actual<f64>,
    /// PR-ZD-1: AC power draw of the metered heat pump (W). Sourced from
    /// zigbee2mqtt push (nodon-mtr-heat-pump), JSON `.power` field.
    pub heat_pump_power: Actual<f64>,
    /// PR-ZD-1: AC power draw of the metered cooker/stove (W). Sourced
    /// from zigbee2mqtt push (nodon-mtr-stove), JSON `.power` field.
    pub cooker_power: Actual<f64>,
    /// PR-ZD-1: Operation mode of MPPT charger 0 (ttyUSB1, DI 289).
    /// 0=Off, 1=V/I-limited, 2=MPPT-tracking. Observability only.
    pub mppt_0_operation_mode: Actual<f64>,
    /// PR-ZD-1: Operation mode of MPPT charger 1 (ttyS2, DI 274).
    /// 0=Off, 1=V/I-limited, 2=MPPT-tracking. Observability only.
    pub mppt_1_operation_mode: Actual<f64>,
    /// PR-LG-THINQ-B: current DHW water temperature (°C). Sourced from
    /// the LG ThinQ cloud poller's `HeatPumpState.dhw_current_c`.
    pub lg_dhw_current_c: Actual<f64>,
    /// PR-LG-THINQ-B: current heating-water temperature (°C). Sourced from
    /// the LG ThinQ cloud poller's `HeatPumpState.heating_water_current_c`.
    pub lg_heating_water_current_c: Actual<f64>,
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
            // PR-ev-soc-sensor.
            SensorId::EvSoc => self.ev_soc,
            // PR-auto-extended-charge.
            SensorId::EvChargeTarget => self.ev_charge_target,
            // PR-ZD-1.
            SensorId::HeatPumpPower => self.heat_pump_power,
            SensorId::CookerPower => self.cooker_power,
            SensorId::Mppt0OperationMode => self.mppt_0_operation_mode,
            SensorId::Mppt1OperationMode => self.mppt_1_operation_mode,
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
            | SensorId::Schedule1AllowDischargeActual
            // PR-LG-THINQ-B: actuated-mirror variants — storage of truth
            // lives on `world.lg_*.actual`, not `Sensors`.
            | SensorId::LgHeatPumpPowerActual
            | SensorId::LgDhwPowerActual
            | SensorId::LgHeatingWaterTargetActual
            | SensorId::LgDhwTargetActual => Actual::unknown(self.battery_soc.since),
            // PR-LG-THINQ-B: plain temperature sensors have dedicated fields.
            SensorId::LgDhwCurrentTemperatureC => self.lg_dhw_current_c,
            SensorId::LgHeatingWaterCurrentTemperatureC => self.lg_heating_water_current_c,
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
            ev_soc: Actual::unknown(now),
            ev_charge_target: Actual::unknown(now),
            // PR-ZD-1.
            heat_pump_power: Actual::unknown(now),
            cooker_power: Actual::unknown(now),
            mppt_0_operation_mode: Actual::unknown(now),
            mppt_1_operation_mode: Actual::unknown(now),
            // PR-LG-THINQ-B.
            lg_dhw_current_c: Actual::unknown(now),
            lg_heating_water_current_c: Actual::unknown(now),
        }
    }
}

/// PR-baseline-forecast: how long after the most recent sunrise/sunset
/// observation we still consider those values trustworthy. Two hours of
/// the default 1 h baseline scheduler cadence plus an hour of slack —
/// the values themselves move slowly so the threshold is generous, but
/// finite so a polar night (no event emissions for weeks) reverts to
/// "Unknown" rather than rendering June values in December.
pub const SUNRISE_SUNSET_FRESHNESS: std::time::Duration =
    std::time::Duration::from_secs(3 * 60 * 60);

/// PR-TS-META-1: canonical freshness window for typed sensors fed by the
/// myenergi cloud poller (Eddi mode, Zappi state). Both the runtime
/// `tick(at, …)` decay (via [`crate::topology::ControllerParams::freshness_myenergi`])
/// and the dashboard's wire-level `staleness_ms` route through this
/// constant so the operator-visible threshold cannot drift from the
/// runtime threshold.
pub const MYENERGI_TYPED_FRESHNESS: std::time::Duration =
    std::time::Duration::from_secs(300);

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
    /// Hourly outdoor temperature (°C) starting at midnight LOCAL today.
    /// Same length/indexing convention as `hourly_kwh`. Empty for
    /// providers that don't fetch temperature (Solcast, Forecast.Solar,
    /// baseline). Consumed by the WeatherSoc planner to derive the
    /// daylight-window average that drives the Cold/Warm column choice.
    pub hourly_temperature_c: Vec<f64>,
}

/// Non-scalar sensor state.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedSensors {
    pub zappi_state: Actual<ZappiState>,
    pub eddi_mode: Actual<EddiMode>,
    pub forecast_solcast: Option<ForecastSnapshot>,
    pub forecast_forecast_solar: Option<ForecastSnapshot>,
    pub forecast_open_meteo: Option<ForecastSnapshot>,
    /// PR-baseline-forecast: locally-computed pessimistic baseline.
    /// Consulted by `forecast_fusion` only when no cloud provider is
    /// fresh — see that module for the fallback gate.
    pub forecast_baseline: Option<ForecastSnapshot>,
    /// PR-EDDI-SENSORS-1: last raw `cgi-jstatus-E*` body the poller
    /// saw, pretty-printed. Sibling to `eddi_mode` (rather than nested
    /// inside `Actual<EddiMode>`) because it intentionally outlives
    /// freshness decay — the operator wants the last good body to
    /// paste into a bug report even after the reading goes Stale.
    pub eddi_raw_json: Option<String>,
    /// PR-EDDI-SENSORS-1: same shape as `eddi_raw_json` for the Zappi
    /// `cgi-jstatus-Z*` body.
    pub zappi_raw_json: Option<String>,
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
            forecast_baseline: None,
            eddi_raw_json: None,
            zappi_raw_json: None,
        }
    }

    #[must_use]
    pub fn forecast(&self, p: ForecastProvider) -> Option<&ForecastSnapshot> {
        match p {
            ForecastProvider::Solcast => self.forecast_solcast.as_ref(),
            ForecastProvider::ForecastSolar => self.forecast_forecast_solar.as_ref(),
            ForecastProvider::OpenMeteo => self.forecast_open_meteo.as_ref(),
            ForecastProvider::Baseline => self.forecast_baseline.as_ref(),
        }
    }
}

/// Cross-cutting bookkeeping. Persisted to retained MQTT when changed.
#[derive(Debug, Clone, Copy, PartialEq)]
// PR-auto-extended-charge: now exceeds clippy's 3-bool struct soft cap
// after adding `auto_extended_today`. Splitting Bookkeeping into a
// state machine for the sake of one extra bool would be pure
// gold-plating — every field is independently meaningful.
#[allow(clippy::struct_excessive_bools)]
pub struct Bookkeeping {
    pub next_full_charge: Option<NaiveDateTime>,
    pub above_soc_date: Option<NaiveDate>,
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
    /// PR-auto-extended-charge: result of the most recent 04:30
    /// auto-extended evaluation. Read by
    /// `process::effective_charge_car_extended` when the user-set mode
    /// is `Auto`. Mutated only by `process::maybe_evaluate_auto_extended`
    /// (latched per local date).
    pub auto_extended_today: bool,
    /// PR-auto-extended-charge: local date the most recent auto
    /// evaluation fired for. `None` until the first 04:30 cycle since
    /// process start.
    pub auto_extended_today_date: Option<NaiveDate>,
}

impl Bookkeeping {
    #[must_use]
    pub const fn fresh_boot() -> Self {
        Self {
            next_full_charge: None,
            above_soc_date: None,
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
            // PR-auto-extended-charge: defensive default. Until the first
            // 04:30 evaluation fires, `Auto` mode treats the EV as not
            // needing the cheap-rate window.
            auto_extended_today: false,
            auto_extended_today_date: None,
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
    /// Target ESS state decision (9 inside the override window on
    /// full-charge days, 10 otherwise).
    pub ess_state_override: Option<Decision>,
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
    /// PR-ZDO-2: dedup cache for controller-derived numerics (encoded wire body).
    pub controller_numeric: std::collections::HashMap<ControllerObservableId, String>,
    /// PR-ZDO-2: dedup cache for controller-derived booleans.
    pub controller_bool: std::collections::HashMap<ControllerObservableId, bool>,
    /// Dedup cache for string-valued controller observables (encoded
    /// wire body — `"unavailable"` for absent values).
    pub controller_enum: std::collections::HashMap<ControllerObservableId, String>,
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

/// Snapshot of the compensated-drain controller's per-tick state.
/// Captured at the end of `run_setpoint` (after both soft-loop and
/// hard-clamp branches have decided their outputs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZappiDrainSnapshot {
    /// Compensated battery drain (W) the controller saw this tick.
    /// Meaningful only when `branch != Disabled`. When `branch == Disabled`,
    /// this carries `0.0` as a placeholder — renderers (PR-ZDO-4) MUST
    /// skip / grey-out `Disabled` samples to avoid plotting a misleading
    /// zero line during safety fallbacks.
    pub compensated_drain_w: f64,
    /// Which branch of the compensated-drain controller fired this tick.
    pub branch: ZappiDrainBranch,
    /// Whether the Fast-mode hard clamp engaged this tick.
    pub hard_clamp_engaged: bool,
    /// W by which the hard clamp raised the proposed setpoint when
    /// `hard_clamp_engaged == true`; 0.0 otherwise.
    pub hard_clamp_excess_w: f64,
    /// Soft-loop threshold (W) at the time of capture. Snapshotted so
    /// the chart's reference line stays consistent if the operator
    /// retunes mid-window.
    pub threshold_w: i32,
    /// Fast-mode hard-clamp threshold (W) at the time of capture.
    pub hard_clamp_w: i32,
    /// Wall-clock UTC epoch ms at capture time. Non-monotonic if the
    /// GX clock jumps backwards; renderers (PR-ZDO-4) sort by this
    /// field at draw time before plotting.
    pub captured_at_ms: i64,
}

/// Compact form for the ring buffer — only the fields the chart needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZappiDrainSample {
    /// Wall-clock UTC epoch ms at capture time. Non-monotonic if the
    /// GX clock jumps backwards; renderers (PR-ZDO-4) sort by this
    /// field at draw time before plotting.
    pub captured_at_ms: i64,
    /// Compensated battery drain (W) the controller saw this tick.
    /// Meaningful only when `branch != Disabled`. When `branch == Disabled`,
    /// this carries `0.0` as a placeholder — renderers (PR-ZDO-4) MUST
    /// skip / grey-out `Disabled` samples to avoid plotting a misleading
    /// zero line during safety fallbacks.
    pub compensated_drain_w: f64,
    /// Which branch of the compensated-drain controller fired this tick.
    pub branch: ZappiDrainBranch,
    /// Whether the Fast-mode hard clamp engaged this tick.
    pub hard_clamp_engaged: bool,
}

impl From<&ZappiDrainSnapshot> for ZappiDrainSample {
    fn from(s: &ZappiDrainSnapshot) -> Self {
        Self {
            captured_at_ms: s.captured_at_ms,
            compensated_drain_w: s.compensated_drain_w,
            branch: s.branch,
            hard_clamp_engaged: s.hard_clamp_engaged,
        }
    }
}

/// Observability surface for the M-ZAPPI-DRAIN compensated-drain loop.
/// Carries the latest snapshot plus a 30-minute rolling history (120
/// samples at the default 15 s tick cadence). Reset on `fresh_boot`;
/// no persistence. **Read-only from the controller's perspective** —
/// no controller branch reads from this struct (locked invariant,
/// PR-ZDO-1.T6).
#[derive(Debug, Clone)]
pub struct ZappiDrainState {
    pub latest: Option<ZappiDrainSnapshot>,
    pub samples: std::collections::VecDeque<ZappiDrainSample>,
}

impl ZappiDrainState {
    /// Maximum number of samples retained in the ring buffer.
    /// 120 × 15 s = 30 min.
    pub const RING_CAPACITY: usize = 120;

    /// Minimum time between consecutive ring-buffer samples (ms). Enforced
    /// by `push` so the buffer covers a fixed 30-minute window regardless
    /// of how often `run_setpoint` is invoked. `latest` is updated on
    /// every `push` call so HA broadcasts and wire-format snapshots stay
    /// lockstep with the controller.
    pub const SAMPLE_INTERVAL_MS: i64 = 15_000;

    /// Update `latest` unconditionally; append to `samples` only when
    /// the new snapshot is at least `SAMPLE_INTERVAL_MS` newer than the
    /// most recent sample. FIFO-evicts on overflow at `RING_CAPACITY`.
    ///
    /// **Clock skew**: a backwards wall-clock jump (e.g. ntpdate correction)
    /// makes the gate `snap.captured_at_ms - prev.captured_at_ms >=
    /// SAMPLE_INTERVAL_MS` reject all subsequent pushes until wall-clock
    /// recovers past `prev.captured_at_ms + SAMPLE_INTERVAL_MS`. During that
    /// window `samples` does not grow and the dashboard chart appears frozen,
    /// but `latest` continues to update every call so HA broadcasts and
    /// wire-format `latest` snapshots stay current.
    pub fn push(&mut self, snap: ZappiDrainSnapshot) {
        self.latest = Some(snap);
        let should_append = match self.samples.back() {
            None => true,
            Some(prev) => snap.captured_at_ms - prev.captured_at_ms >= Self::SAMPLE_INTERVAL_MS,
        };
        if !should_append {
            return;
        }
        if self.samples.len() == Self::RING_CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(ZappiDrainSample::from(&snap));
    }
}

impl Default for ZappiDrainState {
    fn default() -> Self {
        Self {
            latest: None,
            samples: std::collections::VecDeque::with_capacity(Self::RING_CAPACITY),
        }
    }
}

impl PartialEq for ZappiDrainState {
    fn eq(&self, other: &Self) -> bool {
        self.latest == other.latest && self.samples == other.samples
    }
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
    /// PR-keep-batteries-charged: target ESS state (Victron
    /// `/Settings/CGwacs/BatteryLife/State`). Writes are produced by
    /// `EssStateOverrideCore`; readback feeds in via `SensorId::EssState`
    /// (see `apply_sensor_reading` for the on_reading + confirm_if
    /// hook).
    pub ess_state_target: Actuated<i32>,
    // PR-LG-THINQ-B: four LG heat-pump actuated entities.
    /// Master power on/off. Proposed by `apply_knob` (operator); not
    /// proposed by `HeatPumpControlCore` — the controller skips this slot.
    pub lg_heat_pump_power: Actuated<bool>,
    /// DHW power on/off. Proposed by `HeatPumpControlCore` (time-window
    /// schedule) and by `apply_knob` (operator override).
    pub lg_dhw_power: Actuated<bool>,
    /// Heating-water temperature target (°C). Proposed by
    /// `HeatPumpControlCore` (outdoor-temp curve) and operator.
    pub lg_heating_water_target_c: Actuated<i32>,
    /// DHW temperature target (°C, constant 60). Proposed by
    /// `HeatPumpControlCore` and operator.
    pub lg_dhw_target_c: Actuated<i32>,

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

    /// PR-ZDO-1: compensated-drain observability ring buffer. Written by
    /// `run_setpoint` and `apply_setpoint_safety` every tick; never read
    /// by any controller branch (locked invariant, PR-ZDO-1.T6).
    pub zappi_drain_state: ZappiDrainState,

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

    /// PR-baseline-forecast: today's sunrise / sunset in local time.
    /// Driven by `Event::SunriseSunset` from the shell-side baseline
    /// scheduler. `None` until the first event lands (or for polar days
    /// where the sun never rises/sets).
    pub sunrise: Option<NaiveDateTime>,
    pub sunset: Option<NaiveDateTime>,
    /// Monotonic timestamp of the most recent successful sunrise/sunset
    /// observation; matches the freshness pattern used for
    /// `timezone_updated_at`. Consumers compare `now - this` against
    /// [`SUNRISE_SUNSET_FRESHNESS`] to decide whether the cached
    /// sunrise/sunset are still trustworthy. The baseline scheduler
    /// runs at a 1 h default cadence; we allow ~2 cadences plus slack
    /// before we declare the values stale, identical to the cloud
    /// forecast freshness pattern. Polar nights / equipment failure
    /// drop the value back to "no fresh signal" without us having to
    /// fabricate clearing events.
    pub sunrise_sunset_updated_at: Option<Instant>,

    /// PR-pinned-registers: per-register drift state. Keyed by the
    /// joined `service:dbus_path` so the shell-side reader can match
    /// readings back to entries without re-splitting the path. Entries
    /// are seeded once at startup from `[[dbus_pinned_registers]]`;
    /// `run_pinned_registers` is a no-op when the map is empty.
    pub pinned_registers: std::collections::BTreeMap<std::sync::Arc<str>, PinnedRegisterEntity>,

    /// PR-WSOC-ACTIVE-1: the (bucket, temp) cell the WeatherSocPlanner
    /// is currently driving from. Set by `run_weather_soc` after each
    /// successful evaluation; cleared (left as `None`) when the planner
    /// skips because of stale forecast / unusable outdoor temperature.
    /// Pure observability — no controller reads from this. Surfaced on
    /// the wire as `WorldSnapshot.weather_soc_active` for the dashboard
    /// to highlight the active 4-cell group in the Weather-SoC widget.
    pub weather_soc_active:
        Option<(crate::weather_soc_addr::EnergyBucket, crate::weather_soc_addr::TempCol)>,

    /// Inputs the WeatherSoc planner consulted on its most recent
    /// successful tick: temperature value + source, fused solar
    /// energy forecast, and today-vs-tomorrow selection. Set by
    /// `run_weather_soc` after every tick that produces a Cold/Warm
    /// classification; left as `None` when the planner skipped
    /// because no temperature input was usable. Surfaced on the wire
    /// as `WorldSnapshot.weather_soc_inputs` for the dashboard
    /// caption.
    pub weather_soc_inputs: Option<WeatherSocInputs>,
}

/// Source of the temperature value the WeatherSoc planner used on a
/// given tick. Mirrors the wire enum `WeatherSocTemperatureSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherSocTemperatureSource {
    /// Daylight-window arithmetic mean over the day's hourly forecast
    /// (Open-Meteo `temperature_2m`), bounded by `world.sunrise` →
    /// `world.sunset`. The day is today before today's sunset and
    /// tomorrow after.
    Forecast,
    /// Instantaneous outdoor sensor reading
    /// (`world.sensors.outdoor_temperature.value`). Fallback path when
    /// the forecast is stale, missing, or sunrise/sunset is unknown.
    Sensor,
}

/// Which day's forecast slice the planner sourced from. Mirrors the
/// wire enum `WeatherSocDay`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherSocDay {
    Today,
    Tomorrow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherSocInputs {
    pub temperature_c: f64,
    pub temperature_source: WeatherSocTemperatureSource,
    pub energy_kwh: f64,
    pub day: WeatherSocDay,
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
            ess_state_target: Actuated::new(now),
            // PR-LG-THINQ-B.
            lg_heat_pump_power: Actuated::new(now),
            lg_dhw_power: Actuated::new(now),
            lg_heating_water_target_c: Actuated::new(now),
            lg_dhw_target_c: Actuated::new(now),
            knobs: Knobs::safe_defaults(),
            sensors: Sensors::unknown(now),
            typed_sensors: TypedSensors::unknown(now),
            bookkeeping: Bookkeeping::fresh_boot(),
            derived: DerivedState::default(),
            decisions: Decisions::default(),
            cores_state: CoresState::default(),
            published_cache: PublishedCache::default(),
            timers: Timers::default(),
            zappi_drain_state: ZappiDrainState::default(),
            // PR-tz-from-victron: default UTC until the first D-Bus
            // `/Settings/System/TimeZone` reading lands.
            timezone: "Etc/UTC".to_string(),
            timezone_updated_at: None,
            sunrise: None,
            sunset: None,
            sunrise_sunset_updated_at: None,
            pinned_registers: std::collections::BTreeMap::new(),
            weather_soc_active: None,
            weather_soc_inputs: None,
        }
    }
}
