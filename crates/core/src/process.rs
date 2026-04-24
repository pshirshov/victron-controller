//! The pure top-level entry point. See SPEC §5.5.
//!
//! ```text
//! process(event, world, clock, topology) -> Vec<Effect>
//! ```
//!
//! No I/O, no async, no wall-clock reads. All non-determinism is
//! injected: `clock` for time, `event` for external inputs.
//!
//! Flow:
//!
//! 1. Apply the event to the world (update a sensor, apply a knob command,
//!    confirm a readback, decay freshness on `Tick`).
//! 2. Run the seven controllers in dependency order. Each either proposes
//!    a new target (→ phase machine, possibly `WriteDbus` / `CallMyenergi`
//!    effect) or returns "leave alone". Controllers also update
//!    bookkeeping; bookkeeping changes become `Publish` effects.
//! 3. Return the accumulated effects.
//!
//! The controllers are deliberately re-run on *every* event. Each is cheap
//! (pure arithmetic), and doing so sidesteps the "which event triggers
//! which controller" dispatch problem entirely.

use std::time::{Duration, Instant};

use chrono::Timelike;

use crate::Clock;
use crate::controllers::current_limit::{
    CurrentLimitInput, CurrentLimitInputGlobals, evaluate_current_limit,
};
use crate::controllers::zappi_active::classify_zappi_active;
use crate::controllers::eddi_mode::{
    EddiModeInput, EddiModeKnobs, evaluate_eddi_mode,
};
use crate::controllers::eddi_mode::EddiModeAction;
use crate::controllers::schedules::{
    SchedulesInput, SchedulesInputGlobals, evaluate_schedules,
};
use crate::controllers::setpoint::{
    SetpointInput, SetpointInputGlobals, evaluate_setpoint,
};
use crate::controllers::weather_soc::{
    WeatherSocInput, WeatherSocInputGlobals, evaluate_weather_soc,
};
use crate::controllers::zappi_mode::{
    ZappiModeInput, ZappiModeInputGlobals, evaluate_zappi_mode,
};
use crate::controllers::zappi_mode::ZappiModeAction;
use crate::myenergi::EddiMode;
use crate::owner::Owner;
use crate::topology::{ControllerParams, Topology};
use crate::types::{
    ActuatedId, ActuatedReadback, BookkeepingKey, BookkeepingValue, Command, DbusTarget,
    DbusValue, Decision, Effect, Event, ForecastProvider, KnobId, KnobValue, LogLevel,
    MyenergiAction, PublishPayload, ScheduleField, SensorId, SensorReading, TypedReading,
};
use crate::world::{ForecastSnapshot, World};

/// γ-rule window: dashboard write suppresses HA commands within this
/// duration. SPEC §5.4.
const DASHBOARD_HOLD_WINDOW: Duration = Duration::from_secs(1);

/// Evaluate one event against the world, returning effects for the shell
/// to execute.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn process(
    event: &Event,
    world: &mut World,
    clock: &dyn Clock,
    topology: &Topology,
) -> Vec<Effect> {
    let mut effects = Vec::new();
    apply_event(event, world, clock, topology, &mut effects);
    run_controllers(world, clock, topology, &mut effects);
    effects
}

// =============================================================================
// Event application
// =============================================================================

fn apply_event(
    event: &Event,
    world: &mut World,
    clock: &dyn Clock,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    match event {
        Event::Sensor(reading) => apply_sensor_reading(*reading, world),
        Event::TypedSensor(reading) => apply_typed_reading(*reading, world),
        Event::Readback(readback) => apply_readback(*readback, world, topology, effects),
        Event::Command {
            command,
            owner,
            at,
        } => apply_command(*command, *owner, *at, world, effects),
        Event::Tick { at } => apply_tick(*at, world, clock, topology),
    }
}

fn apply_sensor_reading(r: SensorReading, world: &mut World) {
    let v = r.value;
    let at = r.at;
    match r.id {
        SensorId::BatterySoc => world.sensors.battery_soc.on_reading(v, at),
        SensorId::BatterySoh => world.sensors.battery_soh.on_reading(v, at),
        SensorId::BatteryInstalledCapacity => {
            world.sensors.battery_installed_capacity.on_reading(v, at);
        }
        SensorId::BatteryDcPower => world.sensors.battery_dc_power.on_reading(v, at),
        SensorId::MpptPower0 => world.sensors.mppt_power_0.on_reading(v, at),
        SensorId::MpptPower1 => world.sensors.mppt_power_1.on_reading(v, at),
        SensorId::SoltaroPower => world.sensors.soltaro_power.on_reading(v, at),
        SensorId::PowerConsumption => world.sensors.power_consumption.on_reading(v, at),
        SensorId::GridPower => world.sensors.grid_power.on_reading(v, at),
        SensorId::GridVoltage => world.sensors.grid_voltage.on_reading(v, at),
        SensorId::GridCurrent => world.sensors.grid_current.on_reading(v, at),
        SensorId::ConsumptionCurrent => world.sensors.consumption_current.on_reading(v, at),
        SensorId::OffgridPower => world.sensors.offgrid_power.on_reading(v, at),
        SensorId::OffgridCurrent => world.sensors.offgrid_current.on_reading(v, at),
        SensorId::VebusInputCurrent => world.sensors.vebus_input_current.on_reading(v, at),
        SensorId::EvchargerAcPower => world.sensors.evcharger_ac_power.on_reading(v, at),
        SensorId::EvchargerAcCurrent => world.sensors.evcharger_ac_current.on_reading(v, at),
        SensorId::EssState => world.sensors.ess_state.on_reading(v, at),
        SensorId::OutdoorTemperature => world.sensors.outdoor_temperature.on_reading(v, at),
    }
}

fn apply_typed_reading(r: TypedReading, world: &mut World) {
    match r {
        TypedReading::Zappi { state, at } => world.typed_sensors.zappi_state.on_reading(state, at),
        TypedReading::Eddi { mode, at } => world.typed_sensors.eddi_mode.on_reading(mode, at),
        TypedReading::Forecast {
            provider,
            today_kwh,
            tomorrow_kwh,
            at,
        } => {
            let snap = ForecastSnapshot {
                today_kwh,
                tomorrow_kwh,
                fetched_at: at,
            };
            match provider {
                ForecastProvider::Solcast => world.typed_sensors.forecast_solcast = Some(snap),
                ForecastProvider::ForecastSolar => {
                    world.typed_sensors.forecast_forecast_solar = Some(snap);
                }
                ForecastProvider::OpenMeteo => {
                    world.typed_sensors.forecast_open_meteo = Some(snap);
                }
            }
        }
    }
}

fn apply_readback(
    r: ActuatedReadback,
    world: &mut World,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    let params = topology.controller_params;
    match r {
        ActuatedReadback::GridSetpoint { value, at } => {
            world.grid_setpoint.on_reading(value, at);
            let tol = params.setpoint_confirm_tolerance_w;
            if world
                .grid_setpoint
                .confirm_if(|t, a| (*t - *a).abs() <= tol, at)
            {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::GridSetpoint,
                    phase: world.grid_setpoint.target.phase,
                }));
            }
        }
        ActuatedReadback::InputCurrentLimit { value, at } => {
            world.input_current_limit.on_reading(value, at);
            let tol = params.current_limit_confirm_tolerance_a;
            if world
                .input_current_limit
                .confirm_if(|t, a| (*t - *a).abs() <= tol, at)
            {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::InputCurrentLimit,
                    phase: world.input_current_limit.target.phase,
                }));
            }
        }
        ActuatedReadback::ZappiMode { mode, at } => {
            world.zappi_mode.on_reading(mode, at);
            if world.zappi_mode.confirm_if(|t, a| t == a, at) {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::ZappiMode,
                    phase: world.zappi_mode.target.phase,
                }));
            }
        }
        ActuatedReadback::EddiMode { mode, at } => {
            world.eddi_mode.on_reading(mode, at);
            if world.eddi_mode.confirm_if(|t, a| t == a, at) {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::EddiMode,
                    phase: world.eddi_mode.target.phase,
                }));
            }
        }
        ActuatedReadback::Schedule0 { value, at } => {
            world.schedule_0.on_reading(value, at);
            if world.schedule_0.confirm_if(|t, a| t == a, at) {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::Schedule0,
                    phase: world.schedule_0.target.phase,
                }));
            }
        }
        ActuatedReadback::Schedule1 { value, at } => {
            world.schedule_1.on_reading(value, at);
            if world.schedule_1.confirm_if(|t, a| t == a, at) {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::Schedule1,
                    phase: world.schedule_1.target.phase,
                }));
            }
        }
    }
}

fn apply_command(
    command: Command,
    owner: Owner,
    at: Instant,
    world: &mut World,
    effects: &mut Vec<Effect>,
) {
    match command {
        Command::Knob { id, value } => {
            if !accept_knob_command(owner, at, world) {
                effects.push(Effect::Log {
                    level: LogLevel::Debug,
                    source: "process::command",
                    message: format!(
                        "suppressed knob command id={id:?} owner={owner:?} (dashboard γ-hold active)"
                    ),
                });
                return;
            }
            apply_knob(id, value, world, effects);
            if owner == Owner::Dashboard {
                world.knob_provenance.last_dashboard_write = Some(at);
            }
            effects.push(Effect::Publish(PublishPayload::Knob { id, value }));
        }
        Command::KillSwitch(enabled) => {
            let prev = world.knobs.writes_enabled;
            world.knobs.writes_enabled = enabled;
            // Edge-triggered reset (PR-05, A-06/A-07): transitioning from
            // observer (writes suppressed) to active (writes enabled)
            // invalidates every actuated target so the controllers are
            // forced to re-propose + emit a fresh WriteDbus/CallMyenergi
            // on the next tick. Without this, any target that was left
            // in a non-Unset phase (e.g. from an earlier live run, or —
            // once the observer-mode fix lands together with this —
            // from retained MQTT state) would make propose_target's
            // same-value short-circuit fire forever.
            if !prev && enabled {
                world.grid_setpoint.reset_to_unset(at);
                world.input_current_limit.reset_to_unset(at);
                world.zappi_mode.reset_to_unset(at);
                world.eddi_mode.reset_to_unset(at);
                world.schedule_0.reset_to_unset(at);
                world.schedule_1.reset_to_unset(at);
                for id in [
                    ActuatedId::GridSetpoint,
                    ActuatedId::InputCurrentLimit,
                    ActuatedId::ZappiMode,
                    ActuatedId::EddiMode,
                    ActuatedId::Schedule0,
                    ActuatedId::Schedule1,
                ] {
                    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                        id,
                        phase: crate::tass::TargetPhase::Unset,
                    }));
                }
            }
            effects.push(Effect::Publish(PublishPayload::KillSwitch(enabled)));
        }
        Command::Bookkeeping { key, value } => {
            apply_bookkeeping(key, value, world);
            effects.push(Effect::Publish(PublishPayload::Bookkeeping(key, value)));
        }
    }
}

fn apply_bookkeeping(
    key: BookkeepingKey,
    value: BookkeepingValue,
    world: &mut World,
) {
    let bk = &mut world.bookkeeping;
    match (key, value) {
        (BookkeepingKey::NextFullCharge, BookkeepingValue::NaiveDateTime(dt)) => {
            bk.next_full_charge = Some(dt);
        }
        (BookkeepingKey::NextFullCharge, BookkeepingValue::Cleared) => {
            bk.next_full_charge = None;
        }
        (BookkeepingKey::AboveSocDate, BookkeepingValue::NaiveDate(d)) => {
            bk.above_soc_date = Some(d);
        }
        (BookkeepingKey::AboveSocDate, BookkeepingValue::Cleared) => {
            bk.above_soc_date = None;
        }
        (BookkeepingKey::PrevEssState, BookkeepingValue::OptionalInt(v)) => {
            bk.prev_ess_state = v;
        }
        (BookkeepingKey::PrevEssState, BookkeepingValue::Cleared) => {
            bk.prev_ess_state = None;
        }
        _ => {
            // Type mismatch — retained payload shape doesn't match the
            // key's expected shape. Silently drop; the controllers will
            // rebuild state on first tick.
        }
    }
}

fn accept_knob_command(owner: Owner, at: Instant, world: &World) -> bool {
    // γ-rule: dashboard writes suppress HA commands within the hold window.
    if owner == Owner::HaMqtt {
        if let Some(last) = world.knob_provenance.last_dashboard_write {
            if at.saturating_duration_since(last) < DASHBOARD_HOLD_WINDOW {
                return false;
            }
        }
    }
    true
}

#[allow(clippy::too_many_lines)]
fn apply_knob(id: KnobId, value: KnobValue, world: &mut World, effects: &mut Vec<Effect>) {
    let k = &mut world.knobs;
    match (id, value) {
        (KnobId::ForceDisableExport, KnobValue::Bool(v)) => k.force_disable_export = v,
        (KnobId::ExportSocThreshold, KnobValue::Float(v)) => k.export_soc_threshold = v,
        (KnobId::DischargeSocTarget, KnobValue::Float(v)) => k.discharge_soc_target = v,
        (KnobId::BatterySocTarget, KnobValue::Float(v)) => k.battery_soc_target = v,
        (KnobId::FullChargeDischargeSocTarget, KnobValue::Float(v)) => {
            k.full_charge_discharge_soc_target = v;
        }
        (KnobId::FullChargeExportSocThreshold, KnobValue::Float(v)) => {
            k.full_charge_export_soc_threshold = v;
        }
        (KnobId::DischargeTime, KnobValue::DischargeTime(v)) => k.discharge_time = v,
        (KnobId::DebugFullCharge, KnobValue::DebugFullCharge(v)) => k.debug_full_charge = v,
        (KnobId::PessimismMultiplierModifier, KnobValue::Float(v)) => {
            k.pessimism_multiplier_modifier = v;
        }
        (KnobId::DisableNightGridDischarge, KnobValue::Bool(v)) => k.disable_night_grid_discharge = v,
        (KnobId::ChargeCarBoost, KnobValue::Bool(v)) => k.charge_car_boost = v,
        (KnobId::ChargeCarExtended, KnobValue::Bool(v)) => k.charge_car_extended = v,
        (KnobId::ZappiCurrentTarget, KnobValue::Float(v)) => k.zappi_current_target = v,
        (KnobId::ZappiLimit, KnobValue::Float(v)) => k.zappi_limit = v,
        (KnobId::ZappiEmergencyMargin, KnobValue::Float(v)) => k.zappi_emergency_margin = v,
        (KnobId::GridExportLimitW, KnobValue::Uint32(v)) => k.grid_export_limit_w = v,
        (KnobId::GridImportLimitW, KnobValue::Uint32(v)) => k.grid_import_limit_w = v,
        (KnobId::AllowBatteryToCar, KnobValue::Bool(v)) => k.allow_battery_to_car = v,
        (KnobId::EddiEnableSoc, KnobValue::Float(v)) => k.eddi_enable_soc = v,
        (KnobId::EddiDisableSoc, KnobValue::Float(v)) => k.eddi_disable_soc = v,
        (KnobId::EddiDwellS, KnobValue::Uint32(v)) => k.eddi_dwell_s = v,
        (KnobId::WeathersocWinterTemperatureThreshold, KnobValue::Float(v)) => {
            k.weathersoc_winter_temperature_threshold = v;
        }
        (KnobId::WeathersocLowEnergyThreshold, KnobValue::Float(v)) => {
            k.weathersoc_low_energy_threshold = v;
        }
        (KnobId::WeathersocOkEnergyThreshold, KnobValue::Float(v)) => {
            k.weathersoc_ok_energy_threshold = v;
        }
        (KnobId::WeathersocHighEnergyThreshold, KnobValue::Float(v)) => {
            k.weathersoc_high_energy_threshold = v;
        }
        (KnobId::WeathersocTooMuchEnergyThreshold, KnobValue::Float(v)) => {
            k.weathersoc_too_much_energy_threshold = v;
        }
        (KnobId::ForecastDisagreementStrategy, KnobValue::ForecastDisagreementStrategy(v)) => {
            k.forecast_disagreement_strategy = v;
        }
        (KnobId::ChargeBatteryExtendedMode, KnobValue::ChargeBatteryExtendedMode(v)) => {
            k.charge_battery_extended_mode = v;
        }
        _ => {
            effects.push(Effect::Log {
                level: LogLevel::Warn,
                source: "process::command",
                message: format!(
                    "apply_knob: KnobId/KnobValue type mismatch — silently dropped (schema drift?) id={id:?} value={value:?}"
                ),
            });
        }
    }
}

fn apply_tick(at: Instant, world: &mut World, clock: &dyn Clock, topology: &Topology) {
    let p = topology.controller_params;
    let local = p.freshness_local_dbus;
    let myenergi = p.freshness_myenergi;
    let weather = p.freshness_outdoor_temperature;

    let ss = &mut world.sensors;
    ss.battery_soc.tick(at, local);
    ss.battery_soh.tick(at, local);
    ss.battery_installed_capacity.tick(at, local);
    ss.battery_dc_power.tick(at, local);
    ss.mppt_power_0.tick(at, local);
    ss.mppt_power_1.tick(at, local);
    ss.soltaro_power.tick(at, local);
    ss.power_consumption.tick(at, local);
    ss.grid_power.tick(at, local);
    ss.grid_voltage.tick(at, local);
    ss.grid_current.tick(at, local);
    ss.consumption_current.tick(at, local);
    ss.offgrid_power.tick(at, local);
    ss.offgrid_current.tick(at, local);
    ss.vebus_input_current.tick(at, local);
    ss.evcharger_ac_power.tick(at, local);
    ss.evcharger_ac_current.tick(at, local);
    ss.ess_state.tick(at, local);
    ss.outdoor_temperature.tick(at, weather);

    world.typed_sensors.zappi_state.tick(at, myenergi);
    world.typed_sensors.eddi_mode.tick(at, myenergi);

    // A-15: midnight reset of the per-day weather_soc flag. If the date
    // the flag was stamped for isn't today, clear it. Intentionally
    // leave `charge_battery_extended_today_date` alone — it only advances
    // when `run_weather_soc` fires at 01:55, so before the first run on a
    // new day this field still points at yesterday (which is fine: the
    // date mismatch is exactly what drives the reset).
    let today = clock.naive().date();
    if world.bookkeeping.charge_battery_extended_today_date != Some(today) {
        world.bookkeeping.charge_battery_extended_today = false;
    }
}

// =============================================================================
// Controllers
// =============================================================================

/// Values derived from `world` once per tick, before controllers run.
/// Avoids stale-bookkeeping reads across controllers within one cycle
/// (A-05: `run_setpoint` previously consumed `bookkeeping.zappi_active`
/// which `run_current_limit` writes later in the same tick).
#[derive(Debug, Clone, Copy)]
struct DerivedView {
    zappi_active: bool,
}

/// Derive `zappi_active` via the canonical classifier so setpoint and
/// current-limit see exactly the same value within a tick. The
/// classifier itself handles freshness and the WaitingForEv timeout
/// (PR-04-D01/D02/D03).
fn compute_derived_view(world: &World, clock: &dyn Clock) -> DerivedView {
    DerivedView {
        zappi_active: classify_zappi_active(world, clock),
    }
}

fn run_controllers(
    world: &mut World,
    clock: &dyn Clock,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    let derived = compute_derived_view(world, clock);
    run_setpoint(world, derived, clock, topology, effects);
    run_current_limit(world, derived, clock, topology, effects);
    run_schedules(world, clock, effects);
    run_zappi_mode(world, clock, effects);
    run_eddi_mode(world, clock, effects);
    run_weather_soc(world, clock, effects);
}

// --- Setpoint -----------------------------------------------------------------

fn run_setpoint(
    world: &mut World,
    derived: DerivedView,
    clock: &dyn Clock,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    // Required Fresh sensors.
    if !world.sensors.battery_soc.is_usable()
        || !world.sensors.battery_soh.is_usable()
        || !world.sensors.battery_installed_capacity.is_usable()
        || !world.sensors.mppt_power_0.is_usable()
        || !world.sensors.mppt_power_1.is_usable()
        || !world.sensors.soltaro_power.is_usable()
        || !world.sensors.power_consumption.is_usable()
    {
        apply_setpoint_safety(world, clock, topology, effects);
        return;
    }

    let k = &world.knobs;
    let bk = &world.bookkeeping;
    let input = SetpointInput {
        globals: SetpointInputGlobals {
            force_disable_export: k.force_disable_export,
            export_soc_threshold: k.export_soc_threshold,
            discharge_soc_target: k.discharge_soc_target,
            full_charge_export_soc_threshold: k.full_charge_export_soc_threshold,
            full_charge_discharge_soc_target: k.full_charge_discharge_soc_target,
            // A-05: consume the derived-view `zappi_active` (computed
            // once per tick, before any controller) rather than
            // `bk.zappi_active` which is written later in the pipeline
            // by `run_current_limit`.
            zappi_active: derived.zappi_active,
            allow_battery_to_car: k.allow_battery_to_car,
            discharge_time: k.discharge_time,
            debug_full_charge: k.debug_full_charge,
            pessimism_multiplier_modifier: k.pessimism_multiplier_modifier,
            next_full_charge: bk.next_full_charge,
        },
        power_consumption: world.sensors.power_consumption.value.unwrap(),
        battery_soc: world.sensors.battery_soc.value.unwrap(),
        soh: world.sensors.battery_soh.value.unwrap(),
        mppt_power_0: world.sensors.mppt_power_0.value.unwrap(),
        mppt_power_1: world.sensors.mppt_power_1.value.unwrap(),
        soltaro_power: world.sensors.soltaro_power.value.unwrap(),
        capacity: world.sensors.battery_installed_capacity.value.unwrap(),
    };

    let out = evaluate_setpoint(&input, clock);

    // SPEC §5.11: grid-side hard cap — two-sided clamp.
    let export_cap = i32::try_from(k.grid_export_limit_w).unwrap_or(i32::MAX);
    let import_cap = i32::try_from(k.grid_import_limit_w).unwrap_or(i32::MAX);
    let pre_clamp = out.setpoint_target;
    let capped = pre_clamp.clamp(-export_cap, import_cap);

    let decision = out
        .decision
        .clone()
        .with_factor("pre_clamp_setpoint_W", format!("{pre_clamp}"))
        .with_factor("clamp_bounds_W", format!("[-{export_cap}, +{import_cap}]"))
        .with_factor("post_clamp_setpoint_W", format!("{capped}"));
    world.decisions.grid_setpoint = Some(decision);

    maybe_propose_setpoint(
        world,
        capped,
        Owner::SetpointController,
        clock.monotonic(),
        topology.controller_params,
        effects,
    );

    // Update bookkeeping from the setpoint's view.
    update_bookkeeping_from_setpoint(world, &out, effects);
}

fn apply_setpoint_safety(
    world: &mut World,
    clock: &dyn Clock,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    // Safety target: 10 W, owned by System.
    maybe_propose_setpoint(
        world,
        10,
        Owner::System,
        clock.monotonic(),
        topology.controller_params,
        effects,
    );
}

fn maybe_propose_setpoint(
    world: &mut World,
    value: i32,
    owner: Owner,
    now: Instant,
    params: ControllerParams,
    effects: &mut Vec<Effect>,
) {
    // Dead-band filter: don't restart the phase cycle if the current
    // target is within deadband and we're already confirmed.
    if let Some(current_target) = world.grid_setpoint.target.value {
        if (current_target - value).abs() < params.setpoint_retarget_deadband_w {
            return;
        }
    }

    // Observer mode (PR-05, A-06/A-07): emit the "would be set to X" log
    // but do NOT mutate target state. Leaving the target in its prior
    // phase means that when writes flip back on via `Command::KillSwitch`,
    // the edge-trigger there resets every target to `Unset` and the next
    // tick forces a clean propose + write.
    if !world.knobs.writes_enabled {
        effects.push(Effect::Log {
            level: LogLevel::Info,
            source: "observer",
            message: format!(
                "GridSetpoint would be set to {value} W (owner={owner:?}); suppressed by writes_enabled=false"
            ),
        });
        return;
    }

    let changed = world.grid_setpoint.propose_target(value, owner, now);
    if !changed {
        return;
    }

    effects.push(Effect::WriteDbus {
        target: DbusTarget::GridSetpoint,
        value: DbusValue::Int(value),
    });
    world.grid_setpoint.mark_commanded(now);
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::GridSetpoint,
        phase: world.grid_setpoint.target.phase,
    }));
}

fn update_bookkeeping_from_setpoint(
    world: &mut World,
    out: &crate::controllers::setpoint::SetpointOutput,
    effects: &mut Vec<Effect>,
) {
    let new_next = Some(out.bookkeeping.next_full_charge);
    if world.bookkeeping.next_full_charge != new_next {
        world.bookkeeping.next_full_charge = new_next;
        effects.push(Effect::Publish(PublishPayload::Bookkeeping(
            BookkeepingKey::NextFullCharge,
            BookkeepingValue::NaiveDateTime(out.bookkeeping.next_full_charge),
        )));
    }
    world.bookkeeping.charge_to_full_required = out.bookkeeping.charge_to_full_required;
    world.bookkeeping.soc_end_of_day_target = out.bookkeeping.soc_end_of_day_target;
    world.bookkeeping.effective_export_soc_threshold = out.bookkeeping.export_soc_threshold;
}

// --- Current limit ------------------------------------------------------------

fn run_current_limit(
    world: &mut World,
    derived: DerivedView,
    clock: &dyn Clock,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    // Required Fresh sensors.
    let s = &world.sensors;
    if !s.power_consumption.is_usable()
        || !s.offgrid_power.is_usable()
        || !s.offgrid_current.is_usable()
        || !s.grid_voltage.is_usable()
        || !s.grid_power.is_usable()
        || !s.mppt_power_0.is_usable()
        || !s.mppt_power_1.is_usable()
        || !s.soltaro_power.is_usable()
        || !s.evcharger_ac_current.is_usable()
        || !s.battery_dc_power.is_usable()
        || !s.battery_soc.is_usable()
        || !s.ess_state.is_usable()
    {
        return;
    }
    if !world.typed_sensors.zappi_state.is_usable() {
        return;
    }

    let k = &world.knobs;
    let bk = &world.bookkeeping;
    let input = CurrentLimitInput {
        globals: CurrentLimitInputGlobals {
            zappi_current_target: k.zappi_current_target,
            zappi_emergency_margin: k.zappi_emergency_margin,
            zappi_state: world.typed_sensors.zappi_state.value.unwrap(),
            // PR-04-D01/D02/D03: share the DerivedView's `zappi_active`
            // so setpoint and current-limit agree within a tick.
            zappi_active: derived.zappi_active,
            extended_charge_required: k.charge_car_extended
                || world.bookkeeping.charge_to_full_required,
            disable_night_grid_discharge: k.disable_night_grid_discharge,
            battery_soc_target: bk.battery_selected_soc_target,
            force_disable_export: k.force_disable_export,
            prev_ess_state: bk.prev_ess_state,
        },
        consumption_power: s.power_consumption.value.unwrap(),
        offgrid_power: s.offgrid_power.value.unwrap(),
        offgrid_current: s.offgrid_current.value.unwrap(),
        grid_voltage: s.grid_voltage.value.unwrap(),
        grid_power: s.grid_power.value.unwrap(),
        mppt_power_0: s.mppt_power_0.value.unwrap(),
        mppt_power_1: s.mppt_power_1.value.unwrap(),
        soltaro_power: s.soltaro_power.value.unwrap(),
        zappi_current: s.evcharger_ac_current.value.unwrap(),
        #[allow(clippy::cast_possible_truncation)]
        ess_state: s.ess_state.value.unwrap() as i32,
        battery_power: s.battery_dc_power.value.unwrap(),
        battery_soc: s.battery_soc.value.unwrap(),
    };

    let out = evaluate_current_limit(&input, clock);
    world.decisions.input_current_limit = Some(out.decision.clone());

    // Bookkeeping exports.
    if world.bookkeeping.prev_ess_state != out.bookkeeping.prev_ess_state {
        world.bookkeeping.prev_ess_state = out.bookkeeping.prev_ess_state;
        effects.push(Effect::Publish(PublishPayload::Bookkeeping(
            BookkeepingKey::PrevEssState,
            BookkeepingValue::OptionalInt(out.bookkeeping.prev_ess_state),
        )));
    }
    world.bookkeeping.zappi_active = out.bookkeeping.zappi_active;

    // Propose target.
    let value = out.input_current_limit;
    let now = clock.monotonic();
    let params = topology.controller_params;

    if let Some(current_target) = world.input_current_limit.target.value {
        if (current_target - value).abs() < params.current_limit_retarget_deadband_a {
            return;
        }
    }

    // Observer mode (PR-05): see `maybe_propose_setpoint` — target stays
    // untouched; the `Command::KillSwitch(true)` edge-trigger re-arms it.
    if !world.knobs.writes_enabled {
        effects.push(Effect::Log {
            level: LogLevel::Info,
            source: "observer",
            message: format!(
                "InputCurrentLimit would be set to {value:.2} A; suppressed by writes_enabled=false"
            ),
        });
        return;
    }

    let changed = world
        .input_current_limit
        .propose_target(value, Owner::CurrentLimitController, now);
    if !changed {
        return;
    }

    effects.push(Effect::WriteDbus {
        target: DbusTarget::InputCurrentLimit,
        value: DbusValue::Float(value),
    });
    world.input_current_limit.mark_commanded(now);
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::InputCurrentLimit,
        phase: world.input_current_limit.target.phase,
    }));
}

// --- Schedules ----------------------------------------------------------------

fn run_schedules(world: &mut World, clock: &dyn Clock, effects: &mut Vec<Effect>) {
    // Schedules always runs — battery_soc is the only required sensor.
    if !world.sensors.battery_soc.is_usable() {
        return;
    }

    let k = &world.knobs;
    let bk = &world.bookkeeping;
    // A-15: `charge_battery_extended` in Auto mode is true when EITHER:
    //   - the weekly Sunday-17:00 full-charge scheduler fired
    //     (`bk.charge_to_full_required`), or
    //   - today's weather_soc decision requires extended charging
    //     (`bk.charge_battery_extended_today`, set at 01:55 from the
    //     forecast + temperature ladder in `evaluate_weather_soc`;
    //     reset each midnight via `apply_tick`).
    // The legacy `!disable_night_grid_discharge` term was dropped — it
    // made cbe permanently true by default and was never the right
    // semantic.
    let cbe_from_full = bk.charge_to_full_required;
    let cbe_from_weather = bk.charge_battery_extended_today;
    let cbe_derived = cbe_from_full || cbe_from_weather;
    let charge_battery_extended = match k.charge_battery_extended_mode {
        crate::knobs::ChargeBatteryExtendedMode::Auto => cbe_derived,
        crate::knobs::ChargeBatteryExtendedMode::Forced => true,
        crate::knobs::ChargeBatteryExtendedMode::Disabled => false,
    };

    let input = SchedulesInput {
        globals: SchedulesInputGlobals {
            charge_battery_extended,
            charge_car_extended: k.charge_car_extended,
            charge_to_full_required: bk.charge_to_full_required,
            disable_night_grid_discharge: k.disable_night_grid_discharge,
            zappi_active: bk.zappi_active,
            above_soc_date: bk.above_soc_date,
            battery_soc_target: k.battery_soc_target,
        },
        battery_soc: world.sensors.battery_soc.value.unwrap(),
    };

    let out = evaluate_schedules(&input, clock);
    let decision = out
        .decision
        .clone()
        .with_factor(
            "cbe derivation",
            format!(
                "charge_to_full_required={cbe_from_full} || charge_battery_extended_today={cbe_from_weather} = {cbe_derived}"
            ),
        )
        .with_factor(
            "cbe mode override",
            format!("{:?} → {charge_battery_extended}", k.charge_battery_extended_mode),
        );
    world.decisions.schedule_0 = Some(decision.clone());
    world.decisions.schedule_1 = Some(decision);

    // Bookkeeping updates.
    world.bookkeeping.battery_selected_soc_target = out.bookkeeping.battery_selected_soc_target;
    if let Some(new_date) = out.bookkeeping.new_above_soc_date {
        if world.bookkeeping.above_soc_date != Some(new_date) {
            world.bookkeeping.above_soc_date = Some(new_date);
            effects.push(Effect::Publish(PublishPayload::Bookkeeping(
                BookkeepingKey::AboveSocDate,
                BookkeepingValue::NaiveDate(new_date),
            )));
        }
    }

    let now = clock.monotonic();

    maybe_propose_schedule(
        world,
        0,
        out.schedule_0,
        now,
        effects,
    );
    maybe_propose_schedule(
        world,
        1,
        out.schedule_1,
        now,
        effects,
    );
}

fn maybe_propose_schedule(
    world: &mut World,
    index: u8,
    spec: crate::controllers::schedules::ScheduleSpec,
    now: Instant,
    effects: &mut Vec<Effect>,
) {
    let actuated = if index == 0 {
        &mut world.schedule_0
    } else {
        &mut world.schedule_1
    };
    let id = if index == 0 {
        ActuatedId::Schedule0
    } else {
        ActuatedId::Schedule1
    };

    // Observer mode (PR-05): see `maybe_propose_setpoint`. Skip the
    // propose + 5 WriteDbus burst entirely. The `Command::KillSwitch(true)`
    // edge-trigger will reset the target on the way back to live.
    if !world.knobs.writes_enabled {
        effects.push(Effect::Log {
            level: LogLevel::Info,
            source: "observer",
            message: format!(
                "Schedule{index} would be set to {spec:?}; suppressed by writes_enabled=false"
            ),
        });
        return;
    }

    let changed = actuated.propose_target(spec, Owner::ScheduleController, now);
    if !changed {
        return;
    }

    // Emit 5 WriteDbus effects (one per field).
    effects.push(Effect::WriteDbus {
        target: DbusTarget::Schedule {
            index,
            field: ScheduleField::Start,
        },
        value: DbusValue::Int(spec.start_s),
    });
    effects.push(Effect::WriteDbus {
        target: DbusTarget::Schedule {
            index,
            field: ScheduleField::Duration,
        },
        value: DbusValue::Int(spec.duration_s),
    });
    effects.push(Effect::WriteDbus {
        target: DbusTarget::Schedule {
            index,
            field: ScheduleField::Soc,
        },
        value: DbusValue::Float(spec.soc),
    });
    effects.push(Effect::WriteDbus {
        target: DbusTarget::Schedule {
            index,
            field: ScheduleField::Days,
        },
        value: DbusValue::Int(spec.days),
    });
    effects.push(Effect::WriteDbus {
        target: DbusTarget::Schedule {
            index,
            field: ScheduleField::AllowDischarge,
        },
        value: DbusValue::Int(spec.discharge),
    });
    actuated.mark_commanded(now);
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id,
        phase: actuated.target.phase,
    }));
}

// --- Zappi mode ---------------------------------------------------------------

fn run_zappi_mode(world: &mut World, clock: &dyn Clock, effects: &mut Vec<Effect>) {
    if !world.typed_sensors.zappi_state.is_usable() {
        return;
    }
    let current_mode = world.typed_sensors.zappi_state.value.unwrap().zappi_mode;

    let k = &world.knobs;
    // Derive session_charged_pct: we don't have a real channel for this
    // yet (future: add to ZappiState). Assume 0 % for now — the shell will
    // populate once we add the field to the myenergi parser.
    let session_charged_pct = 0.0;

    let input = ZappiModeInput {
        globals: ZappiModeInputGlobals {
            charge_car_boost: k.charge_car_boost,
            charge_car_extended: k.charge_car_extended,
            zappi_limit_pct: k.zappi_limit,
        },
        current_mode,
        session_charged_pct,
    };

    let out = evaluate_zappi_mode(&input, clock);
    world.decisions.zappi_mode = Some(out.decision);
    let desired = match out.action {
        ZappiModeAction::Leave => return,
        ZappiModeAction::Set(m) => m,
    };

    let now = clock.monotonic();

    // Observer mode (PR-05): see `maybe_propose_setpoint`.
    if !world.knobs.writes_enabled {
        effects.push(Effect::Log {
            level: LogLevel::Info,
            source: "observer",
            message: format!(
                "ZappiMode would be set to {desired:?}; suppressed by writes_enabled=false"
            ),
        });
        return;
    }

    let changed = world
        .zappi_mode
        .propose_target(desired, Owner::ZappiController, now);
    if !changed {
        return;
    }

    effects.push(Effect::CallMyenergi(MyenergiAction::SetZappiMode(desired)));
    world.zappi_mode.mark_commanded(now);
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::ZappiMode,
        phase: world.zappi_mode.target.phase,
    }));
}

// --- Eddi mode ----------------------------------------------------------------

fn run_eddi_mode(world: &mut World, clock: &dyn Clock, effects: &mut Vec<Effect>) {
    let soc = &world.sensors.battery_soc;
    let current_mode = world
        .typed_sensors
        .eddi_mode
        .value
        .unwrap_or(EddiMode::Stopped);

    let k = &world.knobs;
    let input = EddiModeInput {
        soc_value: soc.value,
        soc_freshness: soc.freshness,
        current_mode,
        last_transition_at: world.bookkeeping.eddi_last_transition_at,
        knobs: EddiModeKnobs {
            enable_soc: k.eddi_enable_soc,
            disable_soc: k.eddi_disable_soc,
            dwell_s: k.eddi_dwell_s,
        },
    };

    let out = evaluate_eddi_mode(&input, clock);
    world.decisions.eddi_mode = Some(out.decision);
    let desired = match out.action {
        EddiModeAction::Leave => return,
        EddiModeAction::Set(m) => m,
    };

    let now = clock.monotonic();

    // Observer mode (PR-05): see `maybe_propose_setpoint`.
    if !world.knobs.writes_enabled {
        effects.push(Effect::Log {
            level: LogLevel::Info,
            source: "observer",
            message: format!(
                "EddiMode would be set to {desired:?}; suppressed by writes_enabled=false"
            ),
        });
        return;
    }

    let changed = world
        .eddi_mode
        .propose_target(desired, Owner::EddiController, now);
    if !changed {
        return;
    }

    effects.push(Effect::CallMyenergi(MyenergiAction::SetEddiMode(desired)));
    world.eddi_mode.mark_commanded(now);
    world.bookkeeping.eddi_last_transition_at = Some(now);
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::EddiMode,
        phase: world.eddi_mode.target.phase,
    }));
}

// --- Weather-SoC --------------------------------------------------------------

/// Weather-SoC runs only at the 01:55 cron moment. Because this pure core
/// sees no wall clock directly, we trigger when the naive time is in the
/// window 01:55:00–01:55:59. Outside that window (the common case) we
/// still publish a Decision explaining why it didn't evaluate — the
/// last real decision otherwise stays stuck at `None` all day and the
/// dashboard looks broken.
fn run_weather_soc(world: &mut World, clock: &dyn Clock, effects: &mut Vec<Effect>) {
    let now = clock.naive();
    if !(now.hour() == 1 && now.minute() == 55) {
        // Only overwrite with a "didn't run" decision if weather_soc
        // has never produced a real one; once it has, leave the last
        // real decision visible until tomorrow's 01:55.
        if world.decisions.weather_soc.is_none() {
            world.decisions.weather_soc = Some(
                Decision::new(format!(
                    "Not scheduled to run (fires only at 01:55 local; current {:02}:{:02})",
                    now.hour(),
                    now.minute()
                ))
                .with_factor("now_hhmm", format!("{:02}:{:02}", now.hour(), now.minute()))
                .with_factor("scheduled_at", "01:55".to_string()),
            );
        }
        return;
    }

    // Use today's temp if fresh; else skip (with explanation).
    if !world.sensors.outdoor_temperature.is_usable() {
        world.decisions.weather_soc = Some(
            Decision::new("Skipped: outdoor_temperature not usable".to_string())
                .with_factor(
                    "outdoor_temperature.freshness",
                    format!("{:?}", world.sensors.outdoor_temperature.freshness),
                )
                .with_factor(
                    "outdoor_temperature.value",
                    world
                        .sensors
                        .outdoor_temperature
                        .value
                        .map_or("None".to_string(), |v| format!("{v:.1}°C")),
                ),
        );
        return;
    }

    // Fuse forecasts across providers. We don't track provider-level
    // freshness in World yet (would need another Actual per provider);
    // treat all snapshots as fresh — the shell is responsible for only
    // ever publishing fresh data and stopping republishes when stale.
    let strategy = world.knobs.forecast_disagreement_strategy;
    let Some(today_kwh) = crate::controllers::forecast_fusion::fused_today_kwh(
        &world.typed_sensors,
        strategy,
        |_provider, _snap| true,
    ) else {
        world.decisions.weather_soc = Some(
            Decision::new("Skipped: no fused forecast available".to_string())
                .with_factor("strategy", format!("{strategy:?}")),
        );
        return;
    };

    let k = &world.knobs;
    let input = WeatherSocInput {
        globals: WeatherSocInputGlobals {
            charge_to_full_required: world.bookkeeping.charge_to_full_required,
            winter_temperature_threshold_c: k.weathersoc_winter_temperature_threshold,
            low_energy_threshold_kwh: k.weathersoc_low_energy_threshold,
            ok_energy_threshold_kwh: k.weathersoc_ok_energy_threshold,
            high_energy_threshold_kwh: k.weathersoc_high_energy_threshold,
            too_much_energy_threshold_kwh: k.weathersoc_too_much_energy_threshold,
        },
        today_temperature_c: world.sensors.outdoor_temperature.value.unwrap(),
        today_energy_kwh: today_kwh,
    };
    let d = evaluate_weather_soc(&input, clock);
    world.decisions.weather_soc = Some(d.decision.clone());

    // Translate decision into knob proposals (owner=WeatherSocPlanner).
    propose_knob(
        world,
        KnobId::ExportSocThreshold,
        KnobValue::Float(d.export_soc_threshold),
        effects,
    );
    propose_knob(
        world,
        KnobId::DischargeSocTarget,
        KnobValue::Float(d.discharge_soc_target),
        effects,
    );
    propose_knob(
        world,
        KnobId::BatterySocTarget,
        KnobValue::Float(d.battery_soc_target),
        effects,
    );
    propose_knob(
        world,
        KnobId::DisableNightGridDischarge,
        KnobValue::Bool(d.disable_night_grid_discharge),
        effects,
    );
    // A-15: record today's weather_soc decision on a dedicated per-day
    // field. `apply_tick` clears this on calendar-day rollover, so
    // schedules sees a fresh decision each day instead of a sticky OR
    // latch on `charge_to_full_required`.
    world.bookkeeping.charge_battery_extended_today = d.charge_battery_extended;
    world.bookkeeping.charge_battery_extended_today_date = Some(clock.naive().date());
}

fn propose_knob(
    world: &mut World,
    id: KnobId,
    value: KnobValue,
    effects: &mut Vec<Effect>,
) {
    apply_knob(id, value, world, effects);
    effects.push(Effect::Publish(PublishPayload::Knob { id, value }));
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use crate::myenergi::{ZappiMode, ZappiPlugState, ZappiState, ZappiStatus};
    use crate::tass::{Freshness, TargetPhase};
    use chrono::{NaiveDate, NaiveDateTime};
    use std::time::Duration as StdDuration;

    fn naive(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn clock_at(h: u32, m: u32) -> FixedClock {
        FixedClock::at(naive(h, m))
    }

    fn seed_required_sensors(world: &mut World, at: Instant) {
        // Tests that seed sensors want actuation effects; the cold-start
        // default is observer-mode (`writes_enabled=false`).
        world.knobs.writes_enabled = true;
        let ss = &mut world.sensors;
        ss.battery_soc.on_reading(75.0, at);
        ss.battery_soh.on_reading(95.0, at);
        ss.battery_installed_capacity.on_reading(100.0, at);
        ss.battery_dc_power.on_reading(0.0, at);
        ss.mppt_power_0.on_reading(1500.0, at);
        ss.mppt_power_1.on_reading(1000.0, at);
        ss.soltaro_power.on_reading(500.0, at);
        ss.power_consumption.on_reading(1200.0, at);
        ss.grid_power.on_reading(500.0, at);
        ss.grid_voltage.on_reading(230.0, at);
        ss.grid_current.on_reading(2.0, at);
        ss.consumption_current.on_reading(5.0, at);
        ss.offgrid_power.on_reading(500.0, at);
        ss.offgrid_current.on_reading(2.2, at);
        ss.vebus_input_current.on_reading(0.0, at);
        ss.evcharger_ac_power.on_reading(0.0, at);
        ss.evcharger_ac_current.on_reading(0.0, at);
        ss.ess_state.on_reading(10.0, at);
        ss.outdoor_temperature.on_reading(15.0, at);

        let nt = naive(12, 0);
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Off,
                zappi_plug_state: ZappiPlugState::EvDisconnected,
                zappi_status: ZappiStatus::Paused,
                zappi_last_change_signature: nt,
            },
            at,
        );
    }

    // ------------------------------------------------------------------
    // Setpoint flow: fresh-boot → tick → setpoint proposed → WriteDbus
    // ------------------------------------------------------------------

    #[test]
    fn setpoint_proposes_and_commands_when_all_sensors_fresh() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        let effects = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        // Phase moved Unset → Commanded.
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);
        // An Int WriteDbus to GridSetpoint was emitted.
        let wd = effects.iter().find_map(|e| match e {
            Effect::WriteDbus { target: DbusTarget::GridSetpoint, value: DbusValue::Int(v) } => Some(*v),
            _ => None,
        });
        assert!(wd.is_some());
    }

    #[test]
    fn setpoint_freezes_at_10w_when_battery_soc_stale() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // Age battery_soc past the 5 s freshness threshold.
        let later = FixedClock::new(c.monotonic + StdDuration::from_secs(30), naive(12, 0));
        let _ = process(&Event::Tick { at: later.monotonic }, &mut world, &later, &Topology::defaults());

        assert_eq!(world.sensors.battery_soc.freshness, Freshness::Stale);
        assert_eq!(world.grid_setpoint.target.value, Some(10));
        assert_eq!(world.grid_setpoint.target.owner, Owner::System);
    }

    #[test]
    fn setpoint_is_not_emitted_when_writes_enabled_false() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // seed helper enables writes; flip back off for this test.
        world.knobs.writes_enabled = false;

        let effects = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        // No WriteDbus / CallMyenergi should be emitted.
        for e in &effects {
            assert!(
                !matches!(e, Effect::WriteDbus { .. } | Effect::CallMyenergi(_)),
                "unexpected actuation effect: {e:?}"
            );
        }
    }

    #[test]
    fn observer_mode_logs_only_no_target_mutation() {
        // Observer mode = writes_enabled = false. PR-05 invariant:
        //   - at least one Info-level `observer` Log
        //   - NO WriteDbus / CallMyenergi
        //   - NO Publish(ActuatedPhase) for any controller (target state
        //     must stay untouched so the KillSwitch false→true edge can
        //     reset it cleanly)
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // seed helper enables writes; flip back off to get observer mode.
        world.knobs.writes_enabled = false;
        // Raise SoC above export threshold so setpoint isn't just 10.
        world.sensors.battery_soc.on_reading(90.0, c.monotonic);

        let effects = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        // At least one observer-source log line should fire.
        let observer_logs: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::Log { source: "observer", .. }))
            .collect();
        assert!(
            !observer_logs.is_empty(),
            "expected at least one observer-mode log, got {effects:#?}"
        );

        // All observer logs must be Info level.
        for e in &observer_logs {
            if let Effect::Log { level, .. } = e {
                assert_eq!(*level, LogLevel::Info, "observer log should be Info: {e:?}");
            }
        }

        // No actuation effects and no ActuatedPhase publishes — target
        // state must stay pristine so the kill-switch edge-trigger works.
        for e in &effects {
            assert!(
                !matches!(
                    e,
                    Effect::WriteDbus { .. }
                        | Effect::CallMyenergi(_)
                        | Effect::Publish(PublishPayload::ActuatedPhase { .. })
                ),
                "observer mode must not emit actuation or ActuatedPhase publish: {e:?}"
            );
        }
    }

    #[test]
    fn observer_mode_does_not_mutate_target_phase() {
        // PR-05, A-06: on a fresh boot (observer mode is the default),
        // every actuated target must remain `Unset` after a Tick. The
        // controllers must emit observer logs but skip propose_target
        // entirely.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // seed_required_sensors enables writes; don't call it directly —
        // inline the sensor seeds while leaving writes_enabled at its
        // fresh-boot default (false).
        let at = c.monotonic;
        let ss = &mut world.sensors;
        ss.battery_soc.on_reading(75.0, at);
        ss.battery_soh.on_reading(95.0, at);
        ss.battery_installed_capacity.on_reading(100.0, at);
        ss.battery_dc_power.on_reading(0.0, at);
        ss.mppt_power_0.on_reading(1500.0, at);
        ss.mppt_power_1.on_reading(1000.0, at);
        ss.soltaro_power.on_reading(500.0, at);
        ss.power_consumption.on_reading(1200.0, at);
        ss.grid_power.on_reading(500.0, at);
        ss.grid_voltage.on_reading(230.0, at);
        ss.grid_current.on_reading(2.0, at);
        ss.consumption_current.on_reading(5.0, at);
        ss.offgrid_power.on_reading(500.0, at);
        ss.offgrid_current.on_reading(2.2, at);
        ss.vebus_input_current.on_reading(0.0, at);
        ss.evcharger_ac_power.on_reading(0.0, at);
        ss.evcharger_ac_current.on_reading(0.0, at);
        ss.ess_state.on_reading(10.0, at);
        ss.outdoor_temperature.on_reading(15.0, at);
        let nt = naive(12, 0);
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Off,
                zappi_plug_state: ZappiPlugState::EvDisconnected,
                zappi_status: ZappiStatus::Paused,
                zappi_last_change_signature: nt,
            },
            at,
        );

        assert!(!world.knobs.writes_enabled, "fresh boot must be observer mode");

        let effects = process(&Event::Tick { at }, &mut world, &c, &Topology::defaults());

        // Every actuated target stays at Unset.
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Unset);
        assert_eq!(world.input_current_limit.target.phase, TargetPhase::Unset);
        assert_eq!(world.zappi_mode.target.phase, TargetPhase::Unset);
        assert_eq!(world.eddi_mode.target.phase, TargetPhase::Unset);
        assert_eq!(world.schedule_0.target.phase, TargetPhase::Unset);
        assert_eq!(world.schedule_1.target.phase, TargetPhase::Unset);

        // At least one observer-source Log emitted.
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Log { source: "observer", .. })),
            "expected at least one observer log, got {effects:#?}"
        );

        // Zero WriteDbus / CallMyenergi / ActuatedPhase publishes.
        for e in &effects {
            assert!(
                !matches!(
                    e,
                    Effect::WriteDbus { .. }
                        | Effect::CallMyenergi(_)
                        | Effect::Publish(PublishPayload::ActuatedPhase { .. })
                ),
                "observer mode leaked actuation or phase publish: {e:?}"
            );
        }
    }

    #[test]
    fn kill_switch_false_to_true_resets_pending_targets_and_forces_rewrite_next_tick() {
        // PR-05, A-06/A-07: observer→live→observer→live cycle. The key
        // invariant is that the false→true edge RESETS every target
        // before the controllers re-run, so even if propose_target's
        // same-value short-circuit would otherwise keep an in-flight
        // Pending target stuck forever, the reset-then-re-propose
        // pattern produces a fresh WriteDbus on the tick that follows
        // the edge (in practice, the controllers already re-run inside
        // the same `process()` call that handled KillSwitch(true)).
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // Step 1: live tick settles setpoint Commanded.
        let _ = process(
            &Event::Tick { at: c.monotonic },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);
        let v1 = world.grid_setpoint.target.value.expect("setpoint proposed");

        // Step 2: kill switch off — writes stop, but existing targets
        // stay Commanded (we don't reset on the way INTO observer mode).
        let _ = process(
            &Event::Command {
                command: Command::KillSwitch(false),
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert!(!world.knobs.writes_enabled);
        assert_eq!(
            world.grid_setpoint.target.phase,
            TargetPhase::Commanded,
            "entering observer mode must not wipe targets"
        );

        // Step 3: simulate the stuck-Pending hazard A-07 describes by
        // hand — a target left in Pending with the same value the
        // controllers want. Without the edge-trigger reset,
        // propose_target would short-circuit and no WriteDbus would
        // ever fire again.
        world.grid_setpoint.target.phase = TargetPhase::Pending;

        // Step 4: kill switch back on — edge-trigger resets every
        // target, then controllers run and immediately re-propose with
        // a fresh WriteDbus. We verify both the mid-call reset
        // (observable via Publish(ActuatedPhase=Unset) in the effect
        // stream) AND the follow-up Commanded rewrite (Publish with
        // phase=Commanded + WriteDbus).
        let eff_on = process(
            &Event::Command {
                command: Command::KillSwitch(true),
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert!(world.knobs.writes_enabled);

        // Each of the six actuators published Unset during the reset.
        for id in [
            ActuatedId::GridSetpoint,
            ActuatedId::InputCurrentLimit,
            ActuatedId::ZappiMode,
            ActuatedId::EddiMode,
            ActuatedId::Schedule0,
            ActuatedId::Schedule1,
        ] {
            assert!(
                eff_on.iter().any(|e| matches!(
                    e,
                    Effect::Publish(PublishPayload::ActuatedPhase { id: pub_id, phase: TargetPhase::Unset })
                        if *pub_id == id
                )),
                "expected Publish(ActuatedPhase {{ id: {id:?}, phase: Unset }}), got {eff_on:#?}"
            );
        }

        // Post-reset controller run re-proposed setpoint with the same
        // value as before + emitted a fresh WriteDbus.
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);
        assert_eq!(world.grid_setpoint.target.value, Some(v1));
        assert!(
            eff_on.iter().any(|e| matches!(
                e,
                Effect::WriteDbus {
                    target: DbusTarget::GridSetpoint,
                    value: DbusValue::Int(_)
                }
            )),
            "post-reset tick must emit a fresh GridSetpoint WriteDbus (got {eff_on:#?})"
        );
    }

    #[test]
    fn kill_switch_true_to_true_is_noop() {
        // PR-05: the reset edge-trigger is strictly false→true. A
        // redundant `KillSwitch(true)` while already enabled must NOT
        // wipe targets, and must NOT emit six ActuatedPhase=Unset
        // publishes.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        assert!(world.knobs.writes_enabled, "seed helper enables writes");

        // Settle Commanded first so we can tell if a reset fires.
        let _ = process(
            &Event::Tick { at: c.monotonic },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);

        // Redundant KillSwitch(true) — should not reset anything.
        let eff = process(
            &Event::Command {
                command: Command::KillSwitch(true),
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );

        assert_eq!(
            world.grid_setpoint.target.phase,
            TargetPhase::Commanded,
            "no-op KillSwitch(true) must not reset targets"
        );

        // No ActuatedPhase=Unset spam in the published effects.
        let unset_publishes = eff
            .iter()
            .filter(|e| matches!(
                e,
                Effect::Publish(PublishPayload::ActuatedPhase { phase: TargetPhase::Unset, .. })
            ))
            .count();
        assert_eq!(
            unset_publishes, 0,
            "redundant KillSwitch(true) must not publish Unset phases: {eff:#?}"
        );
    }

    #[test]
    fn setpoint_deadband_suppresses_minor_changes() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // First run — setpoint settles.
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let first_target = world.grid_setpoint.target.value.unwrap();

        // Tiny perturbation to consumption — should fall inside dead-band.
        let nudge = SensorReading {
            id: SensorId::PowerConsumption,
            value: 1200.1, // from 1200
            at: c.monotonic,
        };
        let effects = process(&Event::Sensor(nudge), &mut world, &c, &Topology::defaults());

        // Target didn't flip — we expect no new WriteDbus for GridSetpoint.
        let had_write = effects.iter().any(|e| matches!(
            e,
            Effect::WriteDbus { target: DbusTarget::GridSetpoint, .. }
        ));
        assert!(!had_write, "deadband should have suppressed the re-emit");
        assert_eq!(world.grid_setpoint.target.value.unwrap(), first_target);
    }

    // ------------------------------------------------------------------
    // Readback-driven confirmation
    // ------------------------------------------------------------------

    #[test]
    fn setpoint_phase_advances_to_confirmed_on_matching_readback() {
        // Note: confirmation is controller-specific (it uses the tolerance).
        // This core test exercises the primitive only.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let target = world.grid_setpoint.target.value.unwrap();
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);

        // Simulate readback equal to target: TASS primitive `on_reading`
        // updates actual but phase confirmation happens via the explicit
        // tolerance predicate — for test simplicity we call it by hand.
        world.grid_setpoint.on_reading(target, c.monotonic);
        let confirmed = world.grid_setpoint.confirm_if(
            |t, a| (*t - *a).abs() <= 50,
            c.monotonic,
        );
        assert!(confirmed);
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Confirmed);
    }

    #[test]
    fn setpoint_readback_event_drives_confirmation_automatically() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // Tick to propose + command.
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let target = world.grid_setpoint.target.value.unwrap();
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);

        // Readback within tolerance → Confirmed with a PublishPhase effect.
        let effects = process(
            &Event::Readback(ActuatedReadback::GridSetpoint {
                value: target + 12, // within ±50 tolerance
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Confirmed);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::Publish(PublishPayload::ActuatedPhase {
                id: ActuatedId::GridSetpoint,
                phase: TargetPhase::Confirmed
            })
        )));
    }

    #[test]
    fn setpoint_readback_out_of_tolerance_stays_commanded() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let target = world.grid_setpoint.target.value.unwrap();

        let _ = process(
            &Event::Readback(ActuatedReadback::GridSetpoint {
                value: target + 200, // outside ±50
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);
    }

    #[test]
    fn zappi_mode_readback_drives_confirmation_on_exact_match() {
        // Dial up conditions that cause the Zappi controller to propose a
        // target: Boost window with charge_car_boost enabled → Fast.
        let c = FixedClock::at(naive(3, 0));
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        world.knobs.charge_car_boost = true;

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        assert_eq!(world.zappi_mode.target.value, Some(ZappiMode::Fast));
        assert_eq!(world.zappi_mode.target.phase, TargetPhase::Commanded);

        let _ = process(
            &Event::Readback(ActuatedReadback::ZappiMode {
                mode: ZappiMode::Fast,
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.zappi_mode.target.phase, TargetPhase::Confirmed);
    }

    // ------------------------------------------------------------------
    // Knob command (γ hold)
    // ------------------------------------------------------------------

    #[test]
    fn ha_knob_command_suppressed_within_dashboard_hold() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);

        // Dashboard writes first.
        let e1 = process(
            &Event::Command {
                command: Command::Knob {
                    id: KnobId::ExportSocThreshold,
                    value: KnobValue::Float(50.0),
                },
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.knobs.export_soc_threshold, 50.0);
        assert!(e1
            .iter()
            .any(|e| matches!(e, Effect::Publish(PublishPayload::Knob { .. }))));

        // HA writes immediately after — should be suppressed.
        let e2 = process(
            &Event::Command {
                command: Command::Knob {
                    id: KnobId::ExportSocThreshold,
                    value: KnobValue::Float(67.0),
                },
                owner: Owner::HaMqtt,
                at: c.monotonic + StdDuration::from_millis(100),
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.knobs.export_soc_threshold, 50.0, "HA write suppressed");
        // There should be a Log effect noting suppression.
        assert!(
            e2.iter()
                .any(|e| matches!(e, Effect::Log { source: "process::command", .. })),
            "expected a suppression log"
        );
    }

    #[test]
    fn ha_knob_command_accepted_after_hold_expires() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let _ = process(
            &Event::Command {
                command: Command::Knob {
                    id: KnobId::ExportSocThreshold,
                    value: KnobValue::Float(50.0),
                },
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );

        let later = c.monotonic + StdDuration::from_secs(2); // > 1s hold
        let _ = process(
            &Event::Command {
                command: Command::Knob {
                    id: KnobId::ExportSocThreshold,
                    value: KnobValue::Float(67.0),
                },
                owner: Owner::HaMqtt,
                at: later,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.knobs.export_soc_threshold, 67.0);
    }

    // ------------------------------------------------------------------
    // Kill switch
    // ------------------------------------------------------------------

    #[test]
    fn bookkeeping_command_seeds_world_bookkeeping() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let restored_dt = NaiveDate::from_ymd_opt(2026, 4, 26)
            .unwrap()
            .and_hms_opt(17, 0, 0)
            .unwrap();
        let restored_date = NaiveDate::from_ymd_opt(2026, 4, 21).unwrap();

        let _ = process(
            &Event::Command {
                command: Command::Bookkeeping {
                    key: BookkeepingKey::NextFullCharge,
                    value: BookkeepingValue::NaiveDateTime(restored_dt),
                },
                owner: Owner::System,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.bookkeeping.next_full_charge, Some(restored_dt));

        let _ = process(
            &Event::Command {
                command: Command::Bookkeeping {
                    key: BookkeepingKey::AboveSocDate,
                    value: BookkeepingValue::NaiveDate(restored_date),
                },
                owner: Owner::System,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.bookkeeping.above_soc_date, Some(restored_date));

        let _ = process(
            &Event::Command {
                command: Command::Bookkeeping {
                    key: BookkeepingKey::PrevEssState,
                    value: BookkeepingValue::OptionalInt(Some(9)),
                },
                owner: Owner::System,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.bookkeeping.prev_ess_state, Some(9));

        // Cleared variant resets to None.
        let _ = process(
            &Event::Command {
                command: Command::Bookkeeping {
                    key: BookkeepingKey::AboveSocDate,
                    value: BookkeepingValue::Cleared,
                },
                owner: Owner::System,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.bookkeeping.above_soc_date, None);
    }

    #[test]
    fn kill_switch_toggles_writes_enabled_and_publishes() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // Fresh boot is observer-mode by default (§7 safety).
        assert!(!world.knobs.writes_enabled);

        // Flip it on via the kill switch.
        let eff = process(
            &Event::Command {
                command: Command::KillSwitch(true),
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert!(world.knobs.writes_enabled);
        assert!(eff.iter().any(|e| matches!(
            e,
            Effect::Publish(PublishPayload::KillSwitch(true))
        )));

        // And back off.
        let eff = process(
            &Event::Command {
                command: Command::KillSwitch(false),
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert!(!world.knobs.writes_enabled);
        assert!(eff.iter().any(|e| matches!(
            e,
            Effect::Publish(PublishPayload::KillSwitch(false))
        )));
    }

    // ------------------------------------------------------------------
    // Eddi controller wiring
    // ------------------------------------------------------------------

    #[test]
    fn eddi_requires_fresh_soc() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // No battery_soc reading — freshness Unknown.
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        // Desired mode would be Stopped (safety), which equals current
        // (world.eddi_mode.target is Unset, current_mode defaults to
        // Stopped) so no transition.
        assert_eq!(world.eddi_mode.target.phase, TargetPhase::Unset);
    }

    #[test]
    fn eddi_sets_normal_when_soc_above_threshold_and_current_known_stopped() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // Raise SoC above enable threshold.
        world.sensors.battery_soc.on_reading(99.0, c.monotonic);
        // Tell the world what the current Eddi mode is.
        world
            .typed_sensors
            .eddi_mode
            .on_reading(EddiMode::Stopped, c.monotonic);

        let eff = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        assert_eq!(world.eddi_mode.target.value, Some(EddiMode::Normal));
        assert!(eff
            .iter()
            .any(|e| matches!(e, Effect::CallMyenergi(MyenergiAction::SetEddiMode(EddiMode::Normal)))));
    }

    #[test]
    fn eddi_safety_stops_when_soc_becomes_stale() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        world.sensors.battery_soc.on_reading(99.0, c.monotonic);
        world
            .typed_sensors
            .eddi_mode
            .on_reading(EddiMode::Normal, c.monotonic);

        // Age SoC past freshness threshold.
        let later = FixedClock::new(c.monotonic + StdDuration::from_secs(30), naive(12, 0));
        let _ = process(&Event::Tick { at: later.monotonic }, &mut world, &later, &Topology::defaults());

        assert_eq!(world.sensors.battery_soc.freshness, Freshness::Stale);
        assert_eq!(world.eddi_mode.target.value, Some(EddiMode::Stopped));
    }

    // ------------------------------------------------------------------
    // Schedules controller wiring — 5 WriteDbus per changed schedule
    // ------------------------------------------------------------------

    #[test]
    fn schedules_emit_five_writes_when_target_changes() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        let eff = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let s0_writes = eff
            .iter()
            .filter(|e| matches!(
                e,
                Effect::WriteDbus { target: DbusTarget::Schedule { index: 0, .. }, .. }
            ))
            .count();
        assert_eq!(s0_writes, 5, "Schedule 0 should emit all five fields on change");
        let s1_writes = eff
            .iter()
            .filter(|e| matches!(
                e,
                Effect::WriteDbus { target: DbusTarget::Schedule { index: 1, .. }, .. }
            ))
            .count();
        assert_eq!(s1_writes, 5, "Schedule 1 should emit all five fields on change");
    }

    #[test]
    fn second_tick_with_no_change_does_not_re_emit_schedule_writes() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // First tick — schedules emitted.
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        // Second tick at same inputs — no change expected.
        let eff2 = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let sch_writes = eff2
            .iter()
            .filter(|e| matches!(
                e,
                Effect::WriteDbus { target: DbusTarget::Schedule { .. }, .. }
            ))
            .count();
        assert_eq!(sch_writes, 0);
    }

    // ------------------------------------------------------------------
    // Freshness decay via Tick
    // ------------------------------------------------------------------

    #[test]
    fn tick_decays_sensor_freshness_fresh_to_stale() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        world.sensors.battery_soc.on_reading(80.0, c.monotonic);
        assert_eq!(world.sensors.battery_soc.freshness, Freshness::Fresh);

        let later = FixedClock::new(c.monotonic + StdDuration::from_secs(10), naive(12, 0));
        let _ = process(&Event::Tick { at: later.monotonic }, &mut world, &later, &Topology::defaults());
        assert_eq!(world.sensors.battery_soc.freshness, Freshness::Stale);
    }

    // ------------------------------------------------------------------
    // Sensor event plumbing
    // ------------------------------------------------------------------

    #[test]
    fn sensor_event_updates_world_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let _ = process(
            &Event::Sensor(SensorReading {
                id: SensorId::BatterySoc,
                value: 77.5,
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.sensors.battery_soc.value, Some(77.5));
        assert_eq!(world.sensors.battery_soc.freshness, Freshness::Fresh);
    }

    #[test]
    fn typed_sensor_event_updates_zappi_state() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let state = ZappiState {
            zappi_mode: ZappiMode::Eco,
            zappi_plug_state: ZappiPlugState::Charging,
            zappi_status: ZappiStatus::DivertingOrCharging,
            zappi_last_change_signature: naive(12, 0),
        };
        let _ = process(
            &Event::TypedSensor(TypedReading::Zappi {
                state,
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.typed_sensors.zappi_state.value, Some(state));
    }

    // ------------------------------------------------------------------
    // Supersession: a new target during Commanded drops back to Pending
    // (verified here via the primitive, integrated via process)
    // ------------------------------------------------------------------

    #[test]
    fn setpoint_retargets_on_consumption_change_beyond_deadband() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // Daytime PV-multiplier scenario that's neither floor-clamped nor
        // grid-cap-clamped: modest PV above the bad-weather threshold,
        // SoC just above the export threshold.
        world.sensors.battery_soc.on_reading(90.0, c.monotonic);
        world.sensors.mppt_power_0.on_reading(800.0, c.monotonic);
        world.sensors.mppt_power_1.on_reading(600.0, c.monotonic);
        world.sensors.soltaro_power.on_reading(100.0, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let first = world.grid_setpoint.target.value.unwrap();
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);

        // Large change in consumption → target should shift.
        let _ = process(
            &Event::Sensor(SensorReading {
                id: SensorId::PowerConsumption,
                value: 500.0,
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        let second = world.grid_setpoint.target.value.unwrap();
        // We don't assert a specific value, just that the target moved
        // through Pending (now Commanded again after emit).
        assert_ne!(first, second, "large consumption change should move target");
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);
    }

    // ------------------------------------------------------------------
    // PR-09a: two-sided grid-setpoint clamp
    // ------------------------------------------------------------------

    #[test]
    fn setpoint_clamps_to_import_cap() {
        // force_disable_export → evaluate_setpoint produces IDLE_SETPOINT_W
        // (10 W). With grid_import_limit_w below that, the post-clamp value
        // must equal the import cap exactly.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        world.knobs.force_disable_export = true;
        world.knobs.grid_import_limit_w = 5;

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        assert_eq!(world.grid_setpoint.target.value, Some(5));
    }

    #[test]
    fn setpoint_clamps_to_export_cap() {
        // Regression for the existing export clamp — a deeply negative
        // pre-clamp (SoC=99 %, huge solar) must be capped at
        // -grid_export_limit_w after the refactor.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        world.sensors.battery_soc.on_reading(99.0, c.monotonic);
        world.sensors.mppt_power_0.on_reading(5000.0, c.monotonic);
        world.sensors.mppt_power_1.on_reading(5000.0, c.monotonic);
        world.knobs.grid_export_limit_w = 3000;

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let v = world.grid_setpoint.target.value.expect("setpoint proposed");
        assert!(v >= -3000, "setpoint {v} violates export cap -3000");
        assert_eq!(v, -3000, "setpoint should be pinned at the export cap");
    }

    #[test]
    fn setpoint_decision_has_pre_and_post_clamp_factors() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let decision = world.decisions.grid_setpoint.as_ref().expect("decision set");
        let names: Vec<&str> = decision.factors.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"pre_clamp_setpoint_W"),
            "missing pre_clamp_setpoint_W factor in {names:?}"
        );
        assert!(
            names.contains(&"post_clamp_setpoint_W"),
            "missing post_clamp_setpoint_W factor in {names:?}"
        );
        assert!(
            names.contains(&"clamp_bounds_W"),
            "missing clamp_bounds_W factor in {names:?}"
        );
    }

    // ------------------------------------------------------------------
    // PR-04: DerivedView + A-15 cbe derivation
    // ------------------------------------------------------------------

    #[test]
    fn setpoint_first_tick_sees_derived_zappi_active() {
        // A-05 regression: on the very first tick the controllers run,
        // `bookkeeping.zappi_active` is still its default false. Setpoint
        // must nevertheless see zappi_active=true because the per-tick
        // DerivedView derives it from typed_sensors. Without the fix,
        // setpoint would pick a branch that doesn't set the
        // `zappi_active="true"` factor.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // Force bookkeeping to the stale value the bug exhibited.
        world.bookkeeping.zappi_active = false;
        // Stamp the typed-sensor ZappiState as actively charging.
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Eco,
                zappi_plug_state: ZappiPlugState::Charging,
                zappi_status: ZappiStatus::DivertingOrCharging,
                zappi_last_change_signature: naive(12, 0),
            },
            c.monotonic,
        );
        // Raise SoC above export threshold so the zappi-active branch
        // actually fires.
        world.sensors.battery_soc.on_reading(90.0, c.monotonic);

        let _ = process(
            &Event::Tick { at: c.monotonic },
            &mut world,
            &c,
            &Topology::defaults(),
        );

        let decision = world
            .decisions
            .grid_setpoint
            .as_ref()
            .expect("grid_setpoint decision recorded");
        let has_zappi_factor = decision
            .factors
            .iter()
            .any(|f| f.name == "zappi_active" && f.value == "true");
        assert!(
            has_zappi_factor,
            "setpoint did not see derived zappi_active=true on the first tick \
             (factors: {:?})",
            decision.factors
        );
    }

    #[test]
    fn setpoint_follows_live_state_over_stale_bookkeeping_zappi_active() {
        // PR-04-D04: the residual hazard is the opposite direction —
        // bookkeeping is LATCHED true (from a prior tick when the car
        // was charging) but the LIVE typed sensor says the car is now
        // disconnected. Setpoint must follow the live state (via the
        // canonical classifier), not the stale bookkeeping.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // Stale latch: pretend current_limit classified the car as
        // active on a previous tick.
        world.bookkeeping.zappi_active = true;

        // Live state NOW: plug disconnected (definitively inactive).
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Off,
                zappi_plug_state: ZappiPlugState::EvDisconnected,
                zappi_status: ZappiStatus::Paused,
                zappi_last_change_signature: naive(12, 0),
            },
            c.monotonic,
        );
        // Live power reading: comfortably below the 500 W fallback.
        world.sensors.evcharger_ac_power.on_reading(0.0, c.monotonic);
        // Raise SoC above export threshold so the branch choice actually
        // moves between zappi-active vs the daytime default.
        world.sensors.battery_soc.on_reading(90.0, c.monotonic);

        let _ = process(
            &Event::Tick { at: c.monotonic },
            &mut world,
            &c,
            &Topology::defaults(),
        );

        let decision = world
            .decisions
            .grid_setpoint
            .as_ref()
            .expect("grid_setpoint decision recorded");
        let has_zappi_factor = decision
            .factors
            .iter()
            .any(|f| f.name == "zappi_active" && f.value == "true");
        assert!(
            !has_zappi_factor,
            "setpoint followed stale bookkeeping.zappi_active=true when \
             live typed state said EvDisconnected (factors: {:?})",
            decision.factors
        );
    }

    #[test]
    fn charge_to_full_required_resets_after_midnight_if_weekly_not_active() {
        // A-15 regression: weather_soc sets `charge_battery_extended_today`
        // on day N; after the calendar-day rollover, `apply_tick` must
        // clear it so `run_schedules` no longer derives cbe from
        // yesterday's weather decision.
        let day1 = FixedClock::at(naive(2, 0));
        let mut world = World::fresh_boot(day1.monotonic);
        seed_required_sensors(&mut world, day1.monotonic);
        // Seed the bookkeeping as if weather_soc fired yesterday (day1).
        world.bookkeeping.charge_battery_extended_today = true;
        world.bookkeeping.charge_battery_extended_today_date = Some(day1.naive().date());

        // Tick on the same day — flag stays set.
        let _ = process(
            &Event::Tick { at: day1.monotonic },
            &mut world,
            &day1,
            &Topology::defaults(),
        );
        assert!(world.bookkeeping.charge_battery_extended_today);

        // Advance to the next day; tick; the flag must clear.
        let day2_clock = FixedClock::new(
            day1.monotonic + StdDuration::from_secs(24 * 3600),
            NaiveDate::from_ymd_opt(2026, 4, 22)
                .unwrap()
                .and_hms_opt(2, 0, 0)
                .unwrap(),
        );
        seed_required_sensors(&mut world, day2_clock.monotonic);
        let _ = process(
            &Event::Tick { at: day2_clock.monotonic },
            &mut world,
            &day2_clock,
            &Topology::defaults(),
        );
        assert!(
            !world.bookkeeping.charge_battery_extended_today,
            "midnight rollover must clear charge_battery_extended_today"
        );

        // PR-04-D05: also assert the downstream schedules decision
        // reflects the reset — the "cbe derivation" factor must now
        // resolve to false.
        let d = world
            .decisions
            .schedule_0
            .as_ref()
            .expect("schedule decision published after midnight tick");
        let cbe = d
            .factors
            .iter()
            .find(|f| f.name == "cbe derivation")
            .expect("cbe derivation factor present");
        assert!(
            cbe.value.ends_with("= false"),
            "cbe must resolve false after midnight reset: {cbe:?}"
        );
    }

    #[test]
    fn cbe_is_false_on_fresh_boot_default() {
        // User-reported regression: out of the box, with default knobs
        // and no weather_soc run yet, `run_schedules` must derive
        // charge_battery_extended = false. The legacy
        // `!disable_night_grid_discharge` term short-circuited on the
        // `false` default and made cbe permanently true.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // Defaults: disable_night_grid_discharge=false, no weather_soc
        // run today, no weekly full charge pending.
        assert!(!world.knobs.disable_night_grid_discharge);
        assert!(!world.bookkeeping.charge_to_full_required);
        assert!(!world.bookkeeping.charge_battery_extended_today);

        let _ = process(
            &Event::Tick { at: c.monotonic },
            &mut world,
            &c,
            &Topology::defaults(),
        );

        // The "cbe derivation" factor must resolve to false on a fresh boot.
        let decision = world
            .decisions
            .schedule_0
            .as_ref()
            .expect("schedules decision recorded");
        let cbe = decision
            .factors
            .iter()
            .find(|f| f.name == "cbe derivation")
            .expect("cbe derivation factor present");
        assert!(
            cbe.value.ends_with("= false"),
            "expected cbe to resolve false on fresh boot, got {cbe:?}"
        );
    }
}
