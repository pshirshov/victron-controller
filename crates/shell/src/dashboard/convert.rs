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

use std::time::{SystemTime, UNIX_EPOCH};

use victron_controller_core::controllers::schedules::ScheduleSpec;
use victron_controller_core::knobs::{
    ChargeBatteryExtendedMode, DebugFullCharge, DischargeTime, ForecastDisagreementStrategy, Knobs,
};
use std::collections::BTreeMap;
use std::time::Duration;

use victron_controller_core::myenergi::{EddiMode, ZappiMode};
use victron_controller_core::tass::{Actual, Actuated, Freshness, TargetPhase};
use victron_controller_core::topology::ControllerParams;
use victron_controller_core::types::{Command, Decision, Event, KnobId, KnobValue, SensorId, TimerId};
use victron_controller_core::world::World;
use victron_controller_core::Owner;

use crate::config::DbusServices;

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
use victron_controller_dashboard_model::victron_controller::dashboard::world_snapshot::WorldSnapshot;

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
        DebugFullCharge::None => ModelDebugFullCharge::None_,
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
        },
        knobs: knobs_to_model(k),
        bookkeeping: ModelBookkeeping {
            next_full_charge_iso: b.next_full_charge.map(|dt| dt.to_string()),
            above_soc_date_iso: b.above_soc_date.map(|d| d.to_string()),
            prev_ess_state: b.prev_ess_state,
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
        },
        forecasts: ModelForecasts {
            solcast: f.forecast_solcast.as_ref().map(forecast_snapshot),
            forecast_solar: f.forecast_forecast_solar.as_ref().map(forecast_snapshot),
            open_meteo: f.forecast_open_meteo.as_ref().map(forecast_snapshot),
        },
        decisions: decisions_to_model(&world.decisions),
        cores_state: cores_state_to_model(&world.cores_state),
        timers: timers_to_model(&world.timers),
    }
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
        charge_car_extended: k.charge_car_extended,
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
    }
}

struct WorldActuatedRefs<'a> {
    grid_setpoint: &'a Actuated<i32>,
    input_current_limit: &'a Actuated<f64>,
    zappi_mode: &'a Actuated<ZappiMode>,
    eddi_mode: &'a Actuated<EddiMode>,
    schedule_0: &'a Actuated<ScheduleSpec>,
    schedule_1: &'a Actuated<ScheduleSpec>,
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
                ModelDebugFullCharge::None_ => DebugFullCharge::None,
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
        C::SetMode(c) => {
            let id = knob_id_from_name(&c.knob_name)?;
            let core_mode = match c.value {
                ModelMode::Weather => victron_controller_core::knobs::Mode::Weather,
                ModelMode::Forced => victron_controller_core::knobs::Mode::Forced,
            };
            Command::Knob { id, value: KnobValue::Mode(core_mode) }
        }
        C::SetKillSwitch(c) => Command::KillSwitch(c.value),
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
        "charge_car_extended" => KnobId::ChargeCarExtended,
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
        _ => return None,
    })
}
