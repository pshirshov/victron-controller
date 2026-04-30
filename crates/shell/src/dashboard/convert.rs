//! Convert `core::World` into a `WorldSnapshot` for serialization, and
//! decode baboon `Command`s into `core::Event`s.
//!
//! Kept dumb: one function per struct, one field per mapping. Intentionally
//! no clever abstractions — the cost of adding a new core field is a
//! single additional line here rather than a type-level detour.

#![allow(clippy::many_single_char_names)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::ref_option)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::too_many_lines)]

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use victron_controller_core::controllers::schedules::ScheduleSpec;
use victron_controller_core::knobs::{
    ChargeBatteryExtendedMode, DebugFullCharge, DischargeTime, ExtendedChargeMode,
    ForecastDisagreementStrategy, Knobs,
};
use std::collections::BTreeMap;
use std::time::Duration;

use victron_controller_core::myenergi::{EddiMode, ZappiMode};
use victron_controller_core::tass::{Actual, Actuated, Freshness, TargetPhase};
use victron_controller_core::topology::{ControllerParams, HardwareParams};
use victron_controller_core::types::{
    BookkeepingKey, BookkeepingValue, Command, Decision, Event, KnobId, KnobValue, SensorId,
    TimerId,
};
use victron_controller_core::world::World;
use victron_controller_core::Owner;

use crate::config::DbusServices;
use crate::dashboard::soc_history::SocHistoryStore;

/// Runtime inputs needed to build `sensors_meta`. Bundled so callers
/// can pass one reference rather than juggling four arguments.
#[derive(Debug, Clone)]
pub struct MetaContext {
    pub services: DbusServices,
    pub open_meteo_cadence: Duration,
    pub controller_params: ControllerParams,
    /// PR-matter-outdoor-temp: when `Some`, the dashboard's
    /// `outdoor_temperature` row is annotated with the Matter MQTT
    /// origin/topic instead of Open-Meteo. Open-Meteo remains a silent
    /// fallback source (whichever publishes most recently wins via
    /// `Actual::on_reading`'s freshness reset).
    pub matter_outdoor_topic: Option<String>,
    /// PR-ev-soc-sensor: when `Some`, the dashboard's `ev_soc` row is
    /// annotated with this discovery topic. The actual subscribe path
    /// resolves the publisher's `state_topic` from the retained
    /// discovery payload — that resolved topic isn't visible here, so
    /// we surface the discovery topic as the operator-visible
    /// identifier instead.
    pub ev_soc_discovery_topic: Option<String>,
    /// PR-auto-extended-charge: same shape as `ev_soc_discovery_topic`,
    /// for the EV's configured charge-target sensor.
    pub ev_charge_target_discovery_topic: Option<String>,
    /// PR-ZD-1: configured zigbee2mqtt topic for the heat pump power
    /// sensor. Surfaced on the dashboard sensor meta row.
    pub heat_pump_topic: Option<String>,
    /// PR-ZD-1: configured zigbee2mqtt topic for the cooker power sensor.
    pub cooker_topic: Option<String>,
    /// PR-soc-chart: shared in-memory ring of recent SoC samples.
    /// `world_to_snapshot` reads from this synchronously to populate
    /// `soc_chart.history`.
    pub soc_history: Arc<SocHistoryStore>,
    /// PR-soc-chart: hardware params (specifically
    /// `battery_nominal_voltage_v`) for the projection's capacity_wh
    /// computation.
    pub hardware: HardwareParams,
}

use victron_controller_dashboard_model::victron_controller::dashboard::mode::Mode as ModelMode;
use victron_controller_dashboard_model::victron_controller::dashboard::actuated::Actuated as ModelActuated;
use victron_controller_dashboard_model::victron_controller::dashboard::actuated_enum_name::ActuatedEnumName;
use victron_controller_dashboard_model::victron_controller::dashboard::actuated_f64::ActuatedF64 as ModelActuatedF64;
use victron_controller_dashboard_model::victron_controller::dashboard::actuated_i32::ActuatedI32 as ModelActuatedI32;
use victron_controller_dashboard_model::victron_controller::dashboard::actuated_schedule::ActuatedSchedule as ModelActuatedSchedule;
use victron_controller_dashboard_model::victron_controller::dashboard::actual_f64::ActualF64 as ModelActualF64;
use victron_controller_dashboard_model::victron_controller::dashboard::actual_i32::ActualI32 as ModelActualI32;
use victron_controller_dashboard_model::victron_controller::dashboard::bookkeeping::Bookkeeping as ModelBookkeeping;
use victron_controller_dashboard_model::victron_controller::dashboard::command::Command as ModelCommand;
use victron_controller_dashboard_model::victron_controller::dashboard::core_factor::CoreFactor as ModelCoreFactor;
use victron_controller_dashboard_model::victron_controller::dashboard::core_state::CoreState as ModelCoreState;
use victron_controller_dashboard_model::victron_controller::dashboard::cores_state::CoresState as ModelCoresState;
use victron_controller_dashboard_model::victron_controller::dashboard::decision::Decision as ModelDecision;
use victron_controller_dashboard_model::victron_controller::dashboard::decision_factor::DecisionFactor as ModelDecisionFactor;
use victron_controller_dashboard_model::victron_controller::dashboard::decisions::Decisions as ModelDecisions;
use victron_controller_dashboard_model::victron_controller::dashboard::debug_full_charge::DebugFullCharge as ModelDebugFullCharge;
use victron_controller_dashboard_model::victron_controller::dashboard::discharge_time::DischargeTime as ModelDischargeTime;
use victron_controller_dashboard_model::victron_controller::dashboard::charge_battery_extended_mode::ChargeBatteryExtendedMode as ModelCbeMode;
use victron_controller_dashboard_model::victron_controller::dashboard::extended_charge_mode::ExtendedChargeMode as ModelExtendedChargeMode;
use victron_controller_dashboard_model::victron_controller::dashboard::forecast_disagreement_strategy::ForecastDisagreementStrategy as ModelForecastStrategy;
use victron_controller_dashboard_model::victron_controller::dashboard::forecast_snapshot::ForecastSnapshot as ModelForecastSnapshot;
use victron_controller_dashboard_model::victron_controller::dashboard::forecasts::Forecasts as ModelForecasts;
use victron_controller_dashboard_model::victron_controller::dashboard::freshness::Freshness as ModelFreshness;
use victron_controller_dashboard_model::victron_controller::dashboard::knobs::Knobs as ModelKnobs;
use victron_controller_dashboard_model::victron_controller::dashboard::owner::Owner as ModelOwner;
use victron_controller_dashboard_model::victron_controller::dashboard::schedule_spec::ScheduleSpec as ModelScheduleSpec;
use victron_controller_dashboard_model::victron_controller::dashboard::sensor_meta::SensorMeta as ModelSensorMeta;
use victron_controller_dashboard_model::victron_controller::dashboard::sensors::Sensors as ModelSensors;
use victron_controller_dashboard_model::victron_controller::dashboard::target_phase::TargetPhase as ModelTargetPhase;
use victron_controller_dashboard_model::victron_controller::dashboard::timer::Timer as ModelTimer;
use victron_controller_dashboard_model::victron_controller::dashboard::timers::Timers as ModelTimers;
use victron_controller_dashboard_model::victron_controller::dashboard::pinned_register::PinnedRegister as ModelPinnedRegister;
use victron_controller_dashboard_model::victron_controller::dashboard::world_snapshot::WorldSnapshot;
// PR-ZDO-3: compensated-drain observability wire types.
use victron_controller_dashboard_model::victron_controller::dashboard::zappi_drain_branch::ZappiDrainBranch as ModelZappiDrainBranch;
use victron_controller_dashboard_model::victron_controller::dashboard::zappi_drain_sample::ZappiDrainSample as ModelZappiDrainSample;
use victron_controller_dashboard_model::victron_controller::dashboard::zappi_drain_snapshot_wire::ZappiDrainSnapshotWire as ModelZappiDrainSnapshotWire;
use victron_controller_dashboard_model::victron_controller::dashboard::zappi_drain_state::ZappiDrainState as ModelZappiDrainState;

// --- enums ----------------------------------------------------------------

fn freshness(f: Freshness) -> ModelFreshness {
    match f {
        Freshness::Unknown => ModelFreshness::Unknown,
        Freshness::Fresh => ModelFreshness::Fresh,
        Freshness::Stale => ModelFreshness::Stale,
        Freshness::Deprecated => ModelFreshness::Deprecated,
    }
}

fn phase(p: TargetPhase) -> ModelTargetPhase {
    match p {
        TargetPhase::Unset => ModelTargetPhase::Unset,
        TargetPhase::Pending => ModelTargetPhase::Pending,
        TargetPhase::Commanded => ModelTargetPhase::Commanded,
        TargetPhase::Confirmed => ModelTargetPhase::Confirmed,
    }
}

fn owner(o: Owner) -> ModelOwner {
    match o {
        Owner::Unset => ModelOwner::Unset,
        Owner::System => ModelOwner::System,
        Owner::Dashboard => ModelOwner::Dashboard,
        Owner::HaMqtt => ModelOwner::HaMqtt,
        Owner::WeatherSocPlanner => ModelOwner::WeatherSocPlanner,
        Owner::SetpointController => ModelOwner::SetpointController,
        Owner::CurrentLimitController => ModelOwner::CurrentLimitController,
        Owner::ScheduleController => ModelOwner::ScheduleController,
        Owner::ZappiController => ModelOwner::ZappiController,
        Owner::EddiController => ModelOwner::EddiController,
        Owner::FullChargeScheduler => ModelOwner::FullChargeScheduler,
        // PR-keep-batteries-charged.
        Owner::EssStateOverrideController => ModelOwner::EssStateOverrideController,
    }
}

fn discharge_time(d: DischargeTime) -> ModelDischargeTime {
    match d {
        DischargeTime::At0200 => ModelDischargeTime::At0200,
        DischargeTime::At2300 => ModelDischargeTime::At2300,
    }
}

fn debug_full_charge(d: DebugFullCharge) -> ModelDebugFullCharge {
    match d {
        DebugFullCharge::Forbid => ModelDebugFullCharge::Forbid,
        DebugFullCharge::Force => ModelDebugFullCharge::Force,
        DebugFullCharge::Auto => ModelDebugFullCharge::Auto,
    }
}

fn forecast_strategy(s: ForecastDisagreementStrategy) -> ModelForecastStrategy {
    match s {
        ForecastDisagreementStrategy::Max => ModelForecastStrategy::Max,
        ForecastDisagreementStrategy::Min => ModelForecastStrategy::Min,
        ForecastDisagreementStrategy::Mean => ModelForecastStrategy::Mean,
        ForecastDisagreementStrategy::SolcastIfAvailableElseMean => {
            ModelForecastStrategy::SolcastIfAvailableElseMean
        }
    }
}

fn cbe_mode(m: ChargeBatteryExtendedMode) -> ModelCbeMode {
    match m {
        ChargeBatteryExtendedMode::Auto => ModelCbeMode::Auto,
        ChargeBatteryExtendedMode::Forced => ModelCbeMode::Forced,
        ChargeBatteryExtendedMode::Disabled => ModelCbeMode::Disabled,
    }
}

/// PR-auto-extended-charge.
fn extended_charge_mode(m: ExtendedChargeMode) -> ModelExtendedChargeMode {
    match m {
        ExtendedChargeMode::Auto => ModelExtendedChargeMode::Auto,
        ExtendedChargeMode::Forced => ModelExtendedChargeMode::Forced,
        ExtendedChargeMode::Disabled => ModelExtendedChargeMode::Disabled,
    }
}

// --- time helpers ---------------------------------------------------------

/// Convert a monotonic Instant into an approximate wall-clock epoch-ms.
/// We don't have a real mapping from `Instant` to UNIX time, so we
/// approximate using the elapsed wall-clock since boot:
/// `now_epoch_ms - elapsed_ms_since(instant)`. Good enough for a
/// dashboard "last updated" indicator; not suitable for absolute
/// timestamps.
fn instant_to_epoch_ms(instant: std::time::Instant) -> i64 {
    let now = std::time::Instant::now();
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let delta_ms = now.saturating_duration_since(instant).as_millis() as i64;
    now_epoch - delta_ms
}

// --- wrappers -------------------------------------------------------------

fn actual_f64(a: &Actual<f64>) -> ModelActualF64 {
    ModelActualF64 {
        value: a.value,
        freshness: freshness(a.freshness),
        since_epoch_ms: instant_to_epoch_ms(a.since),
    }
}

fn actuated_i32(a: &Actuated<i32>) -> ModelActuatedI32 {
    ModelActuatedI32 {
        target_value: a.target.value,
        target_owner: owner(a.target.owner),
        target_phase: phase(a.target.phase),
        target_since_epoch_ms: instant_to_epoch_ms(a.target.since),
        actual: ModelActualI32 {
            value: a.actual.value,
            freshness: freshness(a.actual.freshness),
            since_epoch_ms: instant_to_epoch_ms(a.actual.since),
        },
    }
}

fn actuated_f64(a: &Actuated<f64>) -> ModelActuatedF64 {
    ModelActuatedF64 {
        target_value: a.target.value,
        target_owner: owner(a.target.owner),
        target_phase: phase(a.target.phase),
        target_since_epoch_ms: instant_to_epoch_ms(a.target.since),
        actual: actual_f64(&a.actual),
    }
}

fn actuated_zappi(a: &Actuated<ZappiMode>, actual_val: Option<&ZappiMode>, actual_fresh: Freshness, actual_since: std::time::Instant) -> ActuatedEnumName {
    ActuatedEnumName {
        target_value: a.target.value.map(|v| format!("{v:?}")),
        target_owner: owner(a.target.owner),
        target_phase: phase(a.target.phase),
        target_since_epoch_ms: instant_to_epoch_ms(a.target.since),
        actual_value: actual_val.map(|v| format!("{v:?}")),
        actual_freshness: freshness(actual_fresh),
        actual_since_epoch_ms: instant_to_epoch_ms(actual_since),
    }
}

fn actuated_eddi(a: &Actuated<EddiMode>, actual_val: Option<&EddiMode>, actual_fresh: Freshness, actual_since: std::time::Instant) -> ActuatedEnumName {
    ActuatedEnumName {
        target_value: a.target.value.map(|v| format!("{v:?}")),
        target_owner: owner(a.target.owner),
        target_phase: phase(a.target.phase),
        target_since_epoch_ms: instant_to_epoch_ms(a.target.since),
        actual_value: actual_val.map(|v| format!("{v:?}")),
        actual_freshness: freshness(actual_fresh),
        actual_since_epoch_ms: instant_to_epoch_ms(actual_since),
    }
}

fn schedule_spec(s: &ScheduleSpec) -> ModelScheduleSpec {
    ModelScheduleSpec {
        start_s: s.start_s,
        duration_s: s.duration_s,
        discharge: s.discharge,
        soc: s.soc,
        days: s.days,
    }
}

fn actuated_schedule(a: &Actuated<ScheduleSpec>) -> ModelActuatedSchedule {
    ModelActuatedSchedule {
        target: a.target.value.as_ref().map(schedule_spec),
        target_owner: owner(a.target.owner),
        target_phase: phase(a.target.phase),
        target_since_epoch_ms: instant_to_epoch_ms(a.target.since),
        actual: a.actual.value.as_ref().map(schedule_spec),
        actual_freshness: freshness(a.actual.freshness),
        actual_since_epoch_ms: instant_to_epoch_ms(a.actual.since),
    }
}

/// PR-ZDO-3: map the core `ZappiDrainBranch` enum onto the wire model enum.
fn zappi_drain_branch_to_model(b: victron_controller_core::types::ZappiDrainBranch) -> ModelZappiDrainBranch {
    use victron_controller_core::types::ZappiDrainBranch as CoreBranch;
    match b {
        CoreBranch::Tighten => ModelZappiDrainBranch::Tighten,
        CoreBranch::Relax => ModelZappiDrainBranch::Relax,
        CoreBranch::Bypass => ModelZappiDrainBranch::Bypass,
        CoreBranch::Disabled => ModelZappiDrainBranch::Disabled,
    }
}

/// PR-ZDO-3: project `core::world::ZappiDrainState` onto the wire
/// `ZappiDrainState`. `samples` is oldest-first (ring-buffer insertion
/// order); the renderer sorts by `captured_at_epoch_ms` at draw time to
/// handle non-monotonic wall-clock jumps.
fn zappi_drain_state_to_model(s: &victron_controller_core::world::ZappiDrainState) -> ModelZappiDrainState {
    use victron_controller_core::world::{ZappiDrainSnapshot, ZappiDrainSample};

    let snap_to_wire = |snap: &ZappiDrainSnapshot| ModelZappiDrainSnapshotWire {
        compensated_drain_w: snap.compensated_drain_w,
        branch: zappi_drain_branch_to_model(snap.branch),
        hard_clamp_engaged: snap.hard_clamp_engaged,
        hard_clamp_excess_w: snap.hard_clamp_excess_w,
        threshold_w: snap.threshold_w,
        hard_clamp_w: snap.hard_clamp_w,
        captured_at_epoch_ms: snap.captured_at_ms,
    };

    let sample_to_wire = |sample: &ZappiDrainSample| ModelZappiDrainSample {
        captured_at_epoch_ms: sample.captured_at_ms,
        compensated_drain_w: sample.compensated_drain_w,
        branch: zappi_drain_branch_to_model(sample.branch),
        hard_clamp_engaged: sample.hard_clamp_engaged,
    };

    ModelZappiDrainState {
        latest: s.latest.as_ref().map(snap_to_wire),
        samples: s.samples.iter().map(sample_to_wire).collect(),
    }
}

/// PR-pinned-registers: project the per-register state on
/// `world.pinned_registers` into the wire `PinnedRegister` shape.
/// Sorted by `path` ascending for a deterministic dashboard render —
/// the `BTreeMap` already iterates in sorted order so we only have to
/// `.collect`.
fn pinned_registers_to_model(
    world: &World,
) -> Vec<ModelPinnedRegister> {
    use victron_controller_core::types::PinnedStatus;
    world
        .pinned_registers
        .values()
        .map(|e| ModelPinnedRegister {
            path: e.path.as_ref().to_string(),
            target_value_str: format!("{}", e.target),
            current_value_str: e.actual.as_ref().map(|v| format!("{v}")),
            status: match e.status {
                PinnedStatus::Unknown => "unknown",
                PinnedStatus::Confirmed => "confirmed",
                PinnedStatus::Drifted => "drifted",
            }
            .to_string(),
            // The wire field is `i32`; drift counts comfortably stay
            // under that ceiling (one increment per hour at most).
            drift_count: i32::try_from(e.drift_count).unwrap_or(i32::MAX),
            last_drift_iso: e.last_drift_at.map(|d| d.to_string()),
            last_check_iso: e.last_check.map(|d| d.to_string()),
        })
        .collect()
}

// --- top-level mapping ----------------------------------------------------

#[must_use]
pub fn world_to_snapshot(world: &World, meta: &MetaContext) -> WorldSnapshot {
    let s = &world.sensors;
    let a = world.actuated();
    let k = &world.knobs;
    let b = &world.bookkeeping;
    let f = &world.typed_sensors;
    let now_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    let now_naive = chrono::Local::now().naive_local().to_string();

    WorldSnapshot {
        captured_at_epoch_ms: now_epoch,
        captured_at_naive_iso: now_naive,
        sensors: ModelSensors {
            battery_soc: actual_f64(&s.battery_soc),
            battery_soh: actual_f64(&s.battery_soh),
            battery_installed_capacity: actual_f64(&s.battery_installed_capacity),
            battery_dc_power: actual_f64(&s.battery_dc_power),
            mppt_power_0: actual_f64(&s.mppt_power_0),
            mppt_power_1: actual_f64(&s.mppt_power_1),
            soltaro_power: actual_f64(&s.soltaro_power),
            power_consumption: actual_f64(&s.power_consumption),
            grid_power: actual_f64(&s.grid_power),
            grid_voltage: actual_f64(&s.grid_voltage),
            grid_current: actual_f64(&s.grid_current),
            consumption_current: actual_f64(&s.consumption_current),
            offgrid_power: actual_f64(&s.offgrid_power),
            offgrid_current: actual_f64(&s.offgrid_current),
            vebus_input_current: actual_f64(&s.vebus_input_current),
            evcharger_ac_power: actual_f64(&s.evcharger_ac_power),
            evcharger_ac_current: actual_f64(&s.evcharger_ac_current),
            ess_state: actual_f64(&s.ess_state),
            outdoor_temperature: actual_f64(&s.outdoor_temperature),
            session_kwh: actual_f64(&s.session_kwh),
            ev_soc: actual_f64(&s.ev_soc),
            ev_charge_target: actual_f64(&s.ev_charge_target),
            // PR-ZD-1.
            heat_pump_power: actual_f64(&s.heat_pump_power),
            cooker_power: actual_f64(&s.cooker_power),
            mppt_0_operation_mode: actual_f64(&s.mppt_0_operation_mode),
            mppt_1_operation_mode: actual_f64(&s.mppt_1_operation_mode),
        },
        sensors_meta: sensors_meta(meta),
        actuated: ModelActuated {
            grid_setpoint: actuated_i32(a.grid_setpoint),
            input_current_limit: actuated_f64(a.input_current_limit),
            zappi_mode: actuated_zappi(
                a.zappi_mode,
                f.zappi_state.value.as_ref().map(|z| &z.zappi_mode),
                f.zappi_state.freshness,
                f.zappi_state.since,
            ),
            eddi_mode: actuated_eddi(
                a.eddi_mode,
                f.eddi_mode.value.as_ref(),
                f.eddi_mode.freshness,
                f.eddi_mode.since,
            ),
            schedule_0: actuated_schedule(a.schedule_0),
            schedule_1: actuated_schedule(a.schedule_1),
            // PR-keep-batteries-charged.
            ess_state_target: actuated_i32(a.ess_state_target),
        },
        knobs: knobs_to_model(k),
        bookkeeping: ModelBookkeeping {
            next_full_charge_iso: b.next_full_charge.map(|dt| dt.to_string()),
            above_soc_date_iso: b.above_soc_date.map(|d| d.to_string()),
            zappi_active: world.derived.zappi_active,
            charge_to_full_required: b.charge_to_full_required,
            soc_end_of_day_target: b.soc_end_of_day_target,
            effective_export_soc_threshold: b.effective_export_soc_threshold,
            battery_selected_soc_target: b.battery_selected_soc_target,
            charge_battery_extended_today: b.charge_battery_extended_today,
            charge_battery_extended_today_date_iso: b
                .charge_battery_extended_today_date
                .map(|d| d.to_string()),
            weather_soc_export_soc_threshold: b.weather_soc_export_soc_threshold,
            weather_soc_discharge_soc_target: b.weather_soc_discharge_soc_target,
            weather_soc_battery_soc_target: b.weather_soc_battery_soc_target,
            weather_soc_disable_night_grid_discharge: b.weather_soc_disable_night_grid_discharge,
            // PR-auto-extended-charge.
            auto_extended_today: b.auto_extended_today,
            auto_extended_today_date_iso: b
                .auto_extended_today_date
                .map(|d| d.to_string()),
        },
        forecasts: ModelForecasts {
            solcast: f.forecast_solcast.as_ref().map(forecast_snapshot),
            forecast_solar: f.forecast_forecast_solar.as_ref().map(forecast_snapshot),
            open_meteo: f.forecast_open_meteo.as_ref().map(forecast_snapshot),
            baseline: f.forecast_baseline.as_ref().map(forecast_snapshot),
        },
        decisions: decisions_to_model(&world.decisions),
        cores_state: cores_state_to_model(&world.cores_state),
        timers: timers_to_model(&world.timers),
        // PR-tz-from-victron: surface the Victron-supplied display TZ.
        timezone: world.timezone.clone(),
        // PR-soc-chart / PR-soc-chart-segments: history + piecewise
        // projection. Reads the in-memory ring synchronously; safe to
        // call under the world-lock because the store has its own
        // internal lock.
        soc_chart: crate::dashboard::convert_soc_chart::compute_soc_chart(
            world,
            &meta.soc_history.snapshot_blocking(),
            meta.hardware,
            meta.controller_params,
            now_epoch,
        ),
        // PR-schedule-section: forward-looking controller actions sorted
        // by next_fire ascending. Pure compute; reads only world state +
        // `now_epoch`.
        scheduled_actions: crate::dashboard::convert_schedule::compute_scheduled_actions(
            world, now_epoch,
        ),
        // PR-pinned-registers: per-register drift state.
        pinned_registers: pinned_registers_to_model(world),
        // PR-baseline-forecast: today's sunrise/sunset. Returned as
        // `None` when the world-state has never been seeded OR when the
        // last successful update is older than
        // `world::SUNRISE_SUNSET_FRESHNESS` (3 h). Both paths render
        // the same em-dash row on the dashboard, which is the right
        // call: from the operator's perspective "we don't know" and
        // "we last knew this 12 h ago" are equivalent.
        sunrise_local_iso: fresh_sunrise_sunset(world).0,
        sunset_local_iso: fresh_sunrise_sunset(world).1,
        // PR-ZDO-3: compensated-drain observability ring buffer + latest
        // snapshot. `latest` is None until the first controller tick runs;
        // `samples` is oldest-first (ring-buffer insertion order).
        zappi_drain_state: zappi_drain_state_to_model(&world.zappi_drain_state),
    }
}

/// Today's sunrise / sunset as ISO-formatted local-time strings, OR
/// `None` when the freshness window has elapsed (or values were never
/// observed). See `world::SUNRISE_SUNSET_FRESHNESS` for the threshold.
fn fresh_sunrise_sunset(world: &World) -> (Option<String>, Option<String>) {
    use std::time::Instant;
    fresh_sunrise_sunset_impl(
        world.sunrise_sunset_updated_at,
        world.sunrise,
        world.sunset,
        Instant::now(),
    )
}

/// Pure helper backing [`fresh_sunrise_sunset`] — split out so tests
/// can drive `now` without poking into `Instant::now()`.
fn fresh_sunrise_sunset_impl(
    updated_at: Option<std::time::Instant>,
    sunrise: Option<chrono::NaiveDateTime>,
    sunset: Option<chrono::NaiveDateTime>,
    now: std::time::Instant,
) -> (Option<String>, Option<String>) {
    let fresh = match updated_at {
        Some(at) => now.saturating_duration_since(at)
            <= victron_controller_core::world::SUNRISE_SUNSET_FRESHNESS,
        None => false,
    };
    if !fresh {
        return (None, None);
    }
    (sunrise.map(|dt| dt.to_string()), sunset.map(|dt| dt.to_string()))
}

/// Convert the per-timer observability state in `world.timers` into the
/// wire `Timers` struct. Sorts by `TimerId::name()` so the dashboard
/// renders in a stable order. Timers that have never fired still appear
/// (with `last_fire_epoch_ms: None`, `status: idle`) so the dashboard
/// table shows the full known set; this is enumerated from
/// `TimerId::ALL` rather than `world.timers.entries` to keep the row
/// list deterministic across reboots.
fn timers_to_model(t: &victron_controller_core::world::Timers) -> ModelTimers {
    let mut entries: Vec<ModelTimer> = TimerId::ALL
        .iter()
        .map(|&id| {
            let entry = t.entries.get(&id);
            let period_ms: i64 = if let Some(e) = entry {
                e.period.as_millis().try_into().unwrap_or(i64::MAX)
            } else {
                0
            };
            let status = if let Some(e) = entry {
                e.status.name().to_string()
            } else {
                victron_controller_core::types::TimerStatus::Idle
                    .name()
                    .to_string()
            };
            ModelTimer {
                id: id.name().to_string(),
                description: id.description().to_string(),
                period_ms,
                last_fire_epoch_ms: entry.and_then(|e| e.last_fire_epoch_ms),
                next_fire_epoch_ms: entry.and_then(|e| e.next_fire_epoch_ms),
                status,
            }
        })
        .collect();
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    ModelTimers { entries }
}

fn cores_state_to_model(c: &victron_controller_core::world::CoresState) -> ModelCoresState {
    let factors = |fs: &[victron_controller_core::world::CoreFactor]| -> Vec<ModelCoreFactor> {
        fs.iter()
            .map(|f| ModelCoreFactor {
                name: f.id.clone(),
                value: f.value.clone(),
            })
            .collect()
    };
    ModelCoresState {
        cores: c
            .cores
            .iter()
            .map(|s| ModelCoreState {
                id: s.id.clone(),
                depends_on: s.depends_on.clone(),
                last_run_outcome: s.last_run_outcome.clone(),
                last_payload: s.last_payload.clone(),
                last_inputs: factors(&s.last_inputs),
                last_outputs: factors(&s.last_outputs),
            })
            .collect(),
        topo_order: c.topo_order.clone(),
    }
}

/// Static provenance per sensor: where the value comes from (`dbus` /
/// `open-meteo` / ...), a source-specific identifier, and the timing
/// contract (how often we refresh it, after how long we treat it as
/// Stale). Mirrors `dbus::subscriber::routing_table` but keyed by the
/// snapshot field name so the dashboard can show Origin + a copy
/// button + timing columns.
fn sensors_meta(ctx: &MetaContext) -> BTreeMap<String, ModelSensorMeta> {
    let s = &ctx.services;
    let om_cadence_ms: i64 = ctx.open_meteo_cadence.as_millis().try_into().unwrap_or(i64::MAX);

    // Per-sensor reseed cadence + staleness — both authoritative on
    // `SensorId` (PR-cadence-per-sensor). Drives the dashboard's
    // Origin / cadence / staleness columns.
    let staleness_ms = |id: SensorId| -> i64 {
        id.freshness_threshold()
            .as_millis()
            .try_into()
            .unwrap_or(i64::MAX)
    };

    let dbus = |svc: &str, path: &str, id: SensorId| ModelSensorMeta {
        origin: "dbus".to_string(),
        identifier: format!("{svc}{path}"),
        cadence_ms: id
            .reseed_cadence()
            .as_millis()
            .try_into()
            .unwrap_or(i64::MAX),
        staleness_ms: staleness_ms(id),
    };
    let mut m: BTreeMap<String, ModelSensorMeta> = BTreeMap::new();
    m.insert(
        "battery_soc".into(),
        dbus(&s.battery, "/Soc", SensorId::BatterySoc),
    );
    m.insert(
        "battery_soh".into(),
        dbus(&s.battery, "/Soh", SensorId::BatterySoh),
    );
    m.insert(
        "battery_installed_capacity".into(),
        dbus(
            &s.battery,
            "/InstalledCapacity",
            SensorId::BatteryInstalledCapacity,
        ),
    );
    m.insert(
        "battery_dc_power".into(),
        dbus(&s.battery, "/Dc/0/Power", SensorId::BatteryDcPower),
    );
    m.insert(
        "mppt_power_0".into(),
        dbus(&s.mppt_0, "/Yield/Power", SensorId::MpptPower0),
    );
    m.insert(
        "mppt_power_1".into(),
        dbus(&s.mppt_1, "/Yield/Power", SensorId::MpptPower1),
    );
    m.insert(
        "soltaro_power".into(),
        dbus(&s.pvinverter_soltaro, "/Ac/Power", SensorId::SoltaroPower),
    );
    m.insert(
        "power_consumption".into(),
        dbus(
            &s.system,
            "/Ac/Consumption/L1/Power",
            SensorId::PowerConsumption,
        ),
    );
    m.insert(
        "grid_power".into(),
        dbus(&s.system, "/Ac/Grid/L1/Power", SensorId::GridPower),
    );
    m.insert(
        "grid_voltage".into(),
        dbus(&s.grid, "/Ac/L1/Voltage", SensorId::GridVoltage),
    );
    m.insert(
        "grid_current".into(),
        dbus(&s.grid, "/Ac/L1/Current", SensorId::GridCurrent),
    );
    m.insert(
        "consumption_current".into(),
        dbus(
            &s.system,
            "/Ac/Consumption/L1/Current",
            SensorId::ConsumptionCurrent,
        ),
    );
    m.insert(
        "offgrid_power".into(),
        dbus(&s.vebus, "/Ac/Out/L1/P", SensorId::OffgridPower),
    );
    m.insert(
        "offgrid_current".into(),
        dbus(&s.vebus, "/Ac/Out/L1/I", SensorId::OffgridCurrent),
    );
    m.insert(
        "vebus_input_current".into(),
        dbus(
            &s.vebus,
            "/Ac/ActiveIn/L1/I",
            SensorId::VebusInputCurrent,
        ),
    );
    m.insert(
        "evcharger_ac_power".into(),
        dbus(&s.evcharger, "/Ac/Power", SensorId::EvchargerAcPower),
    );
    m.insert(
        "evcharger_ac_current".into(),
        dbus(
            &s.evcharger,
            "/Ac/Current",
            SensorId::EvchargerAcCurrent,
        ),
    );
    m.insert(
        "ess_state".into(),
        dbus(
            &s.settings,
            "/Settings/CGwacs/BatteryLife/State",
            SensorId::EssState,
        ),
    );
    // PR-matter-outdoor-temp: when the Matter MQTT bridge is configured,
    // surface its origin/topic/cadence on the dashboard. Open-Meteo
    // stays running as a silent fallback (~30 min cadence, 40 min
    // staleness — either source keeps the sensor fresh).
    m.insert(
        "outdoor_temperature".into(),
        if let Some(topic) = &ctx.matter_outdoor_topic {
            ModelSensorMeta {
                origin: "matter-mqtt".to_string(),
                identifier: topic.clone(),
                // Meross publishes ~every minute when alive.
                cadence_ms: 60_000,
                staleness_ms: staleness_ms(SensorId::OutdoorTemperature),
            }
        } else {
            ModelSensorMeta {
                origin: "open-meteo".to_string(),
                identifier: "api.open-meteo.com/v1/forecast?current=temperature_2m".to_string(),
                cadence_ms: om_cadence_ms,
                staleness_ms: staleness_ms(SensorId::OutdoorTemperature),
            }
        },
    );
    // Zappi session kWh — sourced from the myenergi cloud `che` field
    // on the same poll cadence as the typed Zappi state. PR-session-kwh-sensor.
    m.insert(
        "session_kwh".into(),
        ModelSensorMeta {
            origin: "myenergi".to_string(),
            identifier: "zappi/che".to_string(),
            cadence_ms: SensorId::SessionKwh
                .reseed_cadence()
                .as_millis()
                .try_into()
                .unwrap_or(i64::MAX),
            staleness_ms: staleness_ms(SensorId::SessionKwh),
        },
    );
    // PR-ev-soc-sensor: surfaced only when the bridge is configured.
    // When disabled the row's value stays `Unknown`, but the meta entry
    // is omitted so the operator isn't misled into thinking we have a
    // wire it'll never see traffic on.
    if let Some(topic) = &ctx.ev_soc_discovery_topic {
        m.insert(
            "ev_soc".into(),
            ModelSensorMeta {
                origin: "ext-mqtt".to_string(),
                identifier: topic.clone(),
                cadence_ms: SensorId::EvSoc
                    .reseed_cadence()
                    .as_millis()
                    .try_into()
                    .unwrap_or(i64::MAX),
                staleness_ms: staleness_ms(SensorId::EvSoc),
            },
        );
    }
    // PR-auto-extended-charge: same provenance shape as ev_soc.
    if let Some(topic) = &ctx.ev_charge_target_discovery_topic {
        m.insert(
            "ev_charge_target".into(),
            ModelSensorMeta {
                origin: "ext-mqtt".to_string(),
                identifier: topic.clone(),
                cadence_ms: SensorId::EvChargeTarget
                    .reseed_cadence()
                    .as_millis()
                    .try_into()
                    .unwrap_or(i64::MAX),
                staleness_ms: staleness_ms(SensorId::EvChargeTarget),
            },
        );
    }
    // PR-ZD-1: zigbee2mqtt push sensors. Surfaced only when configured.
    if let Some(topic) = &ctx.heat_pump_topic {
        m.insert(
            "heat_pump_power".into(),
            ModelSensorMeta {
                origin: "zigbee2mqtt".to_string(),
                identifier: topic.clone(),
                cadence_ms: SensorId::HeatPumpPower
                    .reseed_cadence()
                    .as_millis()
                    .try_into()
                    .unwrap_or(i64::MAX),
                staleness_ms: staleness_ms(SensorId::HeatPumpPower),
            },
        );
    }
    if let Some(topic) = &ctx.cooker_topic {
        m.insert(
            "cooker_power".into(),
            ModelSensorMeta {
                origin: "zigbee2mqtt".to_string(),
                identifier: topic.clone(),
                cadence_ms: SensorId::CookerPower
                    .reseed_cadence()
                    .as_millis()
                    .try_into()
                    .unwrap_or(i64::MAX),
                staleness_ms: staleness_ms(SensorId::CookerPower),
            },
        );
    }
    // PR-ZD-1: MPPT op-mode — D-Bus, provenance from routing table.
    m.insert(
        "mppt_0_operation_mode".into(),
        dbus(&s.mppt_0, "/MppOperationMode", SensorId::Mppt0OperationMode),
    );
    m.insert(
        "mppt_1_operation_mode".into(),
        dbus(&s.mppt_1, "/MppOperationMode", SensorId::Mppt1OperationMode),
    );
    m
}

fn decision(d: &Decision) -> ModelDecision {
    ModelDecision {
        summary: d.summary.clone(),
        factors: d
            .factors
            .iter()
            .map(|f| ModelDecisionFactor {
                name: f.name.clone(),
                value: f.value.clone(),
            })
            .collect(),
    }
}

fn decisions_to_model(d: &victron_controller_core::world::Decisions) -> ModelDecisions {
    ModelDecisions {
        grid_setpoint: d.grid_setpoint.as_ref().map(decision),
        input_current_limit: d.input_current_limit.as_ref().map(decision),
        schedule_0: d.schedule_0.as_ref().map(decision),
        schedule_1: d.schedule_1.as_ref().map(decision),
        zappi_mode: d.zappi_mode.as_ref().map(decision),
        eddi_mode: d.eddi_mode.as_ref().map(decision),
        weather_soc: d.weather_soc.as_ref().map(decision),
    }
}

fn knobs_to_model(k: &Knobs) -> ModelKnobs {
    ModelKnobs {
        force_disable_export: k.force_disable_export,
        export_soc_threshold: k.export_soc_threshold,
        discharge_soc_target: k.discharge_soc_target,
        battery_soc_target: k.battery_soc_target,
        full_charge_discharge_soc_target: k.full_charge_discharge_soc_target,
        full_charge_export_soc_threshold: k.full_charge_export_soc_threshold,
        discharge_time: discharge_time(k.discharge_time),
        debug_full_charge: debug_full_charge(k.debug_full_charge),
        pessimism_multiplier_modifier: k.pessimism_multiplier_modifier,
        disable_night_grid_discharge: k.disable_night_grid_discharge,
        charge_car_boost: k.charge_car_boost,
        // PR-auto-extended-charge.
        charge_car_extended_mode: extended_charge_mode(k.charge_car_extended_mode),
        zappi_current_target: k.zappi_current_target,
        zappi_limit: k.zappi_limit,
        zappi_emergency_margin: k.zappi_emergency_margin,
        // A-34 / A-35: saturate u32→i32 instead of wrap-on-cast. With
        // the core-side SAFE_MAX_GRID_LIMIT_W = 10_000 enforcement
        // these shouldn't actually hit the saturation, but the
        // dashboard wire types are i32 and we don't want to sign-flip
        // display values if a knob escapes validation.
        grid_export_limit_w: i32::try_from(k.grid_export_limit_w).unwrap_or(i32::MAX),
        grid_import_limit_w: i32::try_from(k.grid_import_limit_w).unwrap_or(i32::MAX),
        allow_battery_to_car: k.allow_battery_to_car,
        eddi_enable_soc: k.eddi_enable_soc,
        eddi_disable_soc: k.eddi_disable_soc,
        eddi_dwell_s: i32::try_from(k.eddi_dwell_s).unwrap_or(i32::MAX),
        weathersoc_winter_temperature_threshold: k.weathersoc_winter_temperature_threshold,
        weathersoc_low_energy_threshold: k.weathersoc_low_energy_threshold,
        weathersoc_ok_energy_threshold: k.weathersoc_ok_energy_threshold,
        weathersoc_high_energy_threshold: k.weathersoc_high_energy_threshold,
        weathersoc_too_much_energy_threshold: k.weathersoc_too_much_energy_threshold,
        writes_enabled: k.writes_enabled,
        forecast_disagreement_strategy: forecast_strategy(k.forecast_disagreement_strategy),
        charge_battery_extended_mode: cbe_mode(k.charge_battery_extended_mode),
        export_soc_threshold_mode: mode(k.export_soc_threshold_mode),
        discharge_soc_target_mode: mode(k.discharge_soc_target_mode),
        battery_soc_target_mode: mode(k.battery_soc_target_mode),
        disable_night_grid_discharge_mode: mode(k.disable_night_grid_discharge_mode),
        inverter_safe_discharge_enable: k.inverter_safe_discharge_enable,
        // PR-baseline-forecast.
        baseline_winter_start_mm_dd: i32::try_from(k.baseline_winter_start_mm_dd)
            .unwrap_or(i32::MAX),
        baseline_winter_end_mm_dd: i32::try_from(k.baseline_winter_end_mm_dd)
            .unwrap_or(i32::MAX),
        baseline_wh_per_hour_winter: k.baseline_wh_per_hour_winter,
        baseline_wh_per_hour_summer: k.baseline_wh_per_hour_summer,
        // PR-keep-batteries-charged.
        keep_batteries_charged_during_full_charge: k.keep_batteries_charged_during_full_charge,
        sunrise_sunset_offset_min: i32::try_from(k.sunrise_sunset_offset_min)
            .unwrap_or(i32::MAX),
        full_charge_defer_to_next_sunday: k.full_charge_defer_to_next_sunday,
        full_charge_snap_back_max_weekday: i32::try_from(k.full_charge_snap_back_max_weekday)
            .unwrap_or(i32::MAX),
        // PR-ZD-2: compensated battery-drain feedback loop.
        zappi_battery_drain_threshold_w: i32::try_from(k.zappi_battery_drain_threshold_w)
            .unwrap_or(i32::MAX),
        zappi_battery_drain_relax_step_w: i32::try_from(k.zappi_battery_drain_relax_step_w)
            .unwrap_or(i32::MAX),
        zappi_battery_drain_kp: k.zappi_battery_drain_kp,
        zappi_battery_drain_target_w: k.zappi_battery_drain_target_w,
        zappi_battery_drain_hard_clamp_w: i32::try_from(k.zappi_battery_drain_hard_clamp_w)
            .unwrap_or(i32::MAX),
        // PR-ZDP-1: MPPT probe offset.
        zappi_battery_drain_mppt_probe_w: i32::try_from(k.zappi_battery_drain_mppt_probe_w)
            .unwrap_or(i32::MAX),
    }
}

fn mode(m: victron_controller_core::knobs::Mode) -> ModelMode {
    use victron_controller_core::knobs::Mode as CoreMode;
    match m {
        CoreMode::Weather => ModelMode::Weather,
        CoreMode::Forced => ModelMode::Forced,
    }
}

fn forecast_snapshot(f: &victron_controller_core::world::ForecastSnapshot) -> ModelForecastSnapshot {
    ModelForecastSnapshot {
        today_kwh: f.today_kwh,
        tomorrow_kwh: f.tomorrow_kwh,
        fetched_at_epoch_ms: instant_to_epoch_ms(f.fetched_at),
        // PR-soc-chart-solar: surface the hourly array so the
        // dashboard's SoC chart can subdivide Natural segments.
        hourly_kwh: f.hourly_kwh.clone(),
    }
}

// --- command decode -------------------------------------------------------

/// Map a `World` to just the six actuated entities it exposes — a
/// helper so the snapshot converter doesn't duplicate the field list.
fn world_actuated(world: &World) -> WorldActuatedRefs<'_> {
    WorldActuatedRefs {
        grid_setpoint: &world.grid_setpoint,
        input_current_limit: &world.input_current_limit,
        zappi_mode: &world.zappi_mode,
        eddi_mode: &world.eddi_mode,
        schedule_0: &world.schedule_0,
        schedule_1: &world.schedule_1,
        ess_state_target: &world.ess_state_target,
    }
}

struct WorldActuatedRefs<'a> {
    grid_setpoint: &'a Actuated<i32>,
    input_current_limit: &'a Actuated<f64>,
    zappi_mode: &'a Actuated<ZappiMode>,
    eddi_mode: &'a Actuated<EddiMode>,
    schedule_0: &'a Actuated<ScheduleSpec>,
    schedule_1: &'a Actuated<ScheduleSpec>,
    ess_state_target: &'a Actuated<i32>,
}

trait WorldActuatedAccess {
    fn actuated(&self) -> WorldActuatedRefs<'_>;
}
impl WorldActuatedAccess for World {
    fn actuated(&self) -> WorldActuatedRefs<'_> {
        world_actuated(self)
    }
}

/// Decode an incoming dashboard command into a core Event::Command.
/// Unknown knob names return None so the caller can 400.
#[must_use]
pub fn command_to_event(cmd: &ModelCommand, at: std::time::Instant) -> Option<Event> {
    use victron_controller_dashboard_model::victron_controller::dashboard::command::Command as C;
    let core_cmd = match cmd {
        C::SetBoolKnob(c) => {
            let id = knob_id_from_name(&c.knob_name)?;
            Command::Knob { id, value: KnobValue::Bool(c.value) }
        }
        C::SetFloatKnob(c) => {
            let id = knob_id_from_name(&c.knob_name)?;
            Command::Knob { id, value: KnobValue::Float(c.value) }
        }
        C::SetUintKnob(c) => {
            let id = knob_id_from_name(&c.knob_name)?;
            let v = u32::try_from(c.value).ok()?;
            Command::Knob { id, value: KnobValue::Uint32(v) }
        }
        C::SetDischargeTime(c) => Command::Knob {
            id: KnobId::DischargeTime,
            value: KnobValue::DischargeTime(match c.value {
                ModelDischargeTime::At0200 => DischargeTime::At0200,
                ModelDischargeTime::At2300 => DischargeTime::At2300,
            }),
        },
        C::SetDebugFullCharge(c) => Command::Knob {
            id: KnobId::DebugFullCharge,
            value: KnobValue::DebugFullCharge(match c.value {
                ModelDebugFullCharge::Forbid => DebugFullCharge::Forbid,
                ModelDebugFullCharge::Force => DebugFullCharge::Force,
                ModelDebugFullCharge::Auto => DebugFullCharge::Auto,
            }),
        },
        C::SetForecastDisagreementStrategy(c) => Command::Knob {
            id: KnobId::ForecastDisagreementStrategy,
            value: KnobValue::ForecastDisagreementStrategy(match c.value {
                ModelForecastStrategy::Max => ForecastDisagreementStrategy::Max,
                ModelForecastStrategy::Min => ForecastDisagreementStrategy::Min,
                ModelForecastStrategy::Mean => ForecastDisagreementStrategy::Mean,
                ModelForecastStrategy::SolcastIfAvailableElseMean => {
                    ForecastDisagreementStrategy::SolcastIfAvailableElseMean
                }
            }),
        },
        C::SetChargeBatteryExtendedMode(c) => Command::Knob {
            id: KnobId::ChargeBatteryExtendedMode,
            value: KnobValue::ChargeBatteryExtendedMode(match c.value {
                ModelCbeMode::Auto => ChargeBatteryExtendedMode::Auto,
                ModelCbeMode::Forced => ChargeBatteryExtendedMode::Forced,
                ModelCbeMode::Disabled => ChargeBatteryExtendedMode::Disabled,
            }),
        },
        // PR-auto-extended-charge.
        C::SetExtendedChargeMode(c) => Command::Knob {
            id: KnobId::ChargeCarExtendedMode,
            value: KnobValue::ExtendedChargeMode(match c.value {
                ModelExtendedChargeMode::Auto => ExtendedChargeMode::Auto,
                ModelExtendedChargeMode::Forced => ExtendedChargeMode::Forced,
                ModelExtendedChargeMode::Disabled => ExtendedChargeMode::Disabled,
            }),
        },
        C::SetMode(c) => {
            let id = knob_id_from_name(&c.knob_name)?;
            let core_mode = match c.value {
                ModelMode::Weather => victron_controller_core::knobs::Mode::Weather,
                ModelMode::Forced => victron_controller_core::knobs::Mode::Forced,
            };
            Command::Knob { id, value: KnobValue::Mode(core_mode) }
        }
        C::SetKillSwitch(c) => Command::KillSwitch(c.value),
        C::SetBookkeeping(c) => {
            use victron_controller_dashboard_model::victron_controller::dashboard::bookkeeping_key::BookkeepingKey as ModelBkKey;
            use victron_controller_dashboard_model::victron_controller::dashboard::bookkeeping_value::BookkeepingValue as ModelBkValue;
            let key = match c.key {
                ModelBkKey::NextFullCharge => BookkeepingKey::NextFullCharge,
                ModelBkKey::AboveSocDate => BookkeepingKey::AboveSocDate,
            };
            let value = match &c.value {
                ModelBkValue::NaiveDateTime(v) => {
                    // Accept both `T`-separated and space-separated wire
                    // forms; chrono's default Display for NaiveDateTime
                    // emits a space, while HTML5 datetime-local inputs
                    // emit `T`.
                    let parsed =
                        chrono::NaiveDateTime::parse_from_str(&v.iso, "%Y-%m-%dT%H:%M:%S")
                            .or_else(|_| {
                                chrono::NaiveDateTime::parse_from_str(
                                    &v.iso,
                                    "%Y-%m-%dT%H:%M",
                                )
                            })
                            .or_else(|_| {
                                chrono::NaiveDateTime::parse_from_str(
                                    &v.iso,
                                    "%Y-%m-%d %H:%M:%S",
                                )
                            })
                            .ok()?;
                    BookkeepingValue::NaiveDateTime(parsed)
                }
                ModelBkValue::Cleared(_) => BookkeepingValue::Cleared,
            };
            Command::SetBookkeeping { key, value }
        }
    };
    Some(Event::Command {
        command: core_cmd,
        owner: Owner::Dashboard,
        at,
    })
}

/// Exhaustive `snake_case` knob-name → KnobId mapping. Mirrors the MQTT
/// serializer's table to keep one wire vocabulary. Kept here so the
/// dashboard + MQTT subscribers can evolve independently if needed.
fn knob_id_from_name(n: &str) -> Option<KnobId> {
    Some(match n {
        "force_disable_export" => KnobId::ForceDisableExport,
        "export_soc_threshold" => KnobId::ExportSocThreshold,
        "discharge_soc_target" => KnobId::DischargeSocTarget,
        "battery_soc_target" => KnobId::BatterySocTarget,
        "full_charge_discharge_soc_target" => KnobId::FullChargeDischargeSocTarget,
        "full_charge_export_soc_threshold" => KnobId::FullChargeExportSocThreshold,
        "discharge_time" => KnobId::DischargeTime,
        "debug_full_charge" => KnobId::DebugFullCharge,
        "pessimism_multiplier_modifier" => KnobId::PessimismMultiplierModifier,
        "disable_night_grid_discharge" => KnobId::DisableNightGridDischarge,
        "charge_car_boost" => KnobId::ChargeCarBoost,
        // PR-auto-extended-charge.
        "charge_car_extended_mode" => KnobId::ChargeCarExtendedMode,
        "zappi_current_target" => KnobId::ZappiCurrentTarget,
        "zappi_limit" => KnobId::ZappiLimit,
        "zappi_emergency_margin" => KnobId::ZappiEmergencyMargin,
        "grid_export_limit_w" => KnobId::GridExportLimitW,
        "grid_import_limit_w" => KnobId::GridImportLimitW,
        "allow_battery_to_car" => KnobId::AllowBatteryToCar,
        "eddi_enable_soc" => KnobId::EddiEnableSoc,
        "eddi_disable_soc" => KnobId::EddiDisableSoc,
        "eddi_dwell_s" => KnobId::EddiDwellS,
        "weathersoc_winter_temperature_threshold" => KnobId::WeathersocWinterTemperatureThreshold,
        "weathersoc_low_energy_threshold" => KnobId::WeathersocLowEnergyThreshold,
        "weathersoc_ok_energy_threshold" => KnobId::WeathersocOkEnergyThreshold,
        "weathersoc_high_energy_threshold" => KnobId::WeathersocHighEnergyThreshold,
        "weathersoc_too_much_energy_threshold" => KnobId::WeathersocTooMuchEnergyThreshold,
        "forecast_disagreement_strategy" => KnobId::ForecastDisagreementStrategy,
        "charge_battery_extended_mode" => KnobId::ChargeBatteryExtendedMode,
        "export_soc_threshold_mode" => KnobId::ExportSocThresholdMode,
        "discharge_soc_target_mode" => KnobId::DischargeSocTargetMode,
        "battery_soc_target_mode" => KnobId::BatterySocTargetMode,
        "disable_night_grid_discharge_mode" => KnobId::DisableNightGridDischargeMode,
        "inverter_safe_discharge_enable" => KnobId::InverterSafeDischargeEnable,
        // PR-baseline-forecast.
        "baseline_winter_start_mm_dd" => KnobId::BaselineWinterStartMmDd,
        "baseline_winter_end_mm_dd" => KnobId::BaselineWinterEndMmDd,
        "baseline_wh_per_hour_winter" => KnobId::BaselineWhPerHourWinter,
        "baseline_wh_per_hour_summer" => KnobId::BaselineWhPerHourSummer,
        // PR-keep-batteries-charged.
        "keep_batteries_charged_during_full_charge" => {
            KnobId::KeepBatteriesChargedDuringFullCharge
        }
        "sunrise_sunset_offset_min" => KnobId::SunriseSunsetOffsetMin,
        "full_charge_defer_to_next_sunday" => KnobId::FullChargeDeferToNextSunday,
        "full_charge_snap_back_max_weekday" => KnobId::FullChargeSnapBackMaxWeekday,
        // PR-ZD-2: compensated battery-drain feedback loop.
        "zappi_battery_drain_threshold_w" => KnobId::ZappiBatteryDrainThresholdW,
        "zappi_battery_drain_relax_step_w" => KnobId::ZappiBatteryDrainRelaxStepW,
        "zappi_battery_drain_kp" => KnobId::ZappiBatteryDrainKp,
        "zappi_battery_drain_target_w" => KnobId::ZappiBatteryDrainTargetW,
        "zappi_battery_drain_hard_clamp_w" => KnobId::ZappiBatteryDrainHardClampW,
        // PR-ZDP-1.
        "zappi_battery_drain_mppt_probe_w" => KnobId::ZappiBatteryDrainMpptProbeW,
        _ => return None,
    })
}

#[cfg(test)]
mod snapshot_new_sensors_tests {
    use super::*;
    use std::time::{Duration, Instant};
    use victron_controller_core::topology::{ControllerParams, HardwareParams};
    use victron_controller_core::world::World;

    fn test_meta(heat_pump_topic: Option<&str>, cooker_topic: Option<&str>) -> MetaContext {
        MetaContext {
            services: crate::config::DbusServices::default_venus_3_70(),
            open_meteo_cadence: Duration::from_secs(1800),
            controller_params: ControllerParams::defaults(),
            matter_outdoor_topic: None,
            ev_soc_discovery_topic: None,
            ev_charge_target_discovery_topic: None,
            heat_pump_topic: heat_pump_topic.map(str::to_owned),
            cooker_topic: cooker_topic.map(str::to_owned),
            soc_history: crate::dashboard::soc_history::SocHistoryStore::new(),
            hardware: HardwareParams::defaults(),
        }
    }

    /// PR-ZD-1 / D02: world_to_snapshot surfaces heat_pump_power,
    /// cooker_power, mppt_0_operation_mode, and mppt_1_operation_mode
    /// including their sensors_meta entries.
    #[test]
    fn dashboard_snapshot_surfaces_new_sensors() {
        let now = Instant::now();
        let mut world = World::fresh_boot(now);

        world.sensors.heat_pump_power.on_reading(1234.5, now);
        world.sensors.cooker_power.on_reading(789.0, now);
        world.sensors.mppt_0_operation_mode.on_reading(2.0, now);
        world.sensors.mppt_1_operation_mode.on_reading(1.0, now);

        let meta = test_meta(
            Some("zigbee2mqtt/nodon-mtr-heat-pump"),
            Some("zigbee2mqtt/nodon-mtr-stove"),
        );
        let snap = world_to_snapshot(&world, &meta);

        assert_eq!(snap.sensors.heat_pump_power.value, Some(1234.5));
        assert_eq!(snap.sensors.cooker_power.value, Some(789.0));
        assert_eq!(snap.sensors.mppt_0_operation_mode.value, Some(2.0));
        assert_eq!(snap.sensors.mppt_1_operation_mode.value, Some(1.0));

        // All four must appear in sensors_meta.
        assert!(
            snap.sensors_meta.contains_key("heat_pump_power"),
            "heat_pump_power missing from sensors_meta"
        );
        assert!(
            snap.sensors_meta.contains_key("cooker_power"),
            "cooker_power missing from sensors_meta"
        );
        assert!(
            snap.sensors_meta.contains_key("mppt_0_operation_mode"),
            "mppt_0_operation_mode missing from sensors_meta"
        );
        assert!(
            snap.sensors_meta.contains_key("mppt_1_operation_mode"),
            "mppt_1_operation_mode missing from sensors_meta"
        );

        // HP/cooker provenance must reference the configured topic.
        let hp_meta = &snap.sensors_meta["heat_pump_power"];
        assert!(
            hp_meta.identifier.contains("nodon-mtr-heat-pump"),
            "heat_pump_power identifier={:?} does not contain 'nodon-mtr-heat-pump'",
            hp_meta.identifier,
        );
    }

    /// When heat_pump_topic / cooker_topic are None (unconfigured), the
    /// corresponding sensors_meta entries must be absent.
    #[test]
    fn dashboard_snapshot_omits_unconfigured_z2m_sensors_meta() {
        let now = Instant::now();
        let world = World::fresh_boot(now);
        let meta = test_meta(None, None);
        let snap = world_to_snapshot(&world, &meta);
        assert!(
            !snap.sensors_meta.contains_key("heat_pump_power"),
            "heat_pump_power should be absent when topic is None"
        );
        assert!(
            !snap.sensors_meta.contains_key("cooker_power"),
            "cooker_power should be absent when topic is None"
        );
        // MPPT op-mode entries are always present (D-Bus, unconditional).
        assert!(snap.sensors_meta.contains_key("mppt_0_operation_mode"));
        assert!(snap.sensors_meta.contains_key("mppt_1_operation_mode"));
    }
}

#[cfg(test)]
mod zappi_drain_state_tests {
    use super::*;
    use std::time::{Duration, Instant};
    use victron_controller_core::topology::{ControllerParams, HardwareParams};
    use victron_controller_core::types::ZappiDrainBranch as CoreBranch;
    use victron_controller_core::world::{World, ZappiDrainSnapshot};

    fn test_meta() -> MetaContext {
        MetaContext {
            services: crate::config::DbusServices::default_venus_3_70(),
            open_meteo_cadence: Duration::from_secs(1800),
            controller_params: ControllerParams::defaults(),
            matter_outdoor_topic: None,
            ev_soc_discovery_topic: None,
            ev_charge_target_discovery_topic: None,
            heat_pump_topic: None,
            cooker_topic: None,
            soc_history: crate::dashboard::soc_history::SocHistoryStore::new(),
            hardware: HardwareParams::defaults(),
        }
    }

    fn make_snapshot(drain_w: f64, branch: CoreBranch, clamp: bool, excess: f64, ms: i64) -> ZappiDrainSnapshot {
        ZappiDrainSnapshot {
            compensated_drain_w: drain_w,
            branch,
            hard_clamp_engaged: clamp,
            hard_clamp_excess_w: excess,
            threshold_w: 500,
            hard_clamp_w: 200,
            captured_at_ms: ms,
        }
    }

    /// PR-ZDO-3.T1: world_to_snapshot surfaces `zappi_drain_state` with
    /// correct `latest` and `samples` when 5 snapshots have been pushed.
    #[test]
    fn dashboard_snapshot_surfaces_zappi_drain_state() {
        let now = Instant::now();
        let mut world = World::fresh_boot(now);

        // Push 5 snapshots, spacing them by SAMPLE_INTERVAL_MS to guarantee
        // all 5 land in the ring buffer.
        let interval = victron_controller_core::world::ZappiDrainState::SAMPLE_INTERVAL_MS;
        let base_ms = 1_700_000_000_000_i64;
        let snaps = [
            make_snapshot(100.0, CoreBranch::Tighten, false, 0.0, base_ms),
            make_snapshot(200.0, CoreBranch::Relax,   false, 0.0, base_ms + interval),
            make_snapshot(300.0, CoreBranch::Bypass,  false, 0.0, base_ms + interval * 2),
            make_snapshot(400.0, CoreBranch::Disabled,false, 0.0, base_ms + interval * 3),
            make_snapshot(500.0, CoreBranch::Tighten, true,  50.0, base_ms + interval * 4),
        ];
        for s in &snaps {
            world.zappi_drain_state.push(*s);
        }

        let meta = test_meta();
        let snap = world_to_snapshot(&world, &meta);
        let zds = &snap.zappi_drain_state;

        assert_eq!(zds.samples.len(), 5, "expected 5 samples in wire state");

        // latest must reflect the last push (500 W, Tighten, clamp engaged)
        let latest = zds.latest.as_ref().expect("latest must be Some after pushes");
        assert!(
            (latest.compensated_drain_w - 500.0).abs() < f64::EPSILON,
            "latest.compensated_drain_w mismatch: {:?}",
            latest.compensated_drain_w
        );
        assert!(latest.hard_clamp_engaged, "latest.hard_clamp_engaged must be true");

        // samples[0] = first push: 100 W, Tighten
        let s0 = &zds.samples[0];
        assert!(
            (s0.compensated_drain_w - 100.0).abs() < f64::EPSILON,
            "samples[0].compensated_drain_w mismatch"
        );
        assert_eq!(
            s0.branch,
            ModelZappiDrainBranch::Tighten,
            "samples[0].branch must be Tighten"
        );

        // samples[4] = last push: 500 W, Tighten, clamp engaged
        let s4 = &zds.samples[4];
        assert!(
            (s4.compensated_drain_w - 500.0).abs() < f64::EPSILON,
            "samples[4].compensated_drain_w mismatch"
        );
        assert!(s4.hard_clamp_engaged, "samples[4].hard_clamp_engaged must be true");

        // Enum round-trip check: Bypass sample is at index 2
        assert_eq!(
            zds.samples[2].branch,
            ModelZappiDrainBranch::Bypass,
            "samples[2].branch must be Bypass"
        );
    }

    /// PR-ZDO-3.T2: fresh World (no captures) → `latest` is None,
    /// `samples` is empty, no panic.
    #[test]
    fn dashboard_snapshot_handles_empty_zappi_drain_state() {
        let now = Instant::now();
        let world = World::fresh_boot(now);
        let meta = test_meta();
        let snap = world_to_snapshot(&world, &meta);
        let zds = &snap.zappi_drain_state;

        assert!(
            zds.latest.is_none(),
            "latest must be None for fresh World"
        );
        assert_eq!(
            zds.samples.len(),
            0,
            "samples must be empty for fresh World"
        );
    }
}

#[cfg(test)]
mod sunrise_sunset_freshness_tests {
    use super::*;
    use chrono::NaiveDate;
    use std::time::Duration;
    use std::time::Instant;
    use victron_controller_core::world::SUNRISE_SUNSET_FRESHNESS;

    fn dt(h: u32, m: u32) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 6, 21)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    #[test]
    fn fresh_within_window() {
        // `now` is anchored ahead of `updated_at` so the subtraction
        // we care about (`now - updated_at`) doesn't go through
        // Instant arithmetic that clippy flags as unchecked.
        let updated_at = Instant::now();
        let now = updated_at + Duration::from_secs(60);
        let (sr, ss) = fresh_sunrise_sunset_impl(
            Some(updated_at),
            Some(dt(4, 30)),
            Some(dt(22, 0)),
            now,
        );
        assert!(sr.is_some(), "sunrise should be fresh");
        assert!(ss.is_some());
    }

    #[test]
    fn stale_outside_window() {
        let updated_at = Instant::now();
        let now = updated_at + SUNRISE_SUNSET_FRESHNESS + Duration::from_secs(1);
        let (sr, ss) = fresh_sunrise_sunset_impl(
            Some(updated_at),
            Some(dt(4, 30)),
            Some(dt(22, 0)),
            now,
        );
        assert!(sr.is_none(), "sunrise should be stale");
        assert!(ss.is_none());
    }

    #[test]
    fn never_observed_yields_none() {
        let now = Instant::now();
        let (sr, ss) = fresh_sunrise_sunset_impl(None, Some(dt(4, 30)), Some(dt(22, 0)), now);
        assert!(sr.is_none());
        assert!(ss.is_none());
    }

    #[test]
    fn fresh_but_polar_day_yields_none_for_each_missing_value() {
        let updated_at = Instant::now();
        let now = updated_at + Duration::from_secs(10);
        let (sr, ss) = fresh_sunrise_sunset_impl(
            Some(updated_at),
            Some(dt(4, 30)),
            None,
            now,
        );
        assert!(sr.is_some());
        assert!(ss.is_none());
    }
}

