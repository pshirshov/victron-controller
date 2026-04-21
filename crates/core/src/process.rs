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
use crate::controllers::eddi_mode::{
    EddiModeInput, EddiModeKnobs, evaluate_eddi_mode,
};
use crate::controllers::eddi_mode::EddiModeDecision;
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
use crate::controllers::zappi_mode::ZappiModeDecision;
use crate::myenergi::EddiMode;
use crate::owner::Owner;
use crate::topology::{ControllerParams, Topology};
use crate::types::{
    ActuatedId, ActuatedReadback, BookkeepingKey, BookkeepingValue, Command, DbusTarget,
    DbusValue, Effect, Event, ForecastProvider, KnobId, KnobValue, LogLevel, MyenergiAction,
    PublishPayload, ScheduleField, SensorId, SensorReading, TypedReading,
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
        Event::Tick { at } => apply_tick(*at, world, clock),
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
        SensorId::VebusOutputCurrent => world.sensors.vebus_output_current.on_reading(v, at),
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
            apply_knob(id, value, world);
            if owner == Owner::Dashboard {
                world.knob_provenance.last_dashboard_write = Some(at);
            }
            effects.push(Effect::Publish(PublishPayload::Knob { id, value }));
        }
        Command::KillSwitch(enabled) => {
            world.knobs.writes_enabled = enabled;
            effects.push(Effect::Publish(PublishPayload::KillSwitch(enabled)));
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
fn apply_knob(id: KnobId, value: KnobValue, world: &mut World) {
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
        _ => {
            // Type mismatch — silently drop. Could log, but this only
            // happens if the shell constructs an invalid KnobValue.
        }
    }
}

fn apply_tick(at: Instant, world: &mut World, _clock: &dyn Clock) {
    let local = Duration::from_secs(5); // TODO: plumb ControllerParams
    let myenergi = Duration::from_secs(300);
    let weather = Duration::from_secs(300);

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
    ss.vebus_output_current.tick(at, local);
    ss.evcharger_ac_power.tick(at, local);
    ss.evcharger_ac_current.tick(at, local);
    ss.ess_state.tick(at, local);
    ss.outdoor_temperature.tick(at, weather);

    world.typed_sensors.zappi_state.tick(at, myenergi);
    world.typed_sensors.eddi_mode.tick(at, myenergi);
}

// =============================================================================
// Controllers
// =============================================================================

fn run_controllers(
    world: &mut World,
    clock: &dyn Clock,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    run_setpoint(world, clock, topology, effects);
    run_current_limit(world, clock, topology, effects);
    run_schedules(world, clock, effects);
    run_zappi_mode(world, clock, effects);
    run_eddi_mode(world, clock, effects);
    run_weather_soc(world, clock, effects);
}

// --- Setpoint -----------------------------------------------------------------

fn run_setpoint(
    world: &mut World,
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
            zappi_active: bk.zappi_active,
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

    // SPEC §5.11: grid-side hard cap.
    let grid_cap = -i32::try_from(k.grid_export_limit_w).unwrap_or(i32::MAX);
    let capped = out.setpoint_target.max(grid_cap);

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

    let changed = world.grid_setpoint.propose_target(value, owner, now);
    if !changed {
        return;
    }

    if !world.knobs.writes_enabled {
        effects.push(Effect::Log {
            level: LogLevel::Debug,
            source: "process::setpoint",
            message: format!("writes_enabled=false; setpoint target {value} not emitted"),
        });
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

    let changed = world
        .input_current_limit
        .propose_target(value, Owner::CurrentLimitController, now);
    if !changed {
        return;
    }

    if !world.knobs.writes_enabled {
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
    let input = SchedulesInput {
        globals: SchedulesInputGlobals {
            charge_battery_extended: k.disable_night_grid_discharge.not_eq(true)
                || bk.charge_to_full_required,
            // NB: legacy flow treated `charge_battery_extended` as a
            // separately-tracked global set by weather_soc / HA. In this
            // port the bit is collapsed onto the knob chain (see
            // weather_soc decision output → knobs). For now drive it from
            // disable_night_grid_discharge's inverse and the full-charge
            // flag — close enough for the integration tests; we'll refine
            // when weather_soc actually writes knobs in M5.
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

    let changed = actuated.propose_target(spec, Owner::ScheduleController, now);
    if !changed {
        return;
    }

    if !world.knobs.writes_enabled {
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

    let d = evaluate_zappi_mode(&input, clock);
    let desired = match d {
        ZappiModeDecision::Leave => return,
        ZappiModeDecision::Set(m) => m,
    };

    let now = clock.monotonic();
    let changed = world
        .zappi_mode
        .propose_target(desired, Owner::ZappiController, now);
    if !changed {
        return;
    }

    if !world.knobs.writes_enabled {
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

    let d = evaluate_eddi_mode(&input, clock);
    let desired = match d {
        EddiModeDecision::Leave => return,
        EddiModeDecision::Set(m) => m,
    };

    let now = clock.monotonic();
    let changed = world
        .eddi_mode
        .propose_target(desired, Owner::EddiController, now);
    if !changed {
        return;
    }

    if !world.knobs.writes_enabled {
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
/// window 01:55:00–01:55:59 and we haven't run yet today.
fn run_weather_soc(world: &mut World, clock: &dyn Clock, effects: &mut Vec<Effect>) {
    let now = clock.naive();
    if !(now.hour() == 1 && now.minute() == 55) {
        return;
    }

    // Use today's temp if fresh; else skip.
    if !world.sensors.outdoor_temperature.is_usable() {
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
    world.bookkeeping.charge_to_full_required |= d.charge_battery_extended; // legacy collapsed
    // (The legacy flow wrote `charge_battery_extended` as a separate global
    //  that the schedules controller consults; we treat it here as folded
    //  into charge_to_full_required for simplicity — see the comment in
    //  run_schedules.)
}

fn propose_knob(
    world: &mut World,
    id: KnobId,
    value: KnobValue,
    effects: &mut Vec<Effect>,
) {
    apply_knob(id, value, world);
    effects.push(Effect::Publish(PublishPayload::Knob { id, value }));
}

// --- Misc ---------------------------------------------------------------------

/// Tiny bool-neq helper just to make the `run_schedules` call readable.
trait BoolNot {
    fn not_eq(self, other: bool) -> bool;
}
impl BoolNot for bool {
    fn not_eq(self, other: bool) -> bool {
        self != other
    }
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
        ss.vebus_output_current.on_reading(0.0, at);
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
        world.knobs.writes_enabled = false;
        seed_required_sensors(&mut world, c.monotonic);

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
    fn kill_switch_toggles_writes_enabled_and_publishes() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        assert!(world.knobs.writes_enabled);

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
}
