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

use std::sync::OnceLock;
use std::time::{Duration, Instant};


use crate::Clock;
use crate::core_dag::CoreRegistry;
use crate::core_dag::cores::production_cores;
use crate::controllers::current_limit::{
    CurrentLimitInput, CurrentLimitInputGlobals, evaluate_current_limit,
};
use crate::controllers::eddi_mode::{
    EddiModeInput, EddiModeKnobs, evaluate_eddi_mode,
};
use crate::controllers::schedules::{
    ScheduleSpec, SchedulesInput, SchedulesInputGlobals, evaluate_schedules,
};
use crate::controllers::setpoint::{
    SetpointInput, SetpointInputGlobals, compute_compensated_drain, evaluate_setpoint,
};
use crate::controllers::weather_soc::{
    WeatherSocInput, WeatherSocInputGlobals, evaluate_weather_soc,
};
use crate::controllers::zappi_mode::{
    ZappiModeInput, ZappiModeInputGlobals, evaluate_zappi_mode,
};
use crate::controllers::zappi_mode::ZappiModeAction;
use crate::myenergi::{EddiMode, ZappiMode};
use crate::owner::Owner;
use crate::topology::{ControllerParams, Topology};
use crate::types::{
    ActuatedId, BookkeepingKey, BookkeepingValue, Command, DbusTarget,
    DbusValue, Decision, Effect, Event, ForecastProvider, KnobId, KnobValue, LogLevel,
    MyenergiAction, PinnedStatus, PinnedValue, PublishPayload, ScheduleField, SensorId,
    SensorReading, TimerId, TimerStatus, TypedReading, ZappiDrainBranch,
};
use crate::world::TimerEntry;
use crate::world::{ForecastSnapshot, World, ZappiDrainSnapshot};

/// PR-gamma-hold-redesign: the γ-rule + per-knob hold window are gone.
/// Conflicts on the four weather_soc-driven knobs are resolved
/// declaratively via the `*_mode` selectors instead. Knob writes from
/// any owner are accepted unconditionally (subject to type validity).
///
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
        Event::Sensor(reading) => apply_sensor_reading(*reading, world, topology, effects),
        Event::TypedSensor(reading) => apply_typed_reading(reading.clone(), world, effects),
        Event::ScheduleReadback { index, value, at } => {
            apply_schedule_readback(*index, *value, *at, world, effects);
        }
        Event::Command {
            command,
            owner,
            at,
        } => apply_command(*command, *owner, *at, world, effects),
        Event::Tick { at } => apply_tick(*at, world, clock, topology),
        Event::TimerState {
            id,
            last_fire_epoch_ms,
            next_fire_epoch_ms,
            status,
            at: _,
        } => apply_timer_state(
            *id,
            *last_fire_epoch_ms,
            *next_fire_epoch_ms,
            *status,
            world,
        ),
        Event::Timezone { value, at } => {
            apply_timezone(value, *at, world, topology, effects);
        }
        Event::SunriseSunset { sunrise, sunset, at } => {
            world.sunrise = Some(*sunrise);
            world.sunset = Some(*sunset);
            world.sunrise_sunset_updated_at = Some(*at);
        }
        Event::PinnedRegisterReading { path, value, at } => {
            apply_pinned_register_reading(path, value, *at, world, effects);
        }
    }
}

/// PR-tz-from-victron: validate + apply the Victron-supplied display
/// timezone. On a parseable IANA name we update `world.timezone`,
/// bump `timezone_updated_at`, and atomically swap the parsed Tz into
/// `topology.tz_handle` so the shell-side `RealClock::naive()` reads
/// it on its next call. On parse failure we log a Warn and leave both
/// world state and the live handle untouched (controller continues
/// with the previously-loaded Tz, or UTC at boot).
fn apply_timezone(
    value: &str,
    at: Instant,
    world: &mut World,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    use std::str::FromStr;
    match chrono_tz::Tz::from_str(value) {
        Ok(tz) => {
            if world.timezone != value {
                world.timezone = value.to_string();
            }
            world.timezone_updated_at = Some(at);
            topology.tz_handle.set(tz);
        }
        Err(e) => {
            effects.push(Effect::Log {
                level: LogLevel::Warn,
                source: "timezone",
                message: format!(
                    "invalid TZ from Victron: {value:?} — {e}; keeping previous"
                ),
            });
        }
    }
}

/// PR-timers-section: upsert the per-timer entry in `world.timers`.
/// Period is preserved across updates from the first time the entry was
/// seen — the shell currently doesn't surface period in the event itself
/// because the cadence is fixed per-task — so we keep whatever was there
/// (or default to zero on first observation).
fn apply_timer_state(
    id: TimerId,
    last_fire_epoch_ms: i64,
    next_fire_epoch_ms: Option<i64>,
    status: TimerStatus,
    world: &mut World,
) {
    let entry = world
        .timers
        .entries
        .entry(id)
        .or_insert_with(|| TimerEntry {
            period: std::time::Duration::ZERO,
            last_fire_epoch_ms: None,
            next_fire_epoch_ms: None,
            status: TimerStatus::Idle,
        });
    entry.last_fire_epoch_ms = Some(last_fire_epoch_ms);
    entry.next_fire_epoch_ms = next_fire_epoch_ms;
    entry.status = status;
    if let (Some(last), Some(next)) = (entry.last_fire_epoch_ms, entry.next_fire_epoch_ms) {
        if next > last {
            // Period inferred from the spacing between last and next fire.
            // Saturating cast — period in ms easily fits an i64 → u64.
            let dur_ms = u64::try_from(next - last).unwrap_or(0);
            entry.period = std::time::Duration::from_millis(dur_ms);
        }
    }
}

/// PR-pinned-registers: handle a fresh reading of a pinned register.
///
/// 1. Look up the configured entity by the joined `service:dbus_path`.
///    Unknown paths are dropped with a Warn log — the only way to see
///    one is a config drift between the shell's seeded set and a write
///    arriving via a stale `Event` (shouldn't happen in practice, but
///    fail visibly rather than silently).
/// 2. Stamp `actual = Some(value)`, `last_check = Some(at)`.
/// 3. Compare `value` against `target` using `PinnedValue::approx_eq`
///    (float tolerance + bool/int coercion).
/// 4. On match: status becomes `Confirmed`. `drift_count` is intentionally
///    NOT reset — the operator-facing "how many times has this drifted
///    since boot" counter only ever increases.
/// 5. On mismatch: status becomes `Drifted`, `drift_count += 1`,
///    `last_drift_at = Some(at)`. Emit a corrective `Effect::WriteDbusPinned`
///    plus a `Decision`-equivalent `Effect::Log(Info)` summarising the
///    register / old / new triplet (the honesty invariant — every
///    actuating effect must explain itself).
fn apply_pinned_register_reading(
    path: &str,
    value: &PinnedValue,
    at: chrono::NaiveDateTime,
    world: &mut World,
    effects: &mut Vec<Effect>,
) {
    // BTreeMap-key lookup via &str: build an Arc<str> on the fly. The
    // lookup is on a hot-ish path (one per pinned register per hour
    // per process) so the cost is irrelevant.
    let key: std::sync::Arc<str> = std::sync::Arc::from(path);
    let Some(entity) = world.pinned_registers.get_mut(&key) else {
        effects.push(Effect::Log {
            level: LogLevel::Warn,
            source: "pinned_registers",
            message: format!(
                "reading for unconfigured pinned register {path:?}; dropped"
            ),
        });
        return;
    };
    entity.actual = Some(value.clone());
    entity.last_check = Some(at);
    if entity.target.approx_eq(value) {
        entity.status = PinnedStatus::Confirmed;
        return;
    }
    entity.status = PinnedStatus::Drifted;
    entity.drift_count = entity.drift_count.saturating_add(1);
    entity.last_drift_at = Some(at);

    // Split path back into (service, dbus_path) for the write effect.
    // The shell-side validator guarantees the colon shape; defensive
    // fallback emits a log + skips the write rather than panicking if
    // somehow a malformed key snuck in.
    let Some((service, dbus_path)) = path.split_once(':') else {
        effects.push(Effect::Log {
            level: LogLevel::Error,
            source: "pinned_registers",
            message: format!(
                "pinned register {path:?} has no service:path separator; \
                 cannot emit corrective write"
            ),
        });
        return;
    };

    let target_str = format!("{}", entity.target);
    let actual_str = format!("{value}");
    effects.push(Effect::WriteDbusPinned {
        service: service.to_string(),
        path: dbus_path.to_string(),
        value: entity.target.clone(),
    });
    // Honesty invariant: every actuating effect must explain (a) which
    // register, (b) old value, (c) new value. We use Effect::Log
    // because the per-register table on the dashboard already shows
    // status / drift_count / last_drift, and the pinned-register
    // controller has no slot in `world.decisions`.
    effects.push(Effect::Log {
        level: LogLevel::Info,
        source: "pinned_registers",
        message: format!(
            "pinned_register_restored: {path} actual={actual_str} -> target={target_str}"
        ),
    });
}

/// PR-ZD-1 / D01: validate a Victron `/MppOperationMode` reading.
/// The documented enum is 0–5 and always integral. Non-finite, out-of-range,
/// or fractional values indicate a D-Bus decode error and must be dropped.
fn mppt_operation_mode_in_range(v: f64) -> bool {
    v.is_finite() && (0.0..=5.0).contains(&v) && (v - v.round()).abs() < 1e-6
}

fn apply_sensor_reading(
    r: SensorReading,
    world: &mut World,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
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
        // PR-keep-batteries-charged: feed both the primary sensor slot
        // *and* the actuated-target's `actual` side. The actuated entry
        // exists for TASS phase tracking on the daytime override; we
        // keep `SensorId::EssState.actuated_id() == None` so the sensor
        // table / HA sensor entity continues to surface ess_state.
        SensorId::EssState => {
            world.sensors.ess_state.on_reading(v, at);
            #[allow(clippy::cast_possible_truncation)]
            let value = v as i32;
            world.ess_state_target.on_reading(value, at);
            if world
                .ess_state_target
                .confirm_if(|t, a| t == a, at)
            {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::EssStateTarget,
                    phase: world.ess_state_target.target.phase,
                }));
            }
        }
        SensorId::OutdoorTemperature => world.sensors.outdoor_temperature.on_reading(v, at),
        SensorId::SessionKwh => world.sensors.session_kwh.on_reading(v, at),
        // PR-ev-soc-sensor.
        SensorId::EvSoc => world.sensors.ev_soc.on_reading(v, at),
        // PR-auto-extended-charge.
        SensorId::EvChargeTarget => world.sensors.ev_charge_target.on_reading(v, at),
        // PR-ZD-1.
        SensorId::HeatPumpPower => world.sensors.heat_pump_power.on_reading(v, at),
        SensorId::CookerPower => world.sensors.cooker_power.on_reading(v, at),
        // PR-ZD-1 / D01: drop readings outside the documented [0, 5] integer enum.
        // Corrupt values are ignored so the freshness window expires the slot
        // (signalling Stale to the dashboard) rather than overwriting with garbage.
        SensorId::Mppt0OperationMode => {
            if mppt_operation_mode_in_range(v) {
                world.sensors.mppt_0_operation_mode.on_reading(v, at);
            } else {
                effects.push(Effect::Log {
                    level: LogLevel::Warn,
                    source: "apply_sensor_reading",
                    message: format!(
                        "Mppt0OperationMode reading out of expected enum range [0, 5]: {v}; dropping"
                    ),
                });
            }
        }
        SensorId::Mppt1OperationMode => {
            if mppt_operation_mode_in_range(v) {
                world.sensors.mppt_1_operation_mode.on_reading(v, at);
            } else {
                effects.push(Effect::Log {
                    level: LogLevel::Warn,
                    source: "apply_sensor_reading",
                    message: format!(
                        "Mppt1OperationMode reading out of expected enum range [0, 5]: {v}; dropping"
                    ),
                });
            }
        }
        // PR-actuated-as-sensors (PR-AS-A): the actuated-mirror sensor
        // variants don't have dedicated `world.sensors.<field>` slots —
        // their storage of truth is `world.<entity>.actual`, driven by
        // the post-hook below.
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
        | SensorId::Schedule1AllowDischargeActual => {}
    }

    // PR-actuated-as-sensors (PR-AS-A): post-update hook. If this
    // sensor mirrors an actuated entity, drive the matching
    // `world.<entity>.actual.on_reading + confirm_if`. Schedules go
    // through the dedicated `Event::ScheduleReadback` rollup instead
    // (the per-leaf reading can't be combined with a `ScheduleSpec`
    // confirm_if predicate). Zappi/Eddi never enter this pipeline.
    match r.id.actuated_id() {
        None => {}
        Some(ActuatedId::GridSetpoint) => {
            #[allow(clippy::cast_possible_truncation)]
            let value = v as i32;
            world.grid_setpoint.on_reading(value, at);
            let tol = topology.controller_params.setpoint_confirm_tolerance_w;
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
        Some(ActuatedId::InputCurrentLimit) => {
            world.input_current_limit.on_reading(v, at);
            let tol = topology.controller_params.current_limit_confirm_tolerance_a;
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
        Some(other) => {
            // Schedule leaves return `Some(Schedule0/1)` so the broadcast
            // filter (`actuated_id().is_some()`) catches every actuated
            // mirror; per-leaf confirmation isn't possible (a complete
            // `ScheduleSpec` is required) — the rollup arrives via
            // `Event::ScheduleReadback`. ZappiMode/EddiMode have no
            // matching `*Actual` sensor and never reach this branch.
            // `EssStateTarget` is fed by its own dedicated arm in the
            // sensor-id match above (PR-keep-batteries-charged), not here.
            debug_assert!(matches!(
                other,
                ActuatedId::Schedule0 | ActuatedId::Schedule1,
            ));
        }
    }
}

/// PR-actuated-as-sensors (PR-AS-A): handle the rolled-up schedule
/// readback emitted by the subscriber-side accumulator.
fn apply_schedule_readback(
    index: u8,
    value: ScheduleSpec,
    at: Instant,
    world: &mut World,
    effects: &mut Vec<Effect>,
) {
    let (actuated, id) = match index {
        0 => (&mut world.schedule_0, ActuatedId::Schedule0),
        1 => (&mut world.schedule_1, ActuatedId::Schedule1),
        _ => return,
    };
    actuated.on_reading(value, at);
    if actuated.confirm_if(|t, a| t == a, at) {
        effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
            id,
            phase: actuated.target.phase,
        }));
    }
}

fn apply_typed_reading(r: TypedReading, world: &mut World, effects: &mut Vec<Effect>) {
    match r {
        TypedReading::Zappi { state, at, raw_json } => {
            world.typed_sensors.zappi_state.on_reading(state, at);
            // PR-EDDI-SENSORS-1: only overwrite the latched body when
            // this poll carried one — `None` means "no new body this
            // cycle", not "clear the body". The latched value
            // intentionally outlives freshness decay so the operator
            // can paste the last good body into a bug report.
            if raw_json.is_some() {
                world.typed_sensors.zappi_raw_json = raw_json;
            }
            // Mirror onto the actuated side so confirm_if can promote
            // Pending/Commanded → Confirmed when the device's reported
            // mode matches the controller's target. Without this hook
            // the actuated phase has no upgrade path on a typed-sensor
            // ingestion path (M-AS unified the D-Bus sensors but
            // myenergi typed sensors live on a sibling pipeline).
            world.zappi_mode.on_reading(state.zappi_mode, at);
            if world.zappi_mode.confirm_if(|t, a| t == a, at) {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::ZappiMode,
                    phase: world.zappi_mode.target.phase,
                }));
            }
        }
        TypedReading::Eddi { mode, at, raw_json } => {
            world.typed_sensors.eddi_mode.on_reading(mode, at);
            // PR-EDDI-SENSORS-1: same latch-on-Some logic as Zappi above.
            if raw_json.is_some() {
                world.typed_sensors.eddi_raw_json = raw_json;
            }
            // Same mirror as Zappi above.
            world.eddi_mode.on_reading(mode, at);
            if world.eddi_mode.confirm_if(|t, a| t == a, at) {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::EddiMode,
                    phase: world.eddi_mode.target.phase,
                }));
            }
        }
        TypedReading::Forecast {
            provider,
            today_kwh,
            tomorrow_kwh,
            hourly_kwh,
            at,
        } => {
            let snap = ForecastSnapshot {
                today_kwh,
                tomorrow_kwh,
                fetched_at: at,
                hourly_kwh,
            };
            match provider {
                ForecastProvider::Solcast => world.typed_sensors.forecast_solcast = Some(snap),
                ForecastProvider::ForecastSolar => {
                    world.typed_sensors.forecast_forecast_solar = Some(snap);
                }
                ForecastProvider::OpenMeteo => {
                    world.typed_sensors.forecast_open_meteo = Some(snap);
                }
                ForecastProvider::Baseline => {
                    world.typed_sensors.forecast_baseline = Some(snap);
                }
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
    let _ = (owner, at);
    match command {
        Command::Knob { id, value } => {
            // PR-gamma-hold-redesign: knob writes are accepted from any
            // owner. The four weather_soc-driven outputs are arbitrated
            // by the `*_mode` selectors at read-time, not at write-time.
            let changed = apply_knob(id, value, world, effects);
            // Skip the retained-MQTT publish on no-op writes; otherwise
            // any every-tick caller would spam the broker with redundant
            // retains.
            if changed {
                effects.push(Effect::Publish(PublishPayload::Knob { id, value }));
            }
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
        Command::SetBookkeeping { key, value } => {
            apply_set_bookkeeping(key, value, world, effects);
        }
    }
}

/// User-driven bookkeeping edit. Allowlists `(key, value-shape)` pairs:
/// only `NextFullCharge` is editable today, with either a `NaiveDateTime`
/// (set) or `Cleared` (set-to-None). Anything else is dropped with a
/// Warn log so an unexpected wire combination is observable rather than
/// silently mutating state. Accepted edits mutate the world AND emit a
/// `PublishPayload::Bookkeeping` effect so the new value survives a
/// restart via the existing retained-MQTT path. An additional Info log
/// documents what changed for the operator.
fn apply_set_bookkeeping(
    key: BookkeepingKey,
    value: BookkeepingValue,
    world: &mut World,
    effects: &mut Vec<Effect>,
) {
    let accepted = matches!(
        (key, value),
        (
            BookkeepingKey::NextFullCharge,
            BookkeepingValue::NaiveDateTime(_) | BookkeepingValue::Cleared,
        ),
    );
    if !accepted {
        effects.push(Effect::Log {
            level: LogLevel::Warn,
            source: "process::command",
            message: format!("SetBookkeeping rejected: ({key:?}, {value:?})"),
        });
        return;
    }
    apply_bookkeeping(key, value, world);
    effects.push(Effect::Publish(PublishPayload::Bookkeeping(key, value)));
    effects.push(Effect::Log {
        level: LogLevel::Info,
        source: "bookkeeping.edit",
        message: format!("SetBookkeeping accepted: ({key:?}, {value:?})"),
    });
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
        _ => {
            // Type mismatch — retained payload shape doesn't match the
            // key's expected shape. Silently drop; the controllers will
            // rebuild state on first tick.
        }
    }
}

// PR-gamma-hold-redesign: `accept_knob_command` is gone — the γ-hold
// + per-knob provenance map it gated on were the load-bearing pieces
// of the legacy "owner priority" model. With the new design the user
// flips `*_mode` to `Forced` to pin a knob; there is no implicit
// "newer-from-X-beats-Y" rule, so every knob write is accepted.

#[allow(clippy::too_many_lines)]
/// Apply a `(KnobId, KnobValue)` to `world.knobs`. Returns `true` when
/// the value actually changed; callers gate their `Effect::Publish` on
/// the return so we never spam retained MQTT with no-op writes — this
/// is what makes `propose_knob` safe to invoke every tick (the dynamic
/// weather_soc evaluation per PR-weather-soc-dynamic).
fn apply_knob(id: KnobId, value: KnobValue, world: &mut World, effects: &mut Vec<Effect>) -> bool {
    use std::mem::replace;
    let k = &mut world.knobs;
    match (id, value) {
        (KnobId::ForceDisableExport, KnobValue::Bool(v)) => replace(&mut k.force_disable_export, v) != v,
        (KnobId::ExportSocThreshold, KnobValue::Float(v)) => replace(&mut k.export_soc_threshold, v) != v,
        (KnobId::DischargeSocTarget, KnobValue::Float(v)) => replace(&mut k.discharge_soc_target, v) != v,
        (KnobId::BatterySocTarget, KnobValue::Float(v)) => replace(&mut k.battery_soc_target, v) != v,
        (KnobId::FullChargeDischargeSocTarget, KnobValue::Float(v)) => {
            replace(&mut k.full_charge_discharge_soc_target, v) != v
        }
        (KnobId::FullChargeExportSocThreshold, KnobValue::Float(v)) => {
            replace(&mut k.full_charge_export_soc_threshold, v) != v
        }
        (KnobId::DischargeTime, KnobValue::DischargeTime(v)) => replace(&mut k.discharge_time, v) != v,
        (KnobId::DebugFullCharge, KnobValue::DebugFullCharge(v)) => replace(&mut k.debug_full_charge, v) != v,
        (KnobId::PessimismMultiplierModifier, KnobValue::Float(v)) => {
            replace(&mut k.pessimism_multiplier_modifier, v) != v
        }
        (KnobId::DisableNightGridDischarge, KnobValue::Bool(v)) => replace(&mut k.disable_night_grid_discharge, v) != v,
        (KnobId::ChargeCarBoost, KnobValue::Bool(v)) => replace(&mut k.charge_car_boost, v) != v,
        // PR-auto-extended-charge.
        (KnobId::ChargeCarExtendedMode, KnobValue::ExtendedChargeMode(v)) => {
            replace(&mut k.charge_car_extended_mode, v) != v
        }
        (KnobId::ZappiCurrentTarget, KnobValue::Float(v)) => replace(&mut k.zappi_current_target, v) != v,
        (KnobId::ZappiLimit, KnobValue::Float(v)) => replace(&mut k.zappi_limit, v) != v,
        (KnobId::ZappiEmergencyMargin, KnobValue::Float(v)) => replace(&mut k.zappi_emergency_margin, v) != v,
        (KnobId::GridExportLimitW, KnobValue::Uint32(v)) => replace(&mut k.grid_export_limit_w, v) != v,
        (KnobId::GridImportLimitW, KnobValue::Uint32(v)) => replace(&mut k.grid_import_limit_w, v) != v,
        (KnobId::AllowBatteryToCar, KnobValue::Bool(v)) => replace(&mut k.allow_battery_to_car, v) != v,
        (KnobId::EddiEnableSoc, KnobValue::Float(v)) => replace(&mut k.eddi_enable_soc, v) != v,
        (KnobId::EddiDisableSoc, KnobValue::Float(v)) => replace(&mut k.eddi_disable_soc, v) != v,
        (KnobId::EddiDwellS, KnobValue::Uint32(v)) => replace(&mut k.eddi_dwell_s, v) != v,
        (KnobId::WeathersocWinterTemperatureThreshold, KnobValue::Float(v)) => {
            replace(&mut k.weathersoc_winter_temperature_threshold, v) != v
        }
        (KnobId::WeathersocLowEnergyThreshold, KnobValue::Float(v)) => {
            replace(&mut k.weathersoc_low_energy_threshold, v) != v
        }
        (KnobId::WeathersocOkEnergyThreshold, KnobValue::Float(v)) => {
            replace(&mut k.weathersoc_ok_energy_threshold, v) != v
        }
        (KnobId::WeathersocHighEnergyThreshold, KnobValue::Float(v)) => {
            replace(&mut k.weathersoc_high_energy_threshold, v) != v
        }
        (KnobId::WeathersocTooMuchEnergyThreshold, KnobValue::Float(v)) => {
            replace(&mut k.weathersoc_too_much_energy_threshold, v) != v
        }
        // PR-WSOC-TABLE-1: bucket-boundary kWh knob.
        (KnobId::WeathersocVerySunnyThreshold, KnobValue::Float(v)) => {
            replace(&mut k.weathersoc_very_sunny_threshold, v) != v
        }
        (KnobId::ForecastDisagreementStrategy, KnobValue::ForecastDisagreementStrategy(v)) => {
            replace(&mut k.forecast_disagreement_strategy, v) != v
        }
        (KnobId::ChargeBatteryExtendedMode, KnobValue::ChargeBatteryExtendedMode(v)) => {
            replace(&mut k.charge_battery_extended_mode, v) != v
        }
        // PR-gamma-hold-redesign — the four mode selectors.
        (KnobId::ExportSocThresholdMode, KnobValue::Mode(v)) => {
            replace(&mut k.export_soc_threshold_mode, v) != v
        }
        (KnobId::DischargeSocTargetMode, KnobValue::Mode(v)) => {
            replace(&mut k.discharge_soc_target_mode, v) != v
        }
        (KnobId::BatterySocTargetMode, KnobValue::Mode(v)) => {
            replace(&mut k.battery_soc_target_mode, v) != v
        }
        (KnobId::DisableNightGridDischargeMode, KnobValue::Mode(v)) => {
            replace(&mut k.disable_night_grid_discharge_mode, v) != v
        }
        // PR-inverter-safe-discharge-knob.
        (KnobId::InverterSafeDischargeEnable, KnobValue::Bool(v)) => {
            replace(&mut k.inverter_safe_discharge_enable, v) != v
        }
        // PR-baseline-forecast.
        (KnobId::BaselineWinterStartMmDd, KnobValue::Uint32(v)) => {
            replace(&mut k.baseline_winter_start_mm_dd, v) != v
        }
        (KnobId::BaselineWinterEndMmDd, KnobValue::Uint32(v)) => {
            replace(&mut k.baseline_winter_end_mm_dd, v) != v
        }
        (KnobId::BaselineWhPerHourWinter, KnobValue::Float(v)) => {
            replace(&mut k.baseline_wh_per_hour_winter, v) != v
        }
        (KnobId::BaselineWhPerHourSummer, KnobValue::Float(v)) => {
            replace(&mut k.baseline_wh_per_hour_summer, v) != v
        }
        // PR-keep-batteries-charged.
        (KnobId::KeepBatteriesChargedDuringFullCharge, KnobValue::Bool(v)) => {
            replace(&mut k.keep_batteries_charged_during_full_charge, v) != v
        }
        (KnobId::SunriseSunsetOffsetMin, KnobValue::Uint32(v)) => {
            replace(&mut k.sunrise_sunset_offset_min, v) != v
        }
        (KnobId::FullChargeDeferToNextSunday, KnobValue::Bool(v)) => {
            replace(&mut k.full_charge_defer_to_next_sunday, v) != v
        }
        (KnobId::FullChargeSnapBackMaxWeekday, KnobValue::Uint32(v)) => {
            replace(&mut k.full_charge_snap_back_max_weekday, v) != v
        }
        // PR-ZD-2: compensated battery-drain feedback loop.
        // `target_w` is i32 but routes via Float because KnobValue has
        // no Int32 variant; the controller rounds to nearest W on read.
        (KnobId::ZappiBatteryDrainThresholdW, KnobValue::Uint32(v)) => {
            replace(&mut k.zappi_battery_drain_threshold_w, v) != v
        }
        (KnobId::ZappiBatteryDrainRelaxStepW, KnobValue::Uint32(v)) => {
            replace(&mut k.zappi_battery_drain_relax_step_w, v) != v
        }
        (KnobId::ZappiBatteryDrainHardClampW, KnobValue::Uint32(v)) => {
            replace(&mut k.zappi_battery_drain_hard_clamp_w, v) != v
        }
        (KnobId::ZappiBatteryDrainKp, KnobValue::Float(v)) => {
            replace(&mut k.zappi_battery_drain_kp, v) != v
        }
        (KnobId::ZappiBatteryDrainTargetW, KnobValue::Float(v)) => {
            replace(&mut k.zappi_battery_drain_target_w, v.round() as i32) != v.round() as i32
        }
        // PR-ZDP-1.
        (KnobId::ZappiBatteryDrainMpptProbeW, KnobValue::Uint32(v)) => {
            replace(&mut k.zappi_battery_drain_mppt_probe_w, v) != v
        }
        // PR-ACT-RETRY-1.
        (KnobId::ActuatorRetryS, KnobValue::Uint32(v)) => replace(&mut k.actuator_retry_s, v) != v,
        // PR-WSOC-EDIT-1: programmatic per-cell write — one arm covers
        // 48 distinct addressable knobs (12 cells × 4 fields). The
        // (field, value) pair routes to the right field on the cell
        // resolved by `(bucket, temp)`; type mismatch (e.g. Float →
        // Extended bool) falls through to the catch-all warn.
        (KnobId::WeathersocTableCell { bucket, temp, field }, value) => {
            use crate::controllers::weather_soc::cell_mut;
            use crate::weather_soc_addr::CellField;
            let cell = cell_mut(&mut k.weather_soc_table, bucket, temp);
            match (field, value) {
                (CellField::ExportSocThreshold, KnobValue::Float(v)) => {
                    replace(&mut cell.export_soc_threshold, v) != v
                }
                (CellField::BatterySocTarget, KnobValue::Float(v)) => {
                    replace(&mut cell.battery_soc_target, v) != v
                }
                (CellField::DischargeSocTarget, KnobValue::Float(v)) => {
                    replace(&mut cell.discharge_soc_target, v) != v
                }
                (CellField::Extended, KnobValue::Bool(v)) => {
                    replace(&mut cell.extended, v) != v
                }
                _ => {
                    effects.push(Effect::Log {
                        level: LogLevel::Warn,
                        source: "process::command",
                        message: format!(
                            "apply_knob: WeathersocTableCell field/value mismatch — silently dropped \
                             (schema drift?) bucket={bucket:?} temp={temp:?} field={field:?} value={value:?}"
                        ),
                    });
                    false
                }
            }
        }
        _ => {
            effects.push(Effect::Log {
                level: LogLevel::Warn,
                source: "process::command",
                message: format!(
                    "apply_knob: KnobId/KnobValue type mismatch — silently dropped (schema drift?) id={id:?} value={value:?}"
                ),
            });
            false
        }
    }
}

/// PR-ha-knob-sync: enumerate `(KnobId, KnobValue)` pairs covering EVERY
/// user-controllable knob, packaged as `PublishPayload::Knob`. The shell
/// invokes this once at boot (after the retained-MQTT bootstrap window
/// closes) and pushes each payload to the broker so HA's MQTT integration
/// sees a retained `<root>/knob/<name>/state` for every knob — pre-fix
/// only knobs that the user had ever edited had a retained payload, so
/// HA's UI showed "unknown" for the rest.
#[must_use]
pub fn all_knob_publish_payloads(knobs: &crate::knobs::Knobs) -> Vec<PublishPayload> {
    use KnobId as I;
    use KnobValue as V;
    let k = knobs;
    let mut out: Vec<PublishPayload> = vec![
        PublishPayload::Knob { id: I::ForceDisableExport, value: V::Bool(k.force_disable_export) },
        PublishPayload::Knob { id: I::ExportSocThreshold, value: V::Float(k.export_soc_threshold) },
        PublishPayload::Knob { id: I::DischargeSocTarget, value: V::Float(k.discharge_soc_target) },
        PublishPayload::Knob { id: I::BatterySocTarget, value: V::Float(k.battery_soc_target) },
        PublishPayload::Knob {
            id: I::FullChargeDischargeSocTarget,
            value: V::Float(k.full_charge_discharge_soc_target),
        },
        PublishPayload::Knob {
            id: I::FullChargeExportSocThreshold,
            value: V::Float(k.full_charge_export_soc_threshold),
        },
        PublishPayload::Knob { id: I::DischargeTime, value: V::DischargeTime(k.discharge_time) },
        PublishPayload::Knob { id: I::DebugFullCharge, value: V::DebugFullCharge(k.debug_full_charge) },
        PublishPayload::Knob {
            id: I::PessimismMultiplierModifier,
            value: V::Float(k.pessimism_multiplier_modifier),
        },
        PublishPayload::Knob {
            id: I::DisableNightGridDischarge,
            value: V::Bool(k.disable_night_grid_discharge),
        },
        PublishPayload::Knob { id: I::ChargeCarBoost, value: V::Bool(k.charge_car_boost) },
        // PR-auto-extended-charge.
        PublishPayload::Knob {
            id: I::ChargeCarExtendedMode,
            value: V::ExtendedChargeMode(k.charge_car_extended_mode),
        },
        PublishPayload::Knob { id: I::ZappiCurrentTarget, value: V::Float(k.zappi_current_target) },
        PublishPayload::Knob { id: I::ZappiLimit, value: V::Float(k.zappi_limit) },
        PublishPayload::Knob { id: I::ZappiEmergencyMargin, value: V::Float(k.zappi_emergency_margin) },
        PublishPayload::Knob { id: I::GridExportLimitW, value: V::Uint32(k.grid_export_limit_w) },
        PublishPayload::Knob { id: I::GridImportLimitW, value: V::Uint32(k.grid_import_limit_w) },
        PublishPayload::Knob { id: I::AllowBatteryToCar, value: V::Bool(k.allow_battery_to_car) },
        PublishPayload::Knob { id: I::EddiEnableSoc, value: V::Float(k.eddi_enable_soc) },
        PublishPayload::Knob { id: I::EddiDisableSoc, value: V::Float(k.eddi_disable_soc) },
        PublishPayload::Knob { id: I::EddiDwellS, value: V::Uint32(k.eddi_dwell_s) },
        PublishPayload::Knob {
            id: I::WeathersocWinterTemperatureThreshold,
            value: V::Float(k.weathersoc_winter_temperature_threshold),
        },
        PublishPayload::Knob {
            id: I::WeathersocLowEnergyThreshold,
            value: V::Float(k.weathersoc_low_energy_threshold),
        },
        PublishPayload::Knob {
            id: I::WeathersocOkEnergyThreshold,
            value: V::Float(k.weathersoc_ok_energy_threshold),
        },
        PublishPayload::Knob {
            id: I::WeathersocHighEnergyThreshold,
            value: V::Float(k.weathersoc_high_energy_threshold),
        },
        PublishPayload::Knob {
            id: I::WeathersocTooMuchEnergyThreshold,
            value: V::Float(k.weathersoc_too_much_energy_threshold),
        },
        // PR-WSOC-TABLE-1: bucket-boundary kWh knob.
        PublishPayload::Knob {
            id: I::WeathersocVerySunnyThreshold,
            value: V::Float(k.weathersoc_very_sunny_threshold),
        },
        PublishPayload::Knob {
            id: I::ForecastDisagreementStrategy,
            value: V::ForecastDisagreementStrategy(k.forecast_disagreement_strategy),
        },
        PublishPayload::Knob {
            id: I::ChargeBatteryExtendedMode,
            value: V::ChargeBatteryExtendedMode(k.charge_battery_extended_mode),
        },
        // PR-gamma-hold-redesign — four mode selectors.
        PublishPayload::Knob {
            id: I::ExportSocThresholdMode,
            value: V::Mode(k.export_soc_threshold_mode),
        },
        PublishPayload::Knob {
            id: I::DischargeSocTargetMode,
            value: V::Mode(k.discharge_soc_target_mode),
        },
        PublishPayload::Knob {
            id: I::BatterySocTargetMode,
            value: V::Mode(k.battery_soc_target_mode),
        },
        PublishPayload::Knob {
            id: I::DisableNightGridDischargeMode,
            value: V::Mode(k.disable_night_grid_discharge_mode),
        },
        // PR-inverter-safe-discharge-knob.
        PublishPayload::Knob {
            id: I::InverterSafeDischargeEnable,
            value: V::Bool(k.inverter_safe_discharge_enable),
        },
        // PR-baseline-forecast: 4 runtime knobs.
        PublishPayload::Knob {
            id: I::BaselineWinterStartMmDd,
            value: V::Uint32(k.baseline_winter_start_mm_dd),
        },
        PublishPayload::Knob {
            id: I::BaselineWinterEndMmDd,
            value: V::Uint32(k.baseline_winter_end_mm_dd),
        },
        PublishPayload::Knob {
            id: I::BaselineWhPerHourWinter,
            value: V::Float(k.baseline_wh_per_hour_winter),
        },
        PublishPayload::Knob {
            id: I::BaselineWhPerHourSummer,
            value: V::Float(k.baseline_wh_per_hour_summer),
        },
        PublishPayload::Knob {
            id: I::FullChargeDeferToNextSunday,
            value: V::Bool(k.full_charge_defer_to_next_sunday),
        },
        PublishPayload::Knob {
            id: I::FullChargeSnapBackMaxWeekday,
            value: V::Uint32(k.full_charge_snap_back_max_weekday),
        },
        // PR-ZD-2: compensated battery-drain feedback loop.
        PublishPayload::Knob {
            id: I::ZappiBatteryDrainThresholdW,
            value: V::Uint32(k.zappi_battery_drain_threshold_w),
        },
        PublishPayload::Knob {
            id: I::ZappiBatteryDrainRelaxStepW,
            value: V::Uint32(k.zappi_battery_drain_relax_step_w),
        },
        PublishPayload::Knob {
            id: I::ZappiBatteryDrainKp,
            value: V::Float(k.zappi_battery_drain_kp),
        },
        PublishPayload::Knob {
            id: I::ZappiBatteryDrainTargetW,
            value: V::Float(f64::from(k.zappi_battery_drain_target_w)),
        },
        PublishPayload::Knob {
            id: I::ZappiBatteryDrainHardClampW,
            value: V::Uint32(k.zappi_battery_drain_hard_clamp_w),
        },
        // PR-ZDP-1.
        PublishPayload::Knob {
            id: I::ZappiBatteryDrainMpptProbeW,
            value: V::Uint32(k.zappi_battery_drain_mppt_probe_w),
        },
        // PR-ACT-RETRY-1.
        PublishPayload::Knob {
            id: I::ActuatorRetryS,
            value: V::Uint32(k.actuator_retry_s),
        },
    ];
    // PR-WSOC-EDIT-1: append the 48 cell knobs. Programmatic
    // enumeration over the cartesian product
    // EnergyBucket::ALL × TempCol::ALL × CellField::ALL — single source
    // of truth for the bucket / column / field set.
    {
        use crate::controllers::weather_soc::cell_mut;
        use crate::weather_soc_addr::{CellField, EnergyBucket, TempCol};
        // `cell_mut` takes `&mut`; for read-only enumeration we go
        // through a temporary clone of the table so we get an owned
        // `WeatherSocCell` to read each field from. The clone is cheap
        // (36 f64 + 12 bool).
        let mut tmp = k.weather_soc_table;
        for &bucket in EnergyBucket::ALL {
            for &temp in TempCol::ALL {
                let cell = *cell_mut(&mut tmp, bucket, temp);
                for &field in CellField::ALL {
                    let id = I::WeathersocTableCell { bucket, temp, field };
                    let value = match field {
                        CellField::ExportSocThreshold => V::Float(cell.export_soc_threshold),
                        CellField::BatterySocTarget => V::Float(cell.battery_soc_target),
                        CellField::DischargeSocTarget => V::Float(cell.discharge_soc_target),
                        CellField::Extended => V::Bool(cell.extended),
                    };
                    out.push(PublishPayload::Knob { id, value });
                }
            }
        }
    }
    out
}

fn apply_tick(at: Instant, world: &mut World, clock: &dyn Clock, topology: &Topology) {
    use crate::types::SensorId;
    let p = topology.controller_params;
    let myenergi = p.freshness_myenergi;

    let ss = &mut world.sensors;
    ss.battery_soc.tick(at, SensorId::BatterySoc.freshness_threshold());
    ss.battery_soh.tick(at, SensorId::BatterySoh.freshness_threshold());
    ss.battery_installed_capacity
        .tick(at, SensorId::BatteryInstalledCapacity.freshness_threshold());
    ss.battery_dc_power
        .tick(at, SensorId::BatteryDcPower.freshness_threshold());
    ss.mppt_power_0.tick(at, SensorId::MpptPower0.freshness_threshold());
    ss.mppt_power_1.tick(at, SensorId::MpptPower1.freshness_threshold());
    ss.soltaro_power
        .tick(at, SensorId::SoltaroPower.freshness_threshold());
    ss.power_consumption
        .tick(at, SensorId::PowerConsumption.freshness_threshold());
    ss.grid_power.tick(at, SensorId::GridPower.freshness_threshold());
    ss.grid_voltage.tick(at, SensorId::GridVoltage.freshness_threshold());
    ss.grid_current.tick(at, SensorId::GridCurrent.freshness_threshold());
    ss.consumption_current
        .tick(at, SensorId::ConsumptionCurrent.freshness_threshold());
    ss.offgrid_power
        .tick(at, SensorId::OffgridPower.freshness_threshold());
    ss.offgrid_current
        .tick(at, SensorId::OffgridCurrent.freshness_threshold());
    ss.vebus_input_current
        .tick(at, SensorId::VebusInputCurrent.freshness_threshold());
    ss.evcharger_ac_power
        .tick(at, SensorId::EvchargerAcPower.freshness_threshold());
    ss.evcharger_ac_current
        .tick(at, SensorId::EvchargerAcCurrent.freshness_threshold());
    ss.ess_state.tick(at, SensorId::EssState.freshness_threshold());
    ss.outdoor_temperature
        .tick(at, SensorId::OutdoorTemperature.freshness_threshold());
    ss.session_kwh
        .tick(at, SensorId::SessionKwh.freshness_threshold());
    // PR-ev-soc-sensor.
    ss.ev_soc.tick(at, SensorId::EvSoc.freshness_threshold());
    // PR-auto-extended-charge.
    ss.ev_charge_target
        .tick(at, SensorId::EvChargeTarget.freshness_threshold());
    // PR-ZD-1.
    ss.heat_pump_power
        .tick(at, SensorId::HeatPumpPower.freshness_threshold());
    ss.cooker_power
        .tick(at, SensorId::CookerPower.freshness_threshold());
    ss.mppt_0_operation_mode
        .tick(at, SensorId::Mppt0OperationMode.freshness_threshold());
    ss.mppt_1_operation_mode
        .tick(at, SensorId::Mppt1OperationMode.freshness_threshold());

    world.typed_sensors.zappi_state.tick(at, myenergi);
    world.typed_sensors.eddi_mode.tick(at, myenergi);

    // PR-AS-C: actuated readback freshness decays on the same threshold
    // table as the mirroring sensor — single source of truth via the
    // `SensorId` whose `actuated_id()` maps to the entity. Schedule
    // entities are mirrored by 5 leaf SensorIds each that all share the
    // same threshold; pick the `Start` leaf as the canonical
    // representative.
    //
    // Exception: the zappi/eddi mode readbacks come from the myenergi
    // poller (not D-Bus) and share a single freshness window with the
    // typed sensors on the same source — `params.freshness_myenergi`.
    // Routing both through the same constant prevents the two sources
    // of truth drifting apart.
    world
        .grid_setpoint
        .tick(at, SensorId::GridSetpointActual.freshness_threshold());
    world
        .input_current_limit
        .tick(at, SensorId::InputCurrentLimitActual.freshness_threshold());
    world.zappi_mode.tick(at, myenergi);
    world.eddi_mode.tick(at, myenergi);
    world
        .schedule_0
        .tick(at, SensorId::Schedule0StartActual.freshness_threshold());
    world
        .schedule_1
        .tick(at, SensorId::Schedule1StartActual.freshness_threshold());

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

    // PR-auto-extended-charge: per-tick check for the daily 04:30
    // boundary. The function short-circuits in `Forced`/`Disabled` mode
    // and is idempotent within the same local date once the latch flips.
    maybe_evaluate_auto_extended(world, clock);
}

// =============================================================================
// Controllers
// =============================================================================

/// Lazily-initialized production `CoreRegistry`. Construction is
/// infallible for the statically-defined production core list — the
/// validation checks inside `CoreRegistry::build` catch programmer
/// errors at process start, not at runtime per tick.
fn registry() -> &'static CoreRegistry {
    static REGISTRY: OnceLock<CoreRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        CoreRegistry::build(production_cores())
            .expect("production core DAG is statically valid")
    })
}

fn run_controllers(
    world: &mut World,
    clock: &dyn Clock,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    registry().run_all(world, clock, topology, effects);
}

// --- Setpoint -----------------------------------------------------------------

// PR-gamma-hold-redesign: per-knob "effective value" helpers. Each of
// the four weather_soc-driven outputs has a `*_mode` selector:
// `Weather` reads the planner's per-tick derivation
// (`bookkeeping.weather_soc_*`); `Forced` reads the user-owned knob
// directly. These helpers are the single source of truth — every
// controller that needs one of these four values dispatches through
// here so `*_mode` semantics can't drift across consumers.

#[must_use]
pub(crate) fn effective_export_soc_threshold(world: &World) -> f64 {
    use crate::knobs::Mode;
    match world.knobs.export_soc_threshold_mode {
        Mode::Forced => world.knobs.export_soc_threshold,
        Mode::Weather => world.bookkeeping.weather_soc_export_soc_threshold,
    }
}

#[must_use]
pub(crate) fn effective_discharge_soc_target(world: &World) -> f64 {
    use crate::knobs::Mode;
    match world.knobs.discharge_soc_target_mode {
        Mode::Forced => world.knobs.discharge_soc_target,
        Mode::Weather => world.bookkeeping.weather_soc_discharge_soc_target,
    }
}

#[must_use]
pub(crate) fn effective_battery_soc_target(world: &World) -> f64 {
    use crate::knobs::Mode;
    match world.knobs.battery_soc_target_mode {
        Mode::Forced => world.knobs.battery_soc_target,
        Mode::Weather => world.bookkeeping.weather_soc_battery_soc_target,
    }
}

#[must_use]
pub(crate) fn effective_disable_night_grid_discharge(world: &World) -> bool {
    use crate::knobs::Mode;
    match world.knobs.disable_night_grid_discharge_mode {
        Mode::Forced => world.knobs.disable_night_grid_discharge,
        Mode::Weather => world.bookkeeping.weather_soc_disable_night_grid_discharge,
    }
}

/// PR-auto-extended-charge: resolve the effective EV-extended-charge
/// flag controllers feed to their `charge_car_extended` global. Combines
/// the user-set tri-state mode knob with the auto-evaluation result
/// in `bookkeeping.auto_extended_today`:
///
///   * `Forced`   → always `true`.
///   * `Disabled` → always `false`.
///   * `Auto`     → consults bookkeeping (the 04:30 evaluator's verdict
///     for the current local date).
#[must_use]
pub fn effective_charge_car_extended(world: &World) -> bool {
    use crate::knobs::ExtendedChargeMode;
    match world.knobs.charge_car_extended_mode {
        ExtendedChargeMode::Forced => true,
        ExtendedChargeMode::Disabled => false,
        ExtendedChargeMode::Auto => world.bookkeeping.auto_extended_today,
    }
}

/// PR-ZD-4: world-level wrapper for the compensated-drain formula used by
/// both the soft loop (PR-ZD-3, in `evaluate_setpoint`) and the Fast-mode
/// hard clamp (PR-ZD-4, in `run_setpoint`).
///
/// Formula: `max(0, -battery_dc_power - heat_pump_w - cooker_w)`.
/// See `compute_compensated_drain` in `controllers::setpoint` for the
/// canonical definition.
///
/// Stale HP/cooker sensors return `None` from `.value`; treated as 0 W
/// (conservative — clamps tighter on a dead bridge, never looser).
/// `battery_dc_power` uses `.unwrap_or(0.0)` defensively; in
/// production this path runs after `build_setpoint_input` confirms
/// usability, so `unwrap()` would also be safe.
#[must_use]
pub(crate) fn compensated_drain_w(world: &World) -> f64 {
    let battery = world.sensors.battery_dc_power.value.unwrap_or(0.0);
    let hp = world.sensors.heat_pump_power.value.unwrap_or(0.0);
    let cooker = world.sensors.cooker_power.value.unwrap_or(0.0);
    compute_compensated_drain(battery, hp, cooker)
}

/// Returns `true` when at least one MPPT charger reports
/// `MppOperationMode == 1` (voltage/current limited — curtailed by
/// the inverter). This is the only "potential production available"
/// signal we have; when curtailed, the MPPT could produce more if
/// there were demand. Used by `evaluate_setpoint`'s relax branch to
/// gate a probe step deeper than observed `-solar_export`.
///
/// Stale or unknown sensor → `false` (conservative — don't probe
/// blindly when we can't confirm the curtailment state).
///
/// LOCKSTEP NOTE: this is one of two control-loop reads of the
/// MPPT op-mode sensors (the other is the dashboard surface in
/// `convert.rs`). The M-ZAPPI-DRAIN cross-cutting note "MPPT op-mode
/// is observability only" was reversed by M-ZAPPI-DRAIN-PROBE.
#[must_use]
pub(crate) fn mppt_curtailed(world: &World) -> bool {
    /// Victron `/MppOperationMode` enum: 0=Off, 1=V/I-limited, 2=MPPT-tracking.
    const VOLTAGE_OR_CURRENT_LIMITED: f64 = 1.0;

    fn is_curtailed(slot: &crate::tass::Actual<f64>) -> bool {
        slot.is_usable()
            && matches!(slot.value, Some(v) if (v - VOLTAGE_OR_CURRENT_LIMITED).abs() < 1e-6)
    }

    is_curtailed(&world.sensors.mppt_0_operation_mode)
        || is_curtailed(&world.sensors.mppt_1_operation_mode)
}

/// Classify which branch of the compensated-drain Zappi-active controller
/// fired this tick. Mirrors the `if/else if` ladder in
/// `evaluate_setpoint`'s Zappi branch. Pure observability — never feeds
/// back into the controller.
///
/// LOCKSTEP: must stay in sync with `evaluate_setpoint`'s branch ladder.
/// If a new branch lands in the controller, this function MUST be
/// updated in the same commit.
pub(crate) fn classify_zappi_drain_branch(world: &World) -> ZappiDrainBranch {
    if world.knobs.force_disable_export {
        return ZappiDrainBranch::Bypass;
    }
    if !world.derived.zappi_active {
        return ZappiDrainBranch::Disabled;
    }
    if world.knobs.allow_battery_to_car {
        return ZappiDrainBranch::Bypass;
    }
    let drain = compensated_drain_w(world);
    let threshold = f64::from(world.knobs.zappi_battery_drain_threshold_w);
    if drain > threshold {
        ZappiDrainBranch::Tighten
    } else {
        ZappiDrainBranch::Relax
    }
}

/// PR-auto-extended-charge: at the daily 04:30 boundary, when
/// `charge_car_extended_mode = Auto`, decide whether to enable extended
/// charge for the upcoming NightExtended (05:00–08:00) window. The
/// decision is persisted in `bookkeeping.auto_extended_today` and
/// consulted by `effective_charge_car_extended` until the next
/// evaluation overwrites it. The latch is per local date.
///
/// Conditions for enable: `ev_soc < 40` OR `ev_charge_target > 80`.
/// Stale/Unknown `ev_soc` → defensively disable (don't pull cheap-rate
/// grid power without knowing the car's state). Stale/Unknown
/// `ev_charge_target` is treated as "no signal", i.e. only the SoC
/// branch can fire.
///
/// Per-tick safe: idempotent within the same date once the latch flips.
pub(crate) fn maybe_evaluate_auto_extended(world: &mut World, clock: &dyn Clock) {
    use crate::knobs::ExtendedChargeMode;
    use crate::tass::Freshness;
    use chrono::Timelike;
    if !matches!(
        world.knobs.charge_car_extended_mode,
        ExtendedChargeMode::Auto
    ) {
        return;
    }
    let now = clock.naive();
    let today = now.date();
    if world.bookkeeping.auto_extended_today_date == Some(today) {
        return; // already evaluated today
    }
    // Pre-04:30 — wait until the 04:30 boundary fires.
    if now.hour() < 4 || (now.hour() == 4 && now.minute() < 30) {
        return;
    }
    let ev_soc = &world.sensors.ev_soc;
    let ev_target = &world.sensors.ev_charge_target;
    let auto_extended = match (ev_soc.freshness, ev_soc.value) {
        (Freshness::Fresh, Some(soc)) => {
            let target_says_yes = matches!(ev_target.freshness, Freshness::Fresh)
                && ev_target.value.is_some_and(|t| t > 80.0);
            soc < 40.0 || target_says_yes
        }
        // Stale / Unknown / Deprecated `ev_soc` → defensively disable.
        // Don't pull cheap-rate grid power without a current SoC reading.
        _ => false,
    };
    world.bookkeeping.auto_extended_today = auto_extended;
    world.bookkeeping.auto_extended_today_date = Some(today);
}

/// Build the `SetpointInput` from the current world. Returns `None`
/// when the required Fresh-sensor preconditions aren't met (the safety
/// path fires); otherwise returns the live input the controller would
/// run on. PR-core-io-popups: shared between `run_setpoint` and the
/// dashboard's `SetpointCore::last_inputs` so the popup shows exactly
/// the values the controller saw.
///
/// `idle_setpoint_w`: deploy-time idle setpoint from `topology.hardware`.
/// Used as the cold-boot fallback for `setpoint_target_prev` when no
/// prior setpoint has been commanded. Callers that do not have topology
/// in scope (e.g. `CoreId::Setpoint::last_inputs`) should pass
/// `HardwareParams::defaults().idle_setpoint_w as i32`.
#[must_use]
pub(crate) fn build_setpoint_input(world: &World, idle_setpoint_w: i32) -> Option<SetpointInput> {
    // PR-ZD-3: battery_dc_power added to the required-fresh set. A
    // momentary battery-service hiccup falls through to
    // `apply_setpoint_safety` (idle 10 W) — conservative posture
    // matches the safety-first design intent.
    if !world.sensors.battery_soc.is_usable()
        || !world.sensors.battery_soh.is_usable()
        || !world.sensors.battery_installed_capacity.is_usable()
        || !world.sensors.mppt_power_0.is_usable()
        || !world.sensors.mppt_power_1.is_usable()
        || !world.sensors.soltaro_power.is_usable()
        || !world.sensors.power_consumption.is_usable()
        || !world.sensors.evcharger_ac_power.is_usable()
        || !world.sensors.battery_dc_power.is_usable()
    {
        return None;
    }
    let k = &world.knobs;
    let bk = &world.bookkeeping;
    Some(SetpointInput {
        globals: SetpointInputGlobals {
            force_disable_export: k.force_disable_export,
            // PR-gamma-hold-redesign: dispatch on `*_mode`.
            export_soc_threshold: effective_export_soc_threshold(world),
            discharge_soc_target: effective_discharge_soc_target(world),
            full_charge_export_soc_threshold: k.full_charge_export_soc_threshold,
            full_charge_discharge_soc_target: k.full_charge_discharge_soc_target,
            // A-05 (PR-DAG-B): read `world.derived.zappi_active`, written
            // at the top of the tick by `ZappiActiveCore` (which runs
            // first per its `depends_on = []` root status). Never read
            // `bookkeeping` for this — that field was deleted.
            zappi_active: world.derived.zappi_active,
            allow_battery_to_car: k.allow_battery_to_car,
            discharge_time: k.discharge_time,
            debug_full_charge: k.debug_full_charge,
            pessimism_multiplier_modifier: k.pessimism_multiplier_modifier,
            next_full_charge: bk.next_full_charge,
            // PR-inverter-safe-discharge-knob.
            inverter_safe_discharge_enable: k.inverter_safe_discharge_enable,
            full_charge_defer_to_next_sunday: k.full_charge_defer_to_next_sunday,
            full_charge_snap_back_max_weekday: k.full_charge_snap_back_max_weekday,
            // PR-ZD-3: compensated-drain soft-loop knobs.
            zappi_drain_threshold_w: k.zappi_battery_drain_threshold_w,
            zappi_drain_relax_step_w: k.zappi_battery_drain_relax_step_w,
            zappi_drain_kp: k.zappi_battery_drain_kp,
            zappi_drain_target_w: k.zappi_battery_drain_target_w,
            // PR-ZDP-1: MPPT probe fields.
            zappi_drain_mppt_probe_w: k.zappi_battery_drain_mppt_probe_w,
            mppt_curtailed: mppt_curtailed(world),
            grid_export_limit_w: k.grid_export_limit_w,
        },
        power_consumption: world.sensors.power_consumption.value.unwrap(),
        battery_soc: world.sensors.battery_soc.value.unwrap(),
        soh: world.sensors.battery_soh.value.unwrap(),
        mppt_power_0: world.sensors.mppt_power_0.value.unwrap(),
        mppt_power_1: world.sensors.mppt_power_1.value.unwrap(),
        soltaro_power: world.sensors.soltaro_power.value.unwrap(),
        evcharger_ac_power: world.sensors.evcharger_ac_power.value.unwrap(),
        capacity: world.sensors.battery_installed_capacity.value.unwrap(),
        // PR-ZD-3: required-fresh (guarded above).
        battery_dc_power: world.sensors.battery_dc_power.value.unwrap(),
        // PR-ZD-3: stale HP/cooker → 0.0 (tighter clamp; see plan §4).
        heat_pump_power: world.sensors.heat_pump_power.value.unwrap_or(0.0),
        cooker_power: world.sensors.cooker_power.value.unwrap_or(0.0),
        // PR-ZD-3: recurrence base for the soft loop.
        // Falls back to idle_setpoint_w on cold boot (no prior setpoint commanded).
        setpoint_target_prev: world.grid_setpoint.target.value.unwrap_or(idle_setpoint_w),
    })
}

pub(crate) fn run_setpoint(
    world: &mut World,
    clock: &dyn Clock,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    // Required Fresh sensors. A-17: evcharger_ac_power joins the
    // required set — the Hoymiles export term in solar_export depends
    // on the EV-branch CT reading.
    let Some(input) = build_setpoint_input(world, topology.hardware.idle_setpoint_w as i32) else {
        apply_setpoint_safety(world, clock, topology, effects);
        return;
    };

    let k = &world.knobs;

    let out = evaluate_setpoint(&input, clock, &topology.hardware);

    // PR-ZD-4: Fast-mode hard clamp — independent safety net on top of the
    // soft loop. Only fires in Fast mode because Eco/Eco+ self-modulate via
    // Zappi's CT clamp; in those modes the soft loop is sufficient. Fast mode
    // pulls regardless of CT.
    //
    // Reads `world.zappi_mode.target.value` (the commanded mode, not the
    // readback) — predictive arming: the moment the controller commits to Fast,
    // the clamp arms without waiting for the next myenergi poll.
    let zappi_fast = matches!(
        world.zappi_mode.target.value,
        Some(ZappiMode::Fast)
    );
    let hard_clamp_drain_w = compensated_drain_w(world);
    let hard_clamp_w = f64::from(world.knobs.zappi_battery_drain_hard_clamp_w);

    let (hard_clamped_target, hard_clamp_engaged, hard_clamp_excess) =
        if zappi_fast
            && !world.knobs.allow_battery_to_car
            && world.derived.zappi_active
            && hard_clamp_drain_w > hard_clamp_w
        {
            let excess = hard_clamp_drain_w - hard_clamp_w;
            let raised = (f64::from(out.setpoint_target) + excess).round() as i32;
            (raised, true, excess)
        } else {
            (out.setpoint_target, false, 0.0)
        };

    // PR-ZDO-1: Compensated-drain observability capture. Pure read of
    // what the controller just decided; no feedback into any branch.
    // LOCKSTEP: classify_zappi_drain_branch must mirror evaluate_setpoint's
    // Zappi-branch ladder.
    {
        let drain_w = compensated_drain_w(world);
        let branch = classify_zappi_drain_branch(world);
        let snap = ZappiDrainSnapshot {
            compensated_drain_w: drain_w,
            branch,
            hard_clamp_engaged,
            hard_clamp_excess_w: hard_clamp_excess,
            threshold_w: i32::try_from(world.knobs.zappi_battery_drain_threshold_w)
                .unwrap_or(i32::MAX),
            hard_clamp_w: i32::try_from(world.knobs.zappi_battery_drain_hard_clamp_w)
                .unwrap_or(i32::MAX),
            captured_at_ms: clock.wall_clock_epoch_ms(),
        };
        world.zappi_drain_state.push(snap);
    }

    // SPEC §5.11: grid-side hard cap — two-sided clamp.
    // PR-hardware-config: split the former single SAFE_MAX_GRID_LIMIT_W
    // = 10_000 into two per-direction ceilings sourced from
    // `topology.hardware`: export defaults to 6000 W (ESB G99 typical
    // authorisation), import defaults to 13_000 W (MultiPlus continuous
    // import capability). The clamp caps the user knob irrespective of
    // what the MQTT/dashboard ingest validators accept. A-09: without
    // these clamps, a grid_*_limit_w above i32::MAX would pass
    // `i32::try_from` → fall to `unwrap_or(i32::MAX)` and yield
    // effectively unbounded export (since we then unary-minus it).
    // `.min(...).try_into()` is guaranteed to succeed because both
    // ceilings fit in i32.
    let export_cap = i32::try_from(
        k.grid_export_limit_w
            .min(topology.hardware.grid_export_knob_max_w),
    )
    .expect("grid_export_knob_max_w fits in i32");
    let import_cap = i32::try_from(
        k.grid_import_limit_w
            .min(topology.hardware.grid_import_knob_max_w),
    )
    .expect("grid_import_knob_max_w fits in i32");
    // PR-ZD-4: feed `hard_clamped_target` (post-hard-clamp) into the grid-cap
    // clamp. The grid_import_limit_w / grid_export_limit_w are the final
    // ceiling; the hard clamp adds to the soft-loop output, then grid-cap clips.
    let pre_clamp = hard_clamped_target;
    let clamped = pre_clamp.clamp(-export_cap, import_cap);
    // A-10: re-assert the idle-bleed invariant AFTER the clamp. With
    // grid_export_limit_w = 0 the clamp bounds become [-0, +import_cap],
    // so any negative setpoint is pinned to 0 — which some Victron
    // firmware treats distinctly from 10 W ("idle"). If the post-clamp
    // value is >= 0 but the pre-clamp was a real controller decision,
    // the clamp collapsed it to zero — promote to 10 W so vebus sees
    // the explicit idle command instead of a raw 0.
    let capped = if pre_clamp < 0 && clamped == 0 {
        10
    } else {
        clamped
    };

    // PR-09a-D02: only add the clamp factors when the clamp actually
    // altered the value. Previously 3 factors were emitted every tick
    // even in the common `pre_clamp == capped` case, producing 3
    // noise rows per tick on the decision panel.
    //
    // Factor names distinguish the runtime-wrapper clamp
    // (grid_cap_*) from the core-setpoint's internal
    // pre_clamp_setpoint_W factor — they operate at different layers:
    // the core clamps at max_discharge semantics; the wrapper below
    // clamps at the user-configurable grid-side export/import caps.
    //
    // PR-ZD-4: hard-clamp factors are prepended (only when engaged) so
    // they appear near the top of the decision panel alongside the
    // soft-loop factors that precede the grid-cap factors.
    let base_decision = if hard_clamp_engaged {
        out.decision
            .clone()
            .with_factor("hard_clamp_engaged", "true".to_string())
            .with_factor(
                "hard_clamp_excess_W",
                format!("{hard_clamp_excess:.0}"),
            )
            .with_factor(
                "hard_clamp_threshold_W",
                format!("{hard_clamp_w:.0}"),
            )
            .with_factor(
                "hard_clamp_pre_W",
                format!("{}", out.setpoint_target),
            )
            .with_factor(
                "hard_clamp_post_W",
                format!("{hard_clamped_target}"),
            )
    } else {
        out.decision.clone()
    };
    let decision = if pre_clamp == capped {
        base_decision
    } else {
        base_decision
            .with_factor("grid_cap_pre_W", format!("{pre_clamp}"))
            .with_factor(
                "grid_cap_bounds_W",
                format!("[-{export_cap}, +{import_cap}]"),
            )
            .with_factor("grid_cap_post_W", format!("{capped}"))
    };
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
    // PR-09a-D01: always populate the grid_setpoint Decision even on
    // the safety path. The dashboard otherwise shows "None" for
    // grid_setpoint Decision until a Fresh tick arrives — operator
    // doesn't know whether the controller is running-without-data or
    // silent. Honest decision + factors name the missing sensors.
    let missing = missing_required_setpoint_sensors(world);
    world.decisions.grid_setpoint = Some(
        Decision::new("Safety fallback: required sensors not usable → idle 10 W")
            .with_factor("setpoint_W", "10".to_string())
            .with_factor("owner", "System".to_string())
            .with_factor("missing_sensors", missing),
    );
    // Safety target: 10 W, owned by System.
    maybe_propose_setpoint(
        world,
        10,
        Owner::System,
        clock.monotonic(),
        topology.controller_params,
        effects,
    );

    // PR-ZDO-1: Honest observability when the controller couldn't run.
    // branch = Disabled tells the chart "no signal here", instead of
    // freezing at the previous value.
    let snap = ZappiDrainSnapshot {
        compensated_drain_w: 0.0,
        branch: ZappiDrainBranch::Disabled,
        hard_clamp_engaged: false,
        hard_clamp_excess_w: 0.0,
        threshold_w: i32::try_from(world.knobs.zappi_battery_drain_threshold_w)
            .unwrap_or(i32::MAX),
        hard_clamp_w: i32::try_from(world.knobs.zappi_battery_drain_hard_clamp_w)
            .unwrap_or(i32::MAX),
        captured_at_ms: clock.wall_clock_epoch_ms(),
    };
    world.zappi_drain_state.push(snap);
}

/// Build a comma-separated list of the required setpoint sensors that
/// are not `is_usable()`. Used by `apply_setpoint_safety` to populate
/// the Decision so the operator can see exactly what's missing.
fn missing_required_setpoint_sensors(world: &World) -> String {
    let mut missing: Vec<&'static str> = Vec::new();
    if !world.sensors.battery_soc.is_usable() {
        missing.push("battery_soc");
    }
    if !world.sensors.battery_soh.is_usable() {
        missing.push("battery_soh");
    }
    if !world.sensors.battery_installed_capacity.is_usable() {
        missing.push("battery_installed_capacity");
    }
    if !world.sensors.mppt_power_0.is_usable() {
        missing.push("mppt_power_0");
    }
    if !world.sensors.mppt_power_1.is_usable() {
        missing.push("mppt_power_1");
    }
    if !world.sensors.soltaro_power.is_usable() {
        missing.push("soltaro_power");
    }
    if !world.sensors.power_consumption.is_usable() {
        missing.push("power_consumption");
    }
    if !world.sensors.evcharger_ac_power.is_usable() {
        missing.push("evcharger_ac_power");
    }
    // PR-ZD-3: battery_dc_power is required for the compensated-drain loop.
    if !world.sensors.battery_dc_power.is_usable() {
        missing.push("battery_dc_power");
    }
    if missing.is_empty() {
        "<none — safety fallback fired despite all sensors usable; bug>".to_string()
    } else {
        missing.join(", ")
    }
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
    // A-31: promote to i64 before subtracting. Even with PR-09b's
    // SAFE_MAX_GRID_LIMIT_W clamp the setpoint values come from
    // `evaluate_setpoint` and could theoretically reach i32::MIN/MAX
    // via a controller bug; `i32::MIN - i32::MAX` panics in debug and
    // wraps in release. `i64::from(...) - i64::from(...)` cannot
    // overflow for any i32 inputs.
    // PR-ACT-RETRY-1 D01: gate on phase=Confirmed. If phase is
    // Pending/Commanded with mismatching actual, the deadband must NOT
    // pre-empt — `needs_actuation` below has to run for the retry path.
    if let Some(current_target) = world.grid_setpoint.target.value {
        let delta = (i64::from(current_target) - i64::from(value)).abs();
        if delta < i64::from(params.setpoint_retarget_deadband_w)
            && world.grid_setpoint.target.phase == crate::tass::TargetPhase::Confirmed
        {
            return;
        }
    }

    // Propose target unconditionally (PR-SCHED0): target mutation must
    // happen even in observer mode so the dashboard can show the
    // controller's intent. The `writes_enabled` gate moves below, so
    // effect emission (WriteDbus / mark_commanded / ActuatedPhase)
    // stays suppressed. The `Command::KillSwitch(false→true)` edge
    // still resets every target to Unset, which prevents the A-06
    // stuck-Pending hazard when this path runs in observer mode.
    let changed = world.grid_setpoint.propose_target(value, owner, now);
    // PR-ACT-RETRY-1: re-fire the write when actual still doesn't
    // match target after `actuator_retry_s`. `confirm_if` upgrades to
    // Confirmed when actual is within deadband, so phase ∈ {Pending,
    // Commanded} past the threshold means "retry needed".
    let retry_threshold = Duration::from_secs(u64::from(world.knobs.actuator_retry_s));
    if !changed && !world.grid_setpoint.needs_actuation(now, retry_threshold) {
        return;
    }

    // PR-SCHED0-D03: publish phase unconditionally. ActuatedPhase is a
    // state-reporting effect (dashboard retained MQTT), not an
    // actuation effect. Without this publish the dashboard's retained
    // phase would go stale across live→observer transitions that move
    // target Commanded→Pending with no WriteDbus.
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::GridSetpoint,
        phase: world.grid_setpoint.target.phase,
    }));

    if !world.knobs.writes_enabled {
        // PR-ACT-RETRY-1 D04: gate the observer-mode log on `changed`
        // so the retry path (changed=false but needs_actuation=true)
        // doesn't spam the log every tick past the threshold. The
        // ActuatedPhase publish above already surfaces the
        // stuck-Pending state to the dashboard.
        if changed {
            effects.push(Effect::Log {
                level: LogLevel::Info,
                source: "observer",
                message: format!(
                    "GridSetpoint would be set to {value} W (owner={owner:?}); suppressed by writes_enabled=false"
                ),
            });
        }
        return;
    }

    // Probed live-Venus type for `/Settings/CGwacs/AcPowerSetPoint` is `double`
    // (scripts/probe-schedule-types.sh, 2026-04-25). Sending `Int` would be a
    // ticking time bomb the moment writes_enabled flips on — Venus replies
    // "Wrong type" to every tick and the setpoint never lands. Closes A-29's
    // setpoint-side aspect.
    effects.push(Effect::WriteDbus {
        target: DbusTarget::GridSetpoint,
        value: DbusValue::Float(f64::from(value)),
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

/// Build the `CurrentLimitInput` from the current world, or `None` if
/// the controller's required Fresh-sensor gates aren't satisfied.
/// PR-core-io-popups: shared between `run_current_limit` and the
/// dashboard's `CurrentLimitCore::last_inputs`.
#[must_use]
pub(crate) fn build_current_limit_input(world: &World) -> Option<CurrentLimitInput> {
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
        return None;
    }
    if !world.typed_sensors.zappi_state.is_usable() {
        return None;
    }
    let k = &world.knobs;
    let bk = &world.bookkeeping;
    Some(CurrentLimitInput {
        globals: CurrentLimitInputGlobals {
            zappi_current_target: k.zappi_current_target,
            zappi_emergency_margin: k.zappi_emergency_margin,
            zappi_state: world.typed_sensors.zappi_state.value.unwrap(),
            // PR-DAG-B: read `world.derived.zappi_active` (written by
            // `ZappiActiveCore` at the top of the tick) so setpoint and
            // current-limit see the same value within a tick.
            zappi_active: world.derived.zappi_active,
            // PR-auto-extended-charge: dispatch the EV-side flag through
            // the tri-state effective helper so the current-limit
            // controller sees the same value as schedules / zappi_mode.
            extended_charge_required: effective_charge_car_extended(world)
                || world.bookkeeping.charge_to_full_required,
            // PR-gamma-hold-redesign: dispatch on `*_mode`.
            disable_night_grid_discharge: effective_disable_night_grid_discharge(world),
            battery_soc_target: bk.battery_selected_soc_target,
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
    })
}

pub(crate) fn run_current_limit(
    world: &mut World,
    clock: &dyn Clock,
    topology: &Topology,
    effects: &mut Vec<Effect>,
) {
    let Some(input) = build_current_limit_input(world) else {
        return;
    };

    let out = evaluate_current_limit(&input, clock, &topology.hardware);
    world.decisions.input_current_limit = Some(out.decision.clone());

    // Propose target.
    let value = out.input_current_limit;
    let now = clock.monotonic();
    let params = topology.controller_params;

    // PR-ACT-RETRY-1 D01: see `maybe_propose_setpoint` — gate the
    // dead-band early-return on phase=Confirmed so the retry path runs
    // when phase is Pending/Commanded with mismatching actual.
    if let Some(current_target) = world.input_current_limit.target.value {
        if (current_target - value).abs() < params.current_limit_retarget_deadband_a
            && world.input_current_limit.target.phase == crate::tass::TargetPhase::Confirmed
        {
            return;
        }
    }

    // Propose target unconditionally (PR-SCHED0): see
    // `maybe_propose_setpoint`. The KillSwitch false→true edge still
    // resets every target.
    let changed = world
        .input_current_limit
        .propose_target(value, Owner::CurrentLimitController, now);
    // PR-ACT-RETRY-1.
    let retry_threshold = Duration::from_secs(u64::from(world.knobs.actuator_retry_s));
    if !changed && !world.input_current_limit.needs_actuation(now, retry_threshold) {
        return;
    }

    // PR-SCHED0-D03: publish phase unconditionally; see `maybe_propose_setpoint`.
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::InputCurrentLimit,
        phase: world.input_current_limit.target.phase,
    }));

    if !world.knobs.writes_enabled {
        // PR-ACT-RETRY-1 D04: see `maybe_propose_setpoint`.
        if changed {
            effects.push(Effect::Log {
                level: LogLevel::Info,
                source: "observer",
                message: format!(
                    "InputCurrentLimit would be set to {value:.2} A; suppressed by writes_enabled=false"
                ),
            });
        }
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

/// Effective `charge_battery_extended` flag fed to the schedules
/// controller, decomposed for `run_schedules` (which decorates the
/// Decision with the derivation factors) and `SchedulesCore::last_inputs`
/// (which surfaces them in the popup).
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct CbeDerivation {
    pub from_full: bool,
    pub from_weather: bool,
    pub derived: bool,
    pub effective: bool,
}

pub(crate) fn cbe_derivation(world: &World) -> CbeDerivation {
    let bk = &world.bookkeeping;
    let k = &world.knobs;
    let from_full = bk.charge_to_full_required;
    let from_weather = bk.charge_battery_extended_today;
    let derived = from_full || from_weather;
    let effective = match k.charge_battery_extended_mode {
        crate::knobs::ChargeBatteryExtendedMode::Auto => derived,
        crate::knobs::ChargeBatteryExtendedMode::Forced => true,
        crate::knobs::ChargeBatteryExtendedMode::Disabled => false,
    };
    CbeDerivation { from_full, from_weather, derived, effective }
}

/// Build the `SchedulesInput` from the current world, or `None` when
/// the controller's `battery_soc` precondition isn't satisfied.
/// PR-core-io-popups.
#[must_use]
pub(crate) fn build_schedules_input(world: &World) -> Option<SchedulesInput> {
    if !world.sensors.battery_soc.is_usable() {
        return None;
    }
    let bk = &world.bookkeeping;
    let cbe = cbe_derivation(world);
    Some(SchedulesInput {
        globals: SchedulesInputGlobals {
            charge_battery_extended: cbe.effective,
            // PR-auto-extended-charge.
            charge_car_extended: effective_charge_car_extended(world),
            charge_to_full_required: bk.charge_to_full_required,
            // PR-gamma-hold-redesign: dispatch on `*_mode`.
            disable_night_grid_discharge: effective_disable_night_grid_discharge(world),
            zappi_active: world.derived.zappi_active,
            above_soc_date: bk.above_soc_date,
            battery_soc_target: effective_battery_soc_target(world),
        },
        battery_soc: world.sensors.battery_soc.value.unwrap(),
    })
}

pub(crate) fn run_schedules(world: &mut World, clock: &dyn Clock, effects: &mut Vec<Effect>) {
    // Schedules always runs — battery_soc is the only required sensor.
    let Some(input) = build_schedules_input(world) else {
        return;
    };

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
    // PR-core-io-popups: the cbe derivation moved into `cbe_derivation()`
    // so `SchedulesCore::last_inputs` can surface the same factors.
    let cbe = cbe_derivation(world);
    let cbe_from_full = cbe.from_full;
    let cbe_from_weather = cbe.from_weather;
    let cbe_derived = cbe.derived;
    let charge_battery_extended = cbe.effective;

    let out = evaluate_schedules(&input, clock);
    // Schedule 0 Decision: the unconditional-boost invariant. No cbe
    // factors — they don't gate Schedule 0.
    world.decisions.schedule_0 = Some(out.schedule_0_decision.clone());
    // Schedule 1 Decision: branch-specific; decorate with the cbe
    // derivation factors the caller knows about (Auto/Forced/Disabled
    // mode + the underlying bookkeeping).
    let s1_decision = out
        .schedule_1_decision
        .clone()
        .with_factor(
            "cbe derivation",
            format!(
                "charge_to_full_required={cbe_from_full} || charge_battery_extended_today={cbe_from_weather} = {cbe_derived}"
            ),
        )
        .with_factor(
            "cbe mode override",
            format!(
                "{:?} → {charge_battery_extended}",
                world.knobs.charge_battery_extended_mode
            ),
        );
    world.decisions.schedule_1 = Some(s1_decision);

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
    // Capture before the mutable borrow below — `actuated` reborrows
    // through `&mut world.schedule_N` so `world.knobs` would conflict.
    let writes_enabled = world.knobs.writes_enabled;
    let retry_threshold = Duration::from_secs(u64::from(world.knobs.actuator_retry_s));
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

    // Propose target unconditionally (PR-SCHED0): the dashboard reads
    // `schedule_N.target.value` to display the controller's intent, so
    // this must run even in observer mode. Effects below stay gated on
    // `writes_enabled`. The `Command::KillSwitch(true)` edge-trigger
    // still resets every target on observer→live.
    let changed = actuated.propose_target(spec, Owner::ScheduleController, now);
    // PR-ACT-RETRY-1.
    if !changed && !actuated.needs_actuation(now, retry_threshold) {
        return;
    }

    // PR-SCHED0-D03: publish phase unconditionally; see `maybe_propose_setpoint`.
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id,
        phase: actuated.target.phase,
    }));

    if !writes_enabled {
        // PR-ACT-RETRY-1 D04: see `maybe_propose_setpoint`.
        if changed {
            effects.push(Effect::Log {
                level: LogLevel::Info,
                source: "observer",
                message: format!(
                    "Schedule{index} would be set to {spec:?}; suppressed by writes_enabled=false"
                ),
            });
        }
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
    // Probed live-Venus type for Schedule/Charge/{i}/Soc is `int32` despite the
    // SoC value being a percentage (scripts/probe-schedule-types.sh, 2026-04-25).
    // ScheduleSpec.soc is f64 to flow naturally through the planning math; cast
    // to i32 at the wire boundary. Closes A-29 / A-65.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let soc_i32 = spec.soc.round().clamp(0.0, 100.0) as i32;
    effects.push(Effect::WriteDbus {
        target: DbusTarget::Schedule {
            index,
            field: ScheduleField::Soc,
        },
        value: DbusValue::Int(soc_i32),
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

/// Build the `ZappiModeInput` from the current world, or `None` if the
/// typed Zappi state isn't usable. PR-core-io-popups.
#[must_use]
pub(crate) fn build_zappi_mode_input(world: &World) -> Option<ZappiModeInput> {
    if !world.typed_sensors.zappi_state.is_usable() {
        return None;
    }
    let zappi_state = world.typed_sensors.zappi_state.value.unwrap();
    let k = &world.knobs;
    Some(ZappiModeInput {
        globals: ZappiModeInputGlobals {
            charge_car_boost: k.charge_car_boost,
            // PR-auto-extended-charge: dispatch through the tri-state
            // helper so the NightExtended Fast/Off arm follows the
            // user's mode (or the auto-evaluation in `Auto`).
            charge_car_extended: effective_charge_car_extended(world),
            zappi_limit_kwh: k.zappi_limit,
        },
        current_mode: zappi_state.zappi_mode,
        // A-13 + A-14: session kWh flows from myenergi `che`. Compared
        // kWh-to-kWh against `zappi_limit`.
        session_kwh: zappi_state.session_kwh,
    })
}

pub(crate) fn run_zappi_mode(world: &mut World, clock: &dyn Clock, effects: &mut Vec<Effect>) {
    let Some(input) = build_zappi_mode_input(world) else {
        return;
    };

    let out = evaluate_zappi_mode(&input, clock);
    world.decisions.zappi_mode = Some(out.decision);
    let desired = match out.action {
        ZappiModeAction::Leave => return,
        ZappiModeAction::Set(m) => m,
    };

    let now = clock.monotonic();

    // Propose target unconditionally (PR-SCHED0): see
    // `maybe_propose_setpoint`.
    let changed = world
        .zappi_mode
        .propose_target(desired, Owner::ZappiController, now);
    // PR-ACT-RETRY-1.
    let retry_threshold = Duration::from_secs(u64::from(world.knobs.actuator_retry_s));
    if !changed && !world.zappi_mode.needs_actuation(now, retry_threshold) {
        return;
    }

    // PR-SCHED0-D03: publish phase unconditionally; see `maybe_propose_setpoint`.
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::ZappiMode,
        phase: world.zappi_mode.target.phase,
    }));

    if !world.knobs.writes_enabled {
        // PR-ACT-RETRY-1 D04: see `maybe_propose_setpoint`.
        if changed {
            effects.push(Effect::Log {
                level: LogLevel::Info,
                source: "observer",
                message: format!(
                    "ZappiMode would be set to {desired:?}; suppressed by writes_enabled=false"
                ),
            });
        }
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

/// Build the `EddiModeInput` from the current world. Always succeeds —
/// `evaluate_eddi_mode` itself handles a Stale/Unknown SoC. PR-core-io-popups.
#[must_use]
pub(crate) fn build_eddi_mode_input(world: &World) -> EddiModeInput {
    let soc = &world.sensors.battery_soc;
    let current_mode = world
        .typed_sensors
        .eddi_mode
        .value
        .unwrap_or(EddiMode::Stopped);
    let k = &world.knobs;
    EddiModeInput {
        soc_value: soc.value,
        soc_freshness: soc.freshness,
        current_mode,
        last_transition_at: world.bookkeeping.eddi_last_transition_at,
        knobs: EddiModeKnobs {
            enable_soc: k.eddi_enable_soc,
            disable_soc: k.eddi_disable_soc,
            dwell_s: k.eddi_dwell_s,
        },
    }
}

pub(crate) fn run_eddi_mode(world: &mut World, clock: &dyn Clock, effects: &mut Vec<Effect>) {
    let input = build_eddi_mode_input(world);

    let out = evaluate_eddi_mode(&input, clock);
    world.decisions.eddi_mode = Some(out.decision);

    // EDDI-ALWAYS-ACTUATE (user-flagged 2026-04-25): the eddi controller
    // always has a definite opinion on what mode the device should be in
    // (Stopped or Normal, per SoC + dwell + freshness gates). Propose
    // that target unconditionally so the dashboard / HA reflect the
    // controller's intent — pre-fix, the `Leave` arm short-circuited
    // before `propose_target`, leaving `world.eddi_mode.target` stuck at
    // `Unset` whenever the assumed-Stopped first-tick happened to match
    // the controller's Stopped decision (most boots).
    let desired = out.action.target();
    let now = clock.monotonic();
    let changed = world
        .eddi_mode
        .propose_target(desired, Owner::EddiController, now);

    // Always publish the post-propose phase so observers see the target
    // being set (Unset → Pending) on the first interesting tick.
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::EddiMode,
        phase: world.eddi_mode.target.phase,
    }));

    // Actuation gate: only fire `CallMyenergi` when the controller asked
    // for `Set` AND (the target value actually changed OR the universal
    // retry threshold has elapsed since the last propose / mark — see
    // PR-ACT-RETRY-1). `should_actuate=Leave` short-circuits regardless;
    // the retry path requires `Set` (we don't promote a `Leave` decision
    // into a write just because actual disagrees).
    let retry_threshold = Duration::from_secs(u64::from(world.knobs.actuator_retry_s));
    if !out.action.should_actuate() {
        return;
    }
    if !changed && !world.eddi_mode.needs_actuation(now, retry_threshold) {
        return;
    }

    // A-36: record the transition BEFORE the writes_enabled gate. The
    // dwell clock tracks "time since last proposed mode transition" —
    // it's TASS intent state, not actuation. Gating it behind
    // writes_enabled means observer mode perpetually reports "first
    // transition (no dwell)" and every Decision factor the operator is
    // verifying during the shadow run is a lie.
    world.bookkeeping.eddi_last_transition_at = Some(now);

    if !world.knobs.writes_enabled {
        // PR-ACT-RETRY-1 D04: see `maybe_propose_setpoint`.
        if changed {
            effects.push(Effect::Log {
                level: LogLevel::Info,
                source: "observer",
                message: format!(
                    "EddiMode would be set to {desired:?}; suppressed by writes_enabled=false"
                ),
            });
        }
        return;
    }

    effects.push(Effect::CallMyenergi(MyenergiAction::SetEddiMode(desired)));
    world.eddi_mode.mark_commanded(now);
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::EddiMode,
        phase: world.eddi_mode.target.phase,
    }));
}

// --- Weather-SoC --------------------------------------------------------------

/// PR-weather-soc-dynamic: weather_soc evaluates EVERY tick, not just
/// at the 01:55 cron moment. As forecasts refresh through the day
/// (Solcast 5 min, Open-Meteo 30 min, Forecast.Solar 30 min) the
/// planner re-derives its four outputs.
///
/// PR-gamma-hold-redesign: the planner no longer writes user-owned
/// knobs. Instead it writes its derivations to four bookkeeping slots
/// (`weather_soc_*`); the `*_mode = Weather` default routes the
/// setpoint / current-limit / schedules controllers to read those
/// slots. The user picks `Forced` per knob from the dashboard / HA to
/// pin a manually-set knob value through. There is no γ-hold, no
/// owner-priority queue, no `propose_knob`.
///
/// The Decision write (`world.decisions.weather_soc`) stays as before
/// — Honesty invariant: every controller emits a Decision every tick.
pub(crate) fn run_weather_soc(
    world: &mut World,
    clock: &dyn Clock,
    topology: &Topology,
    // PR-gamma-hold-redesign: weather_soc no longer pushes effects
    // (knob writes are gone — bookkeeping is mutated in place). The
    // parameter stays so the registry signature is uniform across
    // controllers.
    _effects: &mut Vec<Effect>,
) {
    let now = clock.naive();
    let today = now.date();

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

    // Fuse forecasts across providers, excluding any snapshot older
    // than `ControllerParams::freshness_forecast` (A-16: previously
    // treated all snapshots as fresh, so a week-old Solcast fetch +
    // API-key expiry would still drive tomorrow's planning). The
    // snapshot's `fetched_at: Instant` is stamped by the shell-side
    // fetcher on every successful fetch, so staleness survives the
    // shell layer's "don't republish stale" contract.
    let strategy = world.knobs.forecast_disagreement_strategy;
    let now_mono = clock.monotonic();
    let freshness_threshold = topology.controller_params.freshness_forecast;
    let is_fresh = |_provider: ForecastProvider, snap: &crate::world::ForecastSnapshot| {
        now_mono.saturating_duration_since(snap.fetched_at) <= freshness_threshold
    };
    let Some(today_kwh) = crate::controllers::forecast_fusion::fused_today_kwh(
        &world.typed_sensors,
        strategy,
        is_fresh,
    ) else {
        world.decisions.weather_soc = Some(
            Decision::new(
                "Skipped: no fresh fused forecast available (all providers stale or missing)"
                    .to_string(),
            )
            .with_factor("strategy", format!("{strategy:?}"))
            .with_factor(
                "freshness_forecast_s",
                format!("{}", freshness_threshold.as_secs()),
            ),
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
    let d = evaluate_weather_soc(
        &input,
        &k.weather_soc_table,
        k.weathersoc_very_sunny_threshold,
        clock,
    );
    world.decisions.weather_soc = Some(d.decision.clone());

    // PR-gamma-hold-redesign: write the planner's per-tick derivations
    // into the four bookkeeping slots. The `*_mode = Weather` default
    // routes the setpoint / current-limit / schedules controllers to
    // read these values. There is no γ-hold and no owner priority —
    // the user pins a manual override by flipping `*_mode` to `Forced`
    // from the dashboard / HA. These slots are pure observability;
    // they're not retained on MQTT (the per-tick recompute is cheap
    // and forecast-driven, so a reboot just rebuilds them).
    world.bookkeeping.weather_soc_export_soc_threshold = d.export_soc_threshold;
    world.bookkeeping.weather_soc_discharge_soc_target = d.discharge_soc_target;
    world.bookkeeping.weather_soc_battery_soc_target = d.battery_soc_target;
    world.bookkeeping.weather_soc_disable_night_grid_discharge = d.disable_night_grid_discharge;

    // A-15: record today's weather_soc decision on a dedicated per-day
    // field. `apply_tick` clears this on calendar-day rollover, so
    // schedules sees a fresh decision each day instead of a sticky OR
    // latch on `charge_to_full_required`.
    world.bookkeeping.charge_battery_extended_today = d.charge_battery_extended;
    world.bookkeeping.charge_battery_extended_today_date = Some(today);
    // A-21: mark today as handled. Now that the planner doesn't fire
    // knob proposals, the once-per-day guard is informational only —
    // kept so the dashboard / tests can observe "did the planner run
    // today yet". Stamped on every successful run.
    world.bookkeeping.last_weather_soc_run_date = Some(today);
}

// PR-gamma-hold-redesign: `propose_knob` is gone. The weather_soc
// planner writes bookkeeping directly; no other caller used it.

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

        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Off,
                zappi_plug_state: ZappiPlugState::EvDisconnected,
                zappi_status: ZappiStatus::Paused,
                zappi_last_change_signature: at,
                session_kwh: 0.0,
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
        // A Float WriteDbus to GridSetpoint was emitted (Venus path is `double`
        // — see scripts/probe-schedule-types.sh for the live-firmware check).
        let wd = effects.iter().find_map(|e| match e {
            Effect::WriteDbus { target: DbusTarget::GridSetpoint, value: DbusValue::Float(v) } => Some(*v),
            _ => None,
        });
        assert!(wd.is_some());
    }

    #[test]
    fn setpoint_freezes_at_10w_when_battery_soc_stale() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // Age battery_soc past the 120 s freshness threshold.
        let later = FixedClock::new(c.monotonic + StdDuration::from_secs(130), naive(12, 0));
        let _ = process(&Event::Tick { at: later.monotonic }, &mut world, &later, &Topology::defaults());

        assert_eq!(world.sensors.battery_soc.freshness, Freshness::Stale);
        assert_eq!(world.grid_setpoint.target.value, Some(10));
        assert_eq!(world.grid_setpoint.target.owner, Owner::System);
    }

    /// Test 25 (PR-ZD-3): stale `battery_dc_power` triggers the safety
    /// fallback — `build_setpoint_input` returns `None`, `apply_setpoint_safety`
    /// posts idle 10 W, owner=System. HP and cooker stale is acceptable
    /// (treated as 0 W); battery_dc_power is required-fresh.
    #[test]
    fn setpoint_safety_fallback_fires_when_battery_dc_power_stale() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // Age battery_dc_power past its freshness threshold (120 s for
        // SensorId::BatteryDcPower, matching the D-Bus sensor cadence).
        // The sensor was seeded with `on_reading(0.0, at)` in
        // `seed_required_sensors`; advancing 130 s past seed makes it Stale.
        let later = FixedClock::new(c.monotonic + StdDuration::from_secs(130), naive(12, 2));
        // Re-feed all other required sensors at `later` so only
        // battery_dc_power ages out.
        {
            let ss = &mut world.sensors;
            ss.battery_soc.on_reading(75.0, later.monotonic);
            ss.battery_soh.on_reading(95.0, later.monotonic);
            ss.battery_installed_capacity.on_reading(100.0, later.monotonic);
            ss.mppt_power_0.on_reading(1500.0, later.monotonic);
            ss.mppt_power_1.on_reading(1000.0, later.monotonic);
            ss.soltaro_power.on_reading(500.0, later.monotonic);
            ss.power_consumption.on_reading(1200.0, later.monotonic);
            ss.evcharger_ac_power.on_reading(0.0, later.monotonic);
            // battery_dc_power intentionally NOT refreshed — it ages out.
        }
        let _ = process(&Event::Tick { at: later.monotonic }, &mut world, &later, &Topology::defaults());

        assert_eq!(
            world.sensors.battery_dc_power.freshness,
            Freshness::Stale,
            "battery_dc_power must be Stale"
        );
        // Safety fallback must have fired: setpoint = 10 W, owner = System.
        assert_eq!(
            world.grid_setpoint.target.value,
            Some(10),
            "stale battery_dc_power → safety fallback → setpoint 10 W"
        );
        assert_eq!(
            world.grid_setpoint.target.owner,
            Owner::System,
            "safety fallback owner must be System"
        );
        // Decision must mention the missing sensor.
        let decision = world.decisions.grid_setpoint.as_ref().expect("decision set");
        assert!(
            decision.summary.contains("Safety fallback"),
            "decision summary should indicate safety fallback, got: {}",
            decision.summary
        );
        let missing_factor = decision.factors.iter().find(|f| f.name == "missing_sensors");
        assert!(
            missing_factor.is_some_and(|f| f.value.contains("battery_dc_power")),
            "missing_sensors factor must name battery_dc_power"
        );
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
    fn observer_mode_propose_target_still_sets_target_but_emits_no_write_effect() {
        // PR-SCHED0: observer mode (`writes_enabled = false`) must:
        //   - still call `propose_target` (so the dashboard can show the
        //     controller's intent via `world.*.target.value`),
        //   - emit at least one Info-level `observer` Log,
        //   - emit NO `WriteDbus` / `CallMyenergi` (actuation effects
        //     stay fully gated by writes_enabled).
        //
        // PR-SCHED0-D03 revises the contract: `Publish(ActuatedPhase)`
        // IS emitted in observer mode because it is a state-reporting
        // effect (dashboard retained MQTT), not an actuation effect.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // seed helper enables writes; flip back off to get observer mode.
        world.knobs.writes_enabled = false;
        // Raise SoC above export threshold so setpoint isn't just 10.
        world.sensors.battery_soc.on_reading(90.0, c.monotonic);

        let effects = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        // Setpoint controller proposed → phase is Pending (not Unset,
        // not Commanded) because the effect-emission path is suppressed.
        assert_eq!(
            world.grid_setpoint.target.phase, TargetPhase::Pending,
            "observer mode must still propose the target"
        );

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

        // No actuation effects — effect emission stays gated by
        // writes_enabled so the bus never physically changes.
        for e in &effects {
            assert!(
                !matches!(e, Effect::WriteDbus { .. } | Effect::CallMyenergi(_)),
                "observer mode must not emit actuation effects: {e:?}"
            );
        }

        // ActuatedPhase publish for the grid setpoint IS expected
        // (PR-SCHED0-D03). Verify phase=Pending since observer-mode
        // didn't call mark_commanded.
        let grid_publish = effects.iter().find_map(|e| match e {
            Effect::Publish(PublishPayload::ActuatedPhase {
                id: ActuatedId::GridSetpoint,
                phase,
            }) => Some(*phase),
            _ => None,
        });
        assert_eq!(
            grid_publish,
            Some(TargetPhase::Pending),
            "observer-mode tick must publish ActuatedPhase=Pending for the grid setpoint"
        );
    }

    #[test]
    fn schedule_0_target_is_always_enabled_in_observer_mode() {
        // PR-SCHED0 regression guard: in observer mode the schedules
        // controller must still propose schedule_0 with `days = 7`
        // (DAYS_ENABLED) so the dashboard reflects the controller's
        // intent. Prior to PR-SCHED0 the observer-mode early-return
        // skipped propose_target entirely, leaving `schedule_0.target`
        // at Unset while the bus retained whatever Venus held (e.g.
        // legacy `days=-7` from Node-RED), producing a
        // schedule_0-disabled reading on the dashboard.
        //
        // PR-SCHED0-D04: assert BOTH schedules land in Pending with
        // full ScheduleSpec equality, not just .days.
        use crate::controllers::schedules::DAYS_ENABLED;
        use crate::controllers::schedules::{
            evaluate_schedules, SchedulesInput, SchedulesInputGlobals,
        };

        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // battery_soc is all run_schedules needs; leave writes_enabled
        // at its fresh-boot default (false).
        world.sensors.battery_soc.on_reading(75.0, c.monotonic);
        assert!(!world.knobs.writes_enabled, "fresh boot must be observer mode");

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        // Compute the expected specs identically to run_schedules so
        // we can assert full equality (not just a single field).
        let bk = &world.bookkeeping;
        let input = SchedulesInput {
            globals: SchedulesInputGlobals {
                charge_battery_extended: bk.charge_to_full_required
                    || bk.charge_battery_extended_today,
                // PR-auto-extended-charge: mirror the dispatch in
                // `build_schedules_input` so the test's expected wire
                // matches what the controller actually consumes.
                charge_car_extended: super::effective_charge_car_extended(&world),
                charge_to_full_required: bk.charge_to_full_required,
                // PR-gamma-hold-redesign: mirror the dispatch in
                // `build_schedules_input` so the expected wire matches
                // what the controller actually consumed.
                disable_night_grid_discharge: super::effective_disable_night_grid_discharge(&world),
                zappi_active: world.derived.zappi_active,
                above_soc_date: bk.above_soc_date,
                battery_soc_target: super::effective_battery_soc_target(&world),
            },
            battery_soc: 75.0,
        };
        let expected = evaluate_schedules(&input, &c);

        let s0 = world
            .schedule_0
            .target
            .value
            .expect("schedule_0 target must be proposed in observer mode");
        assert_eq!(
            s0.days, DAYS_ENABLED,
            "schedule_0 must always be enabled (days=7); got {s0:?}"
        );
        assert_eq!(s0, expected.schedule_0, "schedule_0 full-spec mismatch");
        assert_eq!(world.schedule_0.target.phase, TargetPhase::Pending);

        let s1 = world
            .schedule_1
            .target
            .value
            .expect("schedule_1 target must be proposed in observer mode");
        assert_eq!(s1, expected.schedule_1, "schedule_1 full-spec mismatch");
        assert_eq!(world.schedule_1.target.phase, TargetPhase::Pending);
    }

    #[test]
    fn schedule_0_observer_then_kill_switch_true_emits_write_dbus_next_tick() {
        // PR-SCHED0-D04: two-tick observer → KillSwitch(true) → live
        // transition. Tick 1 observer mode: schedule_0.target.phase ==
        // Pending. Apply KillSwitch(true): reset to Unset fires for
        // every target. Tick 2 live mode: schedule_0.target.phase ==
        // Pending (fresh) AND a WriteDbus effect for Schedule { 0 } is
        // emitted. Mirrors
        // kill_switch_false_to_true_resets_pending_targets_and_forces_rewrite_next_tick
        // for schedules.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        world.sensors.battery_soc.on_reading(75.0, c.monotonic);
        assert!(!world.knobs.writes_enabled, "fresh boot must be observer mode");

        // Tick 1 — observer.
        let _ = process(
            &Event::Tick { at: c.monotonic },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(
            world.schedule_0.target.phase,
            TargetPhase::Pending,
            "observer tick leaves schedule_0 in Pending"
        );
        let observer_target = world
            .schedule_0
            .target
            .value
            .expect("observer tick proposed schedule_0 target");

        // KillSwitch(true) — edge-trigger resets every target to Unset,
        // then controllers re-run inside the same process() call and
        // immediately re-propose with a fresh WriteDbus batch. We
        // assert against `eff_on` directly (mirroring
        // kill_switch_false_to_true_resets_pending_targets_and_forces_rewrite_next_tick).
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
        assert_eq!(
            world.schedule_0.target.phase,
            TargetPhase::Commanded,
            "post-reset controller run moved schedule_0 to Commanded"
        );
        assert_eq!(
            world.schedule_0.target.value,
            Some(observer_target),
            "value unchanged across observer→live transition"
        );

        // Five WriteDbus effects for Schedule { index: 0 } (one per
        // field: Start, Duration, Soc, Days, AllowDischarge) must
        // appear in the KillSwitch(true) dispatch. Collect the
        // observed ScheduleFields into a set so we catch regressions
        // where, e.g., five writes carry the same field.
        use std::collections::HashSet;
        let schedule_fields: HashSet<ScheduleField> = eff_on
            .iter()
            .filter_map(|e| match e {
                Effect::WriteDbus {
                    target: DbusTarget::Schedule { index: 0, field },
                    ..
                } => Some(*field),
                _ => None,
            })
            .collect();
        let expected: HashSet<ScheduleField> = [
            ScheduleField::Start,
            ScheduleField::Duration,
            ScheduleField::Soc,
            ScheduleField::Days,
            ScheduleField::AllowDischarge,
        ]
        .into_iter()
        .collect();
        assert_eq!(
            schedule_fields, expected,
            "KillSwitch(true) must emit one schedule_0 WriteDbus per ScheduleField on the post-reset re-propose; got {eff_on:#?}"
        );
    }

    #[test]
    fn observer_mode_all_actuators_transition_to_pending_with_expected_values() {
        // PR-SCHED0-D05: replaces the coverage lost when
        // `observer_mode_does_not_mutate_target_phase` was deleted.
        // Seed a fully-populated World (all required sensors, zappi
        // state, knobs) then tick once in observer mode and assert
        // each of the six Actuated targets is either:
        //   - Pending with a sensible value, or
        //   - explicitly Unset (with a comment explaining why the
        //     controller has no usable input).
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // seed_required_sensors enables writes; flip back to observer.
        world.knobs.writes_enabled = false;
        // Raise SoC so eddi controller proposes Normal (soc > 96),
        // setpoint isn't clamped to 10 W floor, and schedules get
        // full-SoC context.
        world.sensors.battery_soc.on_reading(97.0, c.monotonic);

        let _ = process(
            &Event::Tick { at: c.monotonic },
            &mut world,
            &c,
            &Topology::defaults(),
        );

        // 1. grid_setpoint — setpoint controller ran with all sensors
        //    fresh; expect Pending.
        assert_eq!(
            world.grid_setpoint.target.phase,
            TargetPhase::Pending,
            "grid_setpoint should be Pending in observer mode"
        );
        assert!(
            world.grid_setpoint.target.value.is_some(),
            "grid_setpoint value must be set"
        );

        // 2. input_current_limit — current limit controller ran with
        //    all sensors fresh; expect Pending.
        assert_eq!(
            world.input_current_limit.target.phase,
            TargetPhase::Pending,
            "input_current_limit should be Pending in observer mode"
        );
        assert!(
            world.input_current_limit.target.value.is_some(),
            "input_current_limit value must be set"
        );

        // 3. schedule_0 — always enabled (days=7), Pending.
        assert_eq!(
            world.schedule_0.target.phase,
            TargetPhase::Pending,
            "schedule_0 should be Pending in observer mode"
        );
        assert!(world.schedule_0.target.value.is_some());

        // 4. schedule_1 — ditto, Pending.
        assert_eq!(
            world.schedule_1.target.phase,
            TargetPhase::Pending,
            "schedule_1 should be Pending in observer mode"
        );
        assert!(world.schedule_1.target.value.is_some());

        // 5. zappi_mode — zappi state is seeded to Off/EvDisconnected.
        //    Without charge_car_boost or charge_car_extended, and with
        //    the disconnected plug, evaluate_zappi_mode returns Leave
        //    at noon. Unset is expected.
        assert_eq!(
            world.zappi_mode.target.phase,
            TargetPhase::Unset,
            "zappi_mode stays Unset: noon + charge_car_boost=false + EV disconnected → Leave"
        );

        // 6. eddi_mode — SoC=97 > enable_soc=96, so EddiController
        //    proposes Normal. Pending expected.
        assert_eq!(
            world.eddi_mode.target.phase,
            TargetPhase::Pending,
            "eddi_mode should be Pending (soc above enable_soc)"
        );
        assert!(world.eddi_mode.target.value.is_some());
    }

    #[test]
    fn observer_mode_zappi_mode_transitions_to_pending_with_boost() {
        // PR-SCHED0-D05 companion: the sibling noon-fixture test leaves
        // zappi_mode at Unset because evaluate_zappi_mode returns Leave
        // for noon + no boost flags. That's a fragile anti-assertion —
        // it silently keeps passing if the controller's propose logic
        // changes. This test exercises the positive path: a 03:00
        // BOOST-window tick with `charge_car_boost = true` forces
        // evaluate_zappi_mode to return Set(Fast), so in observer mode
        // zappi_mode.target must land in Pending with value Fast.
        let c = clock_at(3, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // seed_required_sensors enables writes; flip back to observer.
        world.knobs.writes_enabled = false;
        world.knobs.charge_car_boost = true;

        let eff = process(
            &Event::Tick { at: c.monotonic },
            &mut world,
            &c,
            &Topology::defaults(),
        );

        assert_eq!(
            world.zappi_mode.target.phase,
            TargetPhase::Pending,
            "03:00 BOOST window + charge_car_boost=true must leave zappi_mode in Pending"
        );
        assert_eq!(
            world.zappi_mode.target.value,
            Some(ZappiMode::Fast),
            "BOOST window + charge_car_boost=true → mode=Fast"
        );
        // Observer-mode contract: target mutation happens but no
        // CallMyenergi effect leaks out.
        assert!(
            !eff.iter().any(|e| matches!(e, Effect::CallMyenergi(_))),
            "observer mode must not emit CallMyenergi: {eff:#?}"
        );
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
                    value: DbusValue::Float(_)
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
    fn setpoint_readback_out_of_tolerance_stays_commanded() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let target = world.grid_setpoint.target.value.unwrap();

        let _ = process(
            &Event::Sensor(SensorReading {
                id: SensorId::GridSetpointActual,
                value: f64::from(target + 200), // outside ±50 tolerance
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);
    }

    // ------------------------------------------------------------------
    // PR-actuated-as-sensors (PR-AS-A): the new sensor-id-based
    // confirmation path runs in parallel with `apply_readback`. These
    // tests pin the new path via `Event::Sensor(SensorReading{ id:
    // GridSetpointActual, ... })` and `Event::ScheduleReadback`.
    // ------------------------------------------------------------------

    #[test]
    fn apply_event_grid_setpoint_actual_confirms_target() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let target = world.grid_setpoint.target.value.unwrap();
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);

        // Drive a sensor reading on the actuated-mirror id, value within
        // the ±50 tolerance.
        let effects = process(
            &Event::Sensor(SensorReading {
                id: SensorId::GridSetpointActual,
                value: f64::from(target + 12),
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Confirmed);
        let publishes: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::Publish(PublishPayload::ActuatedPhase {
                        id: ActuatedId::GridSetpoint,
                        phase: TargetPhase::Confirmed,
                    })
                )
            })
            .collect();
        assert_eq!(
            publishes.len(),
            1,
            "expected exactly one ActuatedPhase(GridSetpoint, Confirmed) publish; got {effects:#?}",
        );
    }

    #[test]
    fn apply_event_current_limit_actual_confirms_target() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let target = world.input_current_limit.target.value.unwrap();
        assert_eq!(world.input_current_limit.target.phase, TargetPhase::Commanded);

        // Within the configured tolerance for current-limit confirmation.
        let tol = Topology::defaults()
            .controller_params
            .current_limit_confirm_tolerance_a;
        let effects = process(
            &Event::Sensor(SensorReading {
                id: SensorId::InputCurrentLimitActual,
                value: target + (tol / 2.0),
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.input_current_limit.target.phase, TargetPhase::Confirmed);
        let publishes: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::Publish(PublishPayload::ActuatedPhase {
                        id: ActuatedId::InputCurrentLimit,
                        phase: TargetPhase::Confirmed,
                    })
                )
            })
            .collect();
        assert_eq!(
            publishes.len(),
            1,
            "expected exactly one ActuatedPhase(InputCurrentLimit, Confirmed) publish; got {effects:#?}",
        );
    }

    #[test]
    fn apply_event_schedule_readback_confirms_target() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let target = world.schedule_0.target.value.expect("schedule_0 target set");
        assert_eq!(world.schedule_0.target.phase, TargetPhase::Commanded);

        let effects = process(
            &Event::ScheduleReadback {
                index: 0,
                value: target,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.schedule_0.target.phase, TargetPhase::Confirmed);
        let publishes: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::Publish(PublishPayload::ActuatedPhase {
                        id: ActuatedId::Schedule0,
                        phase: TargetPhase::Confirmed,
                    })
                )
            })
            .collect();
        assert_eq!(
            publishes.len(),
            1,
            "expected exactly one ActuatedPhase(Schedule0, Confirmed) publish; got {effects:#?}",
        );
    }

    // ------------------------------------------------------------------
    // PR-tz-from-victron: Event::Timezone
    // ------------------------------------------------------------------

    #[test]
    fn event_timezone_updates_world_and_handle() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let topo = Topology::defaults();
        // Sanity: starts at UTC default.
        assert_eq!(world.timezone, "Etc/UTC");
        assert!(world.timezone_updated_at.is_none());
        assert_eq!(topo.tz_handle.current(), chrono_tz::UTC);

        let _ = process(
            &Event::Timezone {
                value: "Europe/London".to_string(),
                at: c.monotonic,
            },
            &mut world,
            &c,
            &topo,
        );
        assert_eq!(world.timezone, "Europe/London");
        assert_eq!(world.timezone_updated_at, Some(c.monotonic));
        assert_eq!(topo.tz_handle.current(), chrono_tz::Europe::London);
    }

    #[test]
    fn event_timezone_invalid_logs_warn_and_leaves_state() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // Pre-seed a known-good Tz so we can assert it is NOT replaced.
        let topo = Topology::defaults();
        let _ = process(
            &Event::Timezone {
                value: "Europe/London".to_string(),
                at: c.monotonic,
            },
            &mut world,
            &c,
            &topo,
        );
        assert_eq!(world.timezone, "Europe/London");
        let stamped_at = world.timezone_updated_at;

        let effects = process(
            &Event::Timezone {
                value: "Not/A/Real/Zone".to_string(),
                at: c.monotonic + StdDuration::from_secs(1),
            },
            &mut world,
            &c,
            &topo,
        );
        // World state unchanged.
        assert_eq!(world.timezone, "Europe/London");
        assert_eq!(world.timezone_updated_at, stamped_at);
        assert_eq!(topo.tz_handle.current(), chrono_tz::Europe::London);
        // Warn log emitted.
        let warns: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::Log {
                        level: LogLevel::Warn,
                        source: "timezone",
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(
            warns.len(),
            1,
            "expected exactly one Warn log from the timezone arm; got {effects:#?}",
        );
    }

    // PR-AS-C: the ex-`zappi_mode_readback_drives_confirmation_on_exact_match`
    // test was deleted. It exercised the deleted `apply_readback::ZappiMode`    // arm; production never constructed `Event::Readback(ActuatedReadback::
    // ZappiMode)`, and the production `apply_typed_reading::Zappi` arm
    // updates only `world.typed_sensors.zappi_state` — there is no
    // production path that confirms `world.zappi_mode.target.phase`.
    // Migrating the test to `Event::TypedSensor(TypedReading::Zappi{...})`
    // would assert behaviour that does not exist; the gap (no confirm-side
    // for ZappiMode/EddiMode) is pre-existing and outside this PR's scope.

    // ------------------------------------------------------------------
    // Knob command (PR-gamma-hold-redesign — γ-hold removed)
    // ------------------------------------------------------------------

    #[test]
    fn knob_writes_from_any_owner_are_accepted() {
        // PR-gamma-hold-redesign: there is no γ-hold. A dashboard write
        // followed by an HA write inside the (former) 1 s window is
        // accepted; the user pins a manual override by flipping the
        // matching `*_mode` to `Forced` instead.
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
        assert_eq!(world.knobs.export_soc_threshold, 50.0);

        let _ = process(
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
        // No γ-hold suppression — the HA write lands on top.
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

    // ------------------------------------------------------------------
    // SetBookkeeping (PR-bookkeeping-edit) — user-driven dashboard edit.
    // Mutates the field AND publishes the retained MQTT body so the
    // change survives a restart.
    // ------------------------------------------------------------------

    #[test]
    fn set_bookkeeping_next_full_charge_writes_field_and_persists() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let dt = NaiveDate::from_ymd_opt(2026, 5, 3)
            .unwrap()
            .and_hms_opt(2, 0, 0)
            .unwrap();
        let eff = process(
            &Event::Command {
                command: Command::SetBookkeeping {
                    key: BookkeepingKey::NextFullCharge,
                    value: BookkeepingValue::NaiveDateTime(dt),
                },
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.bookkeeping.next_full_charge, Some(dt));
        assert!(eff.iter().any(|e| matches!(
            e,
            Effect::Publish(PublishPayload::Bookkeeping(
                BookkeepingKey::NextFullCharge,
                BookkeepingValue::NaiveDateTime(_),
            )),
        )));
    }

    #[test]
    fn set_bookkeeping_next_full_charge_cleared_sets_none() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // Seed a value first via the bootstrap path.
        let dt = NaiveDate::from_ymd_opt(2026, 5, 3)
            .unwrap()
            .and_hms_opt(2, 0, 0)
            .unwrap();
        world.bookkeeping.next_full_charge = Some(dt);

        let eff = process(
            &Event::Command {
                command: Command::SetBookkeeping {
                    key: BookkeepingKey::NextFullCharge,
                    value: BookkeepingValue::Cleared,
                },
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.bookkeeping.next_full_charge, None);
        assert!(eff.iter().any(|e| matches!(
            e,
            Effect::Publish(PublishPayload::Bookkeeping(
                BookkeepingKey::NextFullCharge,
                BookkeepingValue::Cleared,
            )),
        )));
    }

    #[test]
    fn set_bookkeeping_rejects_unsupported_keys() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let d = NaiveDate::from_ymd_opt(2026, 4, 21).unwrap();
        let before = world.bookkeeping.above_soc_date;

        let eff = process(
            &Event::Command {
                command: Command::SetBookkeeping {
                    key: BookkeepingKey::AboveSocDate,
                    value: BookkeepingValue::NaiveDate(d),
                },
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.bookkeeping.above_soc_date, before);
        assert!(eff.iter().any(|e| matches!(
            e,
            Effect::Log { level: LogLevel::Warn, source: "process::command", .. },
        )));
        assert!(!eff.iter().any(|e| matches!(
            e,
            Effect::Publish(PublishPayload::Bookkeeping(_, _)),
        )));
    }

    #[test]
    fn set_bookkeeping_rejects_type_mismatch() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let d = NaiveDate::from_ymd_opt(2026, 5, 3).unwrap();
        let before = world.bookkeeping.next_full_charge;

        let eff = process(
            &Event::Command {
                command: Command::SetBookkeeping {
                    key: BookkeepingKey::NextFullCharge,
                    value: BookkeepingValue::NaiveDate(d),
                },
                owner: Owner::Dashboard,
                at: c.monotonic,
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(world.bookkeeping.next_full_charge, before);
        assert!(eff.iter().any(|e| matches!(
            e,
            Effect::Log { level: LogLevel::Warn, source: "process::command", .. },
        )));
        assert!(!eff.iter().any(|e| matches!(
            e,
            Effect::Publish(PublishPayload::Bookkeeping(_, _)),
        )));
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
    fn eddi_requires_fresh_soc_and_pins_safety_target() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // No battery_soc reading — freshness Unknown. Controller's safety
        // direction is Stopped. Pre-EDDI-ALWAYS-ACTUATE (2026-04-25) the
        // target stayed Unset because Leave short-circuited before
        // propose_target. Now we always propose so the dashboard / HA see
        // the controller's intent — target.value=Stopped, phase=Pending,
        // owner=EddiController. No CallMyenergi fires (the device is
        // assumed-Stopped and Leave doesn't actuate).
        let effects = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        assert_eq!(world.eddi_mode.target.phase, TargetPhase::Pending);
        assert_eq!(world.eddi_mode.target.value, Some(EddiMode::Stopped));
        assert_eq!(world.eddi_mode.target.owner, Owner::EddiController);
        assert!(
            !effects.iter().any(|e| matches!(
                e,
                Effect::CallMyenergi(MyenergiAction::SetEddiMode(_))
            )),
            "Leave action must not fire CallMyenergi"
        );
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

        // Age SoC past freshness threshold (120 s).
        let later = FixedClock::new(c.monotonic + StdDuration::from_secs(130), naive(12, 0));
        let _ = process(&Event::Tick { at: later.monotonic }, &mut world, &later, &Topology::defaults());

        assert_eq!(world.sensors.battery_soc.freshness, Freshness::Stale);
        assert_eq!(world.eddi_mode.target.value, Some(EddiMode::Stopped));
    }

    // ------------------------------------------------------------------
    // PR-ACT-RETRY-1: universal actuator retry
    // ------------------------------------------------------------------

    /// Discrete actuator (eddi mode is an enum). After a write,
    /// `actual` arrives mismatching `target`. Within
    /// `actuator_retry_s`, no retry. Past it, the controller re-fires
    /// the same `Effect::CallMyenergi(SetEddiMode(...))` and
    /// `mark_commanded` updates `target.since`.
    #[test]
    fn eddi_mode_retries_after_threshold_when_actual_mismatches() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // SoC=58 is below disable_soc=94 → controller wants Stopped.
        world.sensors.battery_soc.on_reading(58.0, c.monotonic);
        // Pretend the device currently reports Normal — so the desired
        // target Stopped differs from current_mode → action=Set(Stopped).
        world
            .typed_sensors
            .eddi_mode
            .on_reading(EddiMode::Normal, c.monotonic);

        let retry_s = world.knobs.actuator_retry_s;

        // Tick 1: controller fires the write.
        let eff = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        assert!(
            eff.iter().any(|e| matches!(
                e,
                Effect::CallMyenergi(MyenergiAction::SetEddiMode(EddiMode::Stopped))
            )),
            "first tick must fire SetEddiMode(Stopped)"
        );
        assert_eq!(world.eddi_mode.target.phase, TargetPhase::Commanded);
        let since_after_first = world.eddi_mode.target.since;

        // Re-feed actual=Normal so the mismatch persists (the device
        // didn't comply). `on_reading` alone doesn't confirm; phase
        // stays Commanded.
        world
            .typed_sensors
            .eddi_mode
            .on_reading(EddiMode::Normal, c.monotonic);

        // Tick 2: same instant — within the retry window. No new write.
        let eff = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        assert!(
            !eff.iter().any(|e| matches!(
                e,
                Effect::CallMyenergi(MyenergiAction::SetEddiMode(_))
            )),
            "no retry within the threshold window"
        );

        // Tick 3: advance past the retry threshold — the controller
        // must re-fire the same write.
        let later = FixedClock::new(
            c.monotonic + StdDuration::from_secs(u64::from(retry_s) + 1),
            naive(12, 0),
        );
        // Refresh required sensors at `later` so freshness gates pass.
        {
            let ss = &mut world.sensors;
            ss.battery_soc.on_reading(58.0, later.monotonic);
            ss.battery_soh.on_reading(95.0, later.monotonic);
            ss.battery_installed_capacity.on_reading(100.0, later.monotonic);
            ss.battery_dc_power.on_reading(0.0, later.monotonic);
            ss.mppt_power_0.on_reading(1500.0, later.monotonic);
            ss.mppt_power_1.on_reading(1000.0, later.monotonic);
            ss.soltaro_power.on_reading(500.0, later.monotonic);
            ss.power_consumption.on_reading(1200.0, later.monotonic);
            ss.evcharger_ac_power.on_reading(0.0, later.monotonic);
        }
        world
            .typed_sensors
            .eddi_mode
            .on_reading(EddiMode::Normal, later.monotonic);

        let eff = process(&Event::Tick { at: later.monotonic }, &mut world, &later, &Topology::defaults());
        assert!(
            eff.iter().any(|e| matches!(
                e,
                Effect::CallMyenergi(MyenergiAction::SetEddiMode(EddiMode::Stopped))
            )),
            "retry must fire SetEddiMode(Stopped) after the threshold elapses"
        );
        // mark_commanded updated target.since to the new clock.
        assert!(
            world.eddi_mode.target.since > since_after_first,
            "mark_commanded must refresh target.since on retry"
        );
    }

    /// f64-shaped actuator (grid setpoint). Confirmed phase blocks
    /// retry even past `actuator_retry_s`. Two paths conspire to
    /// suppress the write here: (a) the per-tick computed setpoint
    /// matches the existing target exactly, so `propose_target`
    /// returns `changed=false`; (b) phase=Confirmed, so
    /// `needs_actuation` returns false. Either gate alone would
    /// suppress the write; both being satisfied is the steady-state
    /// invariant. The sibling `grid_setpoint_retries_after_threshold_*`
    /// test exercises the `needs_actuation` gate in isolation (deadband
    /// not applicable because phase != Confirmed).
    #[test]
    fn grid_setpoint_confirmed_phase_blocks_retry_past_threshold() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // Tick 1: setpoint controller fires, target lands at Commanded.
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let target_value = world.grid_setpoint.target.value.expect("target set");
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);

        // Feed the actual readback at exactly the target — confirm_if
        // (driven by `apply_sensor_reading`) promotes phase to Confirmed.
        let _ = process(
            &Event::Sensor(crate::types::SensorReading {
                id: SensorId::GridSetpointActual,
                value: f64::from(target_value),
                at: c.monotonic,
            }),
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert_eq!(
            world.grid_setpoint.target.phase,
            TargetPhase::Confirmed,
            "matching readback must promote phase to Confirmed"
        );

        // Advance well past the retry threshold.
        let retry_s = world.knobs.actuator_retry_s;
        let later = FixedClock::new(
            c.monotonic + StdDuration::from_secs(u64::from(retry_s) + 60),
            naive(12, 1),
        );
        // Refresh required sensors so freshness gates still pass.
        {
            let ss = &mut world.sensors;
            ss.battery_soc.on_reading(75.0, later.monotonic);
            ss.battery_soh.on_reading(95.0, later.monotonic);
            ss.battery_installed_capacity.on_reading(100.0, later.monotonic);
            ss.battery_dc_power.on_reading(0.0, later.monotonic);
            ss.mppt_power_0.on_reading(1500.0, later.monotonic);
            ss.mppt_power_1.on_reading(1000.0, later.monotonic);
            ss.soltaro_power.on_reading(500.0, later.monotonic);
            ss.power_consumption.on_reading(1200.0, later.monotonic);
            ss.grid_power.on_reading(500.0, later.monotonic);
            ss.grid_voltage.on_reading(230.0, later.monotonic);
            ss.grid_current.on_reading(2.0, later.monotonic);
            ss.consumption_current.on_reading(5.0, later.monotonic);
            ss.offgrid_power.on_reading(500.0, later.monotonic);
            ss.offgrid_current.on_reading(2.2, later.monotonic);
            ss.vebus_input_current.on_reading(0.0, later.monotonic);
            ss.evcharger_ac_power.on_reading(0.0, later.monotonic);
            ss.evcharger_ac_current.on_reading(0.0, later.monotonic);
        }

        let eff = process(&Event::Tick { at: later.monotonic }, &mut world, &later, &Topology::defaults());

        assert!(
            !eff.iter().any(|e| matches!(
                e,
                Effect::WriteDbus { target: DbusTarget::GridSetpoint, .. }
            )),
            "Confirmed phase must not retry, even past actuator_retry_s"
        );
        assert_eq!(
            world.grid_setpoint.target.phase,
            TargetPhase::Confirmed,
            "phase must remain Confirmed across the retry window"
        );
    }

    /// PR-ACT-RETRY-1 D01/D02 regression lock. Phase=Commanded with
    /// mismatching actual: the deadband filter must NOT pre-empt the
    /// retry path. Per-tick computed setpoint sits within the 25 W
    /// deadband of the existing target, but actual is 500 W away from
    /// target (well outside the 50 W confirm tolerance) so phase stays
    /// Commanded. Past `actuator_retry_s`, `needs_actuation` must
    /// return true and a fresh `Effect::WriteDbus(GridSetpoint, ...)`
    /// must fire.
    ///
    /// Under pre-D01 (deadband early-return unconditional): the
    /// dead-band match would early-return before `needs_actuation` is
    /// ever consulted, no write fires, this test FAILS.
    /// Under post-D01 (deadband gated on phase=Confirmed): phase is
    /// Commanded, deadband doesn't pre-empt, `needs_actuation` returns
    /// true past threshold, write fires, this test PASSES.
    #[test]
    fn grid_setpoint_retries_after_threshold_when_actual_mismatches_within_deadband() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // writes_enabled is required for WriteDbus emission.
        world.knobs.writes_enabled = true;

        // Hand-build a Commanded target at -1000 W with a mismatching
        // actual (-500 W is 500 W outside the 50 W confirm tolerance).
        let target_value: i32 = -1000;
        assert!(world
            .grid_setpoint
            .propose_target(target_value, Owner::SetpointController, c.monotonic));
        world.grid_setpoint.mark_commanded(c.monotonic);
        world.grid_setpoint.on_reading(-500, c.monotonic);
        // Sanity: confirm_if with the production tolerance must NOT
        // promote — the test's premise depends on phase staying Commanded.
        let tol = Topology::defaults().controller_params.setpoint_confirm_tolerance_w;
        let promoted = world
            .grid_setpoint
            .confirm_if(|t, a| (*t - *a).abs() <= tol, c.monotonic);
        assert!(!promoted, "actual must not promote to Confirmed");
        assert_eq!(world.grid_setpoint.target.phase, TargetPhase::Commanded);

        // Advance clock past actuator_retry_s.
        let retry_s = world.knobs.actuator_retry_s;
        let later_instant = c.monotonic + StdDuration::from_secs(u64::from(retry_s) + 5);

        // Per-tick computed setpoint sits WITHIN the 25 W retarget
        // deadband of the existing target. Under pre-D01 the deadband
        // filter would early-return here.
        let computed: i32 = -1010;
        let deadband = Topology::defaults().controller_params.setpoint_retarget_deadband_w;
        assert!((target_value - computed).abs() < deadband);

        let mut effects: Vec<Effect> = Vec::new();
        maybe_propose_setpoint(
            &mut world,
            computed,
            Owner::SetpointController,
            later_instant,
            Topology::defaults().controller_params,
            &mut effects,
        );

        let wrote = effects.iter().any(|e| matches!(
            e,
            Effect::WriteDbus { target: DbusTarget::GridSetpoint, .. }
        ));
        assert!(
            wrote,
            "retry path must emit WriteDbus when phase=Commanded with mismatching actual past retry threshold, even when computed value sits within the retarget deadband"
        );
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

        // Must exceed SensorId::BatterySoc.freshness_threshold() (120 s).
        let later = FixedClock::new(c.monotonic + StdDuration::from_secs(130), naive(12, 0));
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
            zappi_last_change_signature: c.monotonic,
            session_kwh: 0.0,
        };
        let _ = process(
            &Event::TypedSensor(TypedReading::Zappi {
                state,
                at: c.monotonic,
                raw_json: None,
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
    fn setpoint_decision_has_pre_and_post_clamp_factors_when_clamp_fires() {
        // PR-09a-D02 + D04: clamp factors are only emitted when the
        // clamp actually altered the value. Force that by setting an
        // export cap well below what the controller wants (SoC=99 + big
        // solar = far-negative pre-clamp; export_cap=2000 → capped to
        // -2000). Verify the factors are present AND that their
        // values match the actual pre/post numbers, not just the names.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        world.sensors.battery_soc.on_reading(99.0, c.monotonic);
        world.sensors.mppt_power_0.on_reading(5000.0, c.monotonic);
        world.sensors.mppt_power_1.on_reading(5000.0, c.monotonic);
        world.knobs.grid_export_limit_w = 2000;

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let decision = world.decisions.grid_setpoint.as_ref().expect("decision set");
        let factor = |name: &str| -> String {
            decision
                .factors
                .iter()
                .find(|f| f.name == name)
                .unwrap_or_else(|| panic!("missing factor {name}; have {:?}",
                    decision.factors.iter().map(|f| &f.name).collect::<Vec<_>>()))
                .value
                .clone()
        };
        let pre = factor("grid_cap_pre_W");
        let post = factor("grid_cap_post_W");
        let bounds = factor("grid_cap_bounds_W");
        // Pre-clamp must be more negative than post-clamp
        // (= the whole point of the clamp firing).
        let pre_n: i32 = pre.parse().expect("grid_cap_pre_W factor is an i32");
        let post_n: i32 = post.parse().expect("grid_cap_post_W factor is an i32");
        assert_eq!(
            post_n, -2000,
            "grid_cap_post_W should equal -export_cap when clamp fires"
        );
        assert!(
            pre_n < post_n,
            "grid_cap_pre_W ({pre_n}) should be more negative than grid_cap_post_W ({post_n}) when the clamp altered the value"
        );
        assert!(
            bounds.contains("-2000"),
            "grid_cap_bounds_W factor should mention the export cap: {bounds}"
        );
        assert_eq!(
            post_n, world.grid_setpoint.target.value.expect("target set"),
            "grid_cap_post_W must match the actual committed target"
        );
    }

    #[test]
    fn setpoint_decision_omits_clamp_factors_when_clamp_didnt_fire() {
        // PR-09a-D02: common case — pre_clamp is within the export/
        // import bounds; no clamp factors should be emitted.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let decision = world.decisions.grid_setpoint.as_ref().expect("decision set");
        let names: Vec<&str> = decision.factors.iter().map(|f| f.name.as_str()).collect();
        for bad in ["grid_cap_pre_W", "grid_cap_post_W", "grid_cap_bounds_W"] {
            assert!(
                !names.contains(&bad),
                "grid-cap factor {bad} emitted without clamp firing; factors = {names:?}"
            );
        }
    }

    // ------------------------------------------------------------------
    // PR-DAG-B: ZappiActiveCore → world.derived.zappi_active + A-15 cbe derivation
    // ------------------------------------------------------------------

    #[test]
    fn setpoint_first_tick_sees_derived_zappi_active() {
        // A-05 regression: on the very first tick, no prior state has
        // classified the Zappi. `world.derived.zappi_active` is the
        // default `false`. Setpoint must nevertheless see
        // zappi_active=true because `ZappiActiveCore` writes
        // `world.derived.zappi_active` from `typed_sensors` BEFORE
        // setpoint runs (enforced by `SetpointCore.depends_on =
        // [ZappiActive]`).
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);
        // Stamp the typed-sensor ZappiState as actively charging.
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Eco,
                zappi_plug_state: ZappiPlugState::Charging,
                zappi_status: ZappiStatus::DivertingOrCharging,
                zappi_last_change_signature: c.monotonic,
                session_kwh: 0.0,
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

        assert!(
            world.derived.zappi_active,
            "ZappiActiveCore must classify from typed_sensors on the first tick"
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
    fn setpoint_follows_derived_state_not_stale_classification() {
        // PR-DAG-B successor to the A-05/PR-04-D04 regression: even if a
        // prior tick's classification (now erased with
        // `bookkeeping.zappi_active`) had reported "active", setpoint
        // must follow the live typed state through `ZappiActiveCore`'s
        // per-tick write to `world.derived.zappi_active`. Here we seed
        // the derived field as `true` directly and assert the top-of-tick
        // `ZappiActiveCore` overwrites it from live sensors.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_required_sensors(&mut world, c.monotonic);

        // Simulate a stale derivation from a hypothetical earlier tick.
        world.derived.zappi_active = true;

        // Live state NOW: plug disconnected (definitively inactive).
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Off,
                zappi_plug_state: ZappiPlugState::EvDisconnected,
                zappi_status: ZappiStatus::Paused,
                zappi_last_change_signature: c.monotonic,
                session_kwh: 0.0,
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

        assert!(
            !world.derived.zappi_active,
            "ZappiActiveCore must recompute and clear the stale `true`"
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
            "setpoint followed stale world.derived.zappi_active=true when \
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
            .schedule_1
            .as_ref()
            .expect("schedule_1 decision published after midnight tick");
        let cbe = d
            .factors
            .iter()
            .find(|f| f.name == "cbe derivation")
            .expect("cbe derivation factor present on schedule_1");
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
        // After the schedule Decision split (to stop saying "Schedule 1
        // disabled" on the schedule_0 row), `cbe derivation` lives on
        // schedule_1's Decision only — schedule_0 is unconditionally
        // enabled and doesn't need cbe factors.
        let decision = world
            .decisions
            .schedule_1
            .as_ref()
            .expect("schedule_1 decision recorded");
        let cbe = decision
            .factors
            .iter()
            .find(|f| f.name == "cbe derivation")
            .expect("cbe derivation factor present on schedule_1");
        assert!(
            cbe.value.ends_with("= false"),
            "expected cbe to resolve false on fresh boot, got {cbe:?}"
        );
    }

    // ------------------------------------------------------------------
    // weather_soc — A-20 (γ-hold bypass) + A-21 (once-per-day guard)
    // ------------------------------------------------------------------

    /// Build a FixedClock at a specific H:M:S on 2026-04-21.
    fn clock_at_hms(h: u32, m: u32, s: u32) -> FixedClock {
        let nt = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(h, m, s)
            .unwrap();
        FixedClock::new(Instant::now(), nt)
    }

    /// Seed the minimal state `run_weather_soc` needs to fire its full
    /// decision path: outdoor_temperature fresh, and at least one fused
    /// forecast snapshot so `fused_today_kwh` returns Some.
    fn seed_weather_soc_inputs(world: &mut World, at: Instant) {
        world.sensors.outdoor_temperature.on_reading(10.0, at);
        world.typed_sensors.forecast_open_meteo = Some(ForecastSnapshot {
            today_kwh: 25.0,
            tomorrow_kwh: 25.0,
            fetched_at: at,
            hourly_kwh: Vec::new(),
        });
    }

    #[test]
    fn weather_soc_writes_bookkeeping_does_not_publish_knob() {
        // PR-gamma-hold-redesign: the planner no longer writes
        // `Knobs::*` for the four weather_soc-driven outputs. Instead
        // it writes `Bookkeeping::weather_soc_*`. There must be NO
        // Publish(Knob) for `ExportSocThreshold` / `DischargeSocTarget`
        // / `BatterySocTarget` / `DisableNightGridDischarge` from a
        // weather_soc tick — those topics are user-owned now.
        let c0 = clock_at_hms(12, 0, 0);
        let mut world = World::fresh_boot(c0.monotonic);
        seed_weather_soc_inputs(&mut world, c0.monotonic);

        let effects = process(&Event::Tick { at: c0.monotonic }, &mut world, &c0, &Topology::defaults());

        // No Publish(Knob) for the four weather_soc-driven outputs.
        let weather_knob_publishes: Vec<&KnobId> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::Publish(PublishPayload::Knob { id, .. }) => match id {
                    KnobId::ExportSocThreshold
                    | KnobId::DischargeSocTarget
                    | KnobId::BatterySocTarget
                    | KnobId::DisableNightGridDischarge => Some(id),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert!(
            weather_knob_publishes.is_empty(),
            "weather_soc must not publish knobs; got: {weather_knob_publishes:?}"
        );

        // Bookkeeping slots populated.
        assert!((world.bookkeeping.weather_soc_export_soc_threshold - 67.0).abs() < f64::EPSILON
            || (world.bookkeeping.weather_soc_export_soc_threshold - 100.0).abs() < f64::EPSILON
            || (world.bookkeeping.weather_soc_export_soc_threshold - 50.0).abs() < f64::EPSILON
            || (world.bookkeeping.weather_soc_export_soc_threshold - 35.0).abs() < f64::EPSILON
            || (world.bookkeeping.weather_soc_export_soc_threshold - 80.0).abs() < f64::EPSILON,
            "weather_soc must populate the bookkeeping slot to one of the legal export thresholds; got {}",
            world.bookkeeping.weather_soc_export_soc_threshold
        );

        // The user-owned knob is unchanged from safe_defaults.
        assert!((world.knobs.export_soc_threshold - 80.0).abs() < f64::EPSILON);

        // Once-per-day stamp still advances (informational now).
        assert_eq!(
            world.bookkeeping.last_weather_soc_run_date,
            Some(c0.naive.date())
        );
    }

    #[test]
    fn setpoint_reads_weather_soc_threshold_when_mode_weather() {
        // PR-gamma-hold-redesign: when `export_soc_threshold_mode =
        // Weather` (the default), the setpoint controller must read
        // `bookkeeping.weather_soc_export_soc_threshold`, not
        // `knobs.export_soc_threshold`.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // Diverge: knob says 50, bookkeeping says 67.
        world.knobs.export_soc_threshold = 50.0;
        world.bookkeeping.weather_soc_export_soc_threshold = 67.0;
        // Default mode = Weather.
        assert_eq!(
            super::effective_export_soc_threshold(&world),
            67.0,
            "Weather mode must dispatch to bookkeeping slot"
        );
    }

    #[test]
    fn setpoint_reads_forced_threshold_when_mode_forced() {
        // PR-gamma-hold-redesign: when `export_soc_threshold_mode =
        // Forced`, the setpoint controller must read
        // `knobs.export_soc_threshold`, ignoring the bookkeeping slot.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        world.knobs.export_soc_threshold = 50.0;
        world.bookkeeping.weather_soc_export_soc_threshold = 67.0;
        world.knobs.export_soc_threshold_mode = crate::knobs::Mode::Forced;
        assert_eq!(
            super::effective_export_soc_threshold(&world),
            50.0,
            "Forced mode must dispatch to user-owned knob"
        );
    }

    /// PR-ev-soc-sensor: an `Event::Sensor(EvSoc, ...)` lands on
    /// `world.sensors.ev_soc` and the slot becomes `Fresh` immediately.
    /// Pure dispatch test — no controller interaction.
    #[test]
    fn apply_sensor_reading_ev_soc_writes_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let event = Event::Sensor(SensorReading {
            id: SensorId::EvSoc,
            value: 75.0,
            at: c.monotonic,
        });
        let _ = process(&event, &mut world, &c, &Topology::defaults());
        assert_eq!(world.sensors.ev_soc.value, Some(75.0));
        assert_eq!(world.sensors.ev_soc.freshness, Freshness::Fresh);
    }

    #[test]
    fn mode_default_is_weather() {
        // PR-gamma-hold-redesign back-compat invariant: cold-start
        // safe_defaults set every `*_mode` to `Weather`. Together with
        // the bookkeeping initialisation (also matching safe_defaults)
        // the controller behaviour out-of-the-box matches pre-PR
        // (planner drives the four weather_soc-driven outputs).
        let k = crate::knobs::Knobs::safe_defaults();
        assert_eq!(k.export_soc_threshold_mode, crate::knobs::Mode::Weather);
        assert_eq!(k.discharge_soc_target_mode, crate::knobs::Mode::Weather);
        assert_eq!(k.battery_soc_target_mode, crate::knobs::Mode::Weather);
        assert_eq!(k.disable_night_grid_discharge_mode, crate::knobs::Mode::Weather);
    }

    // ------------------------------------------------------------------
    // PR-auto-extended-charge
    // ------------------------------------------------------------------

    /// PR-auto-extended-charge: dispatch test for the new sensor.
    #[test]
    fn apply_sensor_reading_ev_charge_target_writes_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let event = Event::Sensor(SensorReading {
            id: SensorId::EvChargeTarget,
            value: 90.0,
            at: c.monotonic,
        });
        let _ = process(&event, &mut world, &c, &Topology::defaults());
        assert_eq!(world.sensors.ev_charge_target.value, Some(90.0));
        assert_eq!(world.sensors.ev_charge_target.freshness, Freshness::Fresh);
    }

    /// PR-auto-extended-charge: truth table for the effective helper.
    #[test]
    fn effective_charge_car_extended_truth_table() {
        use crate::knobs::ExtendedChargeMode;
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);

        world.knobs.charge_car_extended_mode = ExtendedChargeMode::Forced;
        assert!(super::effective_charge_car_extended(&world));

        world.knobs.charge_car_extended_mode = ExtendedChargeMode::Disabled;
        assert!(!super::effective_charge_car_extended(&world));

        world.knobs.charge_car_extended_mode = ExtendedChargeMode::Auto;
        world.bookkeeping.auto_extended_today = true;
        assert!(super::effective_charge_car_extended(&world));
        world.bookkeeping.auto_extended_today = false;
        assert!(!super::effective_charge_car_extended(&world));
    }

    /// PR-auto-extended-charge: Forced mode short-circuits — no
    /// bookkeeping mutation regardless of EV sensor state.
    #[test]
    fn maybe_evaluate_auto_extended_skips_in_forced_mode() {
        use crate::knobs::ExtendedChargeMode;
        let c = clock_at_hms(4, 30, 0);
        let mut world = World::fresh_boot(c.monotonic);
        world.knobs.charge_car_extended_mode = ExtendedChargeMode::Forced;
        // Even with a Fresh low SoC reading, Forced must NOT touch
        // bookkeeping — the latch fields stay at their defaults.
        world.sensors.ev_soc.on_reading(20.0, c.monotonic);
        super::maybe_evaluate_auto_extended(&mut world, &c);
        assert!(!world.bookkeeping.auto_extended_today);
        assert_eq!(world.bookkeeping.auto_extended_today_date, None);
    }

    /// PR-auto-extended-charge: pre-04:30 the evaluator does not fire.
    #[test]
    fn maybe_evaluate_auto_extended_does_not_fire_pre_0430() {
        let c = clock_at_hms(4, 29, 0);
        let mut world = World::fresh_boot(c.monotonic);
        world.sensors.ev_soc.on_reading(20.0, c.monotonic);
        super::maybe_evaluate_auto_extended(&mut world, &c);
        assert_eq!(world.bookkeeping.auto_extended_today_date, None);
        assert!(!world.bookkeeping.auto_extended_today);
    }

    /// PR-auto-extended-charge: 04:30 with Fresh low SoC enables.
    #[test]
    fn maybe_evaluate_auto_extended_fires_at_0430() {
        let c = clock_at_hms(4, 30, 0);
        let mut world = World::fresh_boot(c.monotonic);
        world.sensors.ev_soc.on_reading(30.0, c.monotonic);
        super::maybe_evaluate_auto_extended(&mut world, &c);
        assert!(world.bookkeeping.auto_extended_today);
        assert_eq!(
            world.bookkeeping.auto_extended_today_date,
            Some(c.naive().date()),
        );
    }

    /// PR-auto-extended-charge: 04:30 with Fresh high SoC but a high
    /// configured target (`> 80`) still enables.
    #[test]
    fn maybe_evaluate_auto_extended_fires_at_0430_with_high_target() {
        let c = clock_at_hms(4, 30, 0);
        let mut world = World::fresh_boot(c.monotonic);
        world.sensors.ev_soc.on_reading(80.0, c.monotonic);
        world.sensors.ev_charge_target.on_reading(90.0, c.monotonic);
        super::maybe_evaluate_auto_extended(&mut world, &c);
        assert!(world.bookkeeping.auto_extended_today);
    }

    /// PR-auto-extended-charge: idempotent within the same local date.
    /// Two 04:30 ticks with conflicting SoC values must not flip the
    /// latch a second time.
    #[test]
    fn maybe_evaluate_auto_extended_skips_when_already_evaluated_today() {
        let c = clock_at_hms(4, 30, 0);
        let mut world = World::fresh_boot(c.monotonic);
        world.sensors.ev_soc.on_reading(30.0, c.monotonic);
        super::maybe_evaluate_auto_extended(&mut world, &c);
        assert!(world.bookkeeping.auto_extended_today);

        // Same day, later in the morning — change the SoC to a value
        // that would have flipped the answer; idempotent latch must
        // suppress re-evaluation.
        let c2 = clock_at_hms(7, 0, 0);
        world.sensors.ev_soc.on_reading(95.0, c2.monotonic);
        world.sensors.ev_charge_target.on_reading(50.0, c2.monotonic);
        super::maybe_evaluate_auto_extended(&mut world, &c2);
        assert!(
            world.bookkeeping.auto_extended_today,
            "second call same date must not re-evaluate (latch preserved)",
        );
    }

    /// PR-auto-extended-charge: stale SoC at 04:30 → defensively
    /// disable, regardless of value or target.
    #[test]
    fn maybe_evaluate_auto_extended_disables_on_stale_soc() {
        let c = clock_at_hms(4, 30, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // Force the SoC slot stale: never call on_reading.
        assert_eq!(world.sensors.ev_soc.freshness, Freshness::Unknown);
        // Even if the (Fresh) target says 90 — without a current SoC we
        // must not pull cheap-rate grid.
        world.sensors.ev_charge_target.on_reading(90.0, c.monotonic);
        super::maybe_evaluate_auto_extended(&mut world, &c);
        assert!(!world.bookkeeping.auto_extended_today);
        assert_eq!(
            world.bookkeeping.auto_extended_today_date,
            Some(c.naive().date()),
        );
    }

    /// PR-auto-extended-charge: the latch is per-date — a tick the
    /// next day re-fires the evaluation.
    #[test]
    fn maybe_evaluate_auto_extended_re_fires_next_day() {
        // Day 1 at 04:30: low SoC → enables, latches today's date.
        let c1 = clock_at_hms(4, 30, 0);
        let mut world = World::fresh_boot(c1.monotonic);
        world.sensors.ev_soc.on_reading(20.0, c1.monotonic);
        super::maybe_evaluate_auto_extended(&mut world, &c1);
        assert!(world.bookkeeping.auto_extended_today);
        let day1 = c1.naive().date();
        assert_eq!(world.bookkeeping.auto_extended_today_date, Some(day1));

        // Day 2 at 04:30: SoC fresh but high, target low → disables.
        let day2_naive = NaiveDate::from_ymd_opt(2026, 4, 22)
            .unwrap()
            .and_hms_opt(4, 30, 0)
            .unwrap();
        let c2 = FixedClock::new(
            c1.monotonic + StdDuration::from_secs(24 * 3600),
            day2_naive,
        );
        // Refresh the SoC reading on day 2 so it's still Fresh
        world.sensors.ev_soc.on_reading(95.0, c2.monotonic);
        super::maybe_evaluate_auto_extended(&mut world, &c2);
        assert_eq!(
            world.bookkeeping.auto_extended_today_date,
            Some(c2.naive().date()),
            "next-day tick must overwrite the latch date",
        );
        assert!(
            !world.bookkeeping.auto_extended_today,
            "high SoC + no high target on day 2 must flip the latch off",
        );
    }

    // PR-pinned-registers ----------------------------------------------------

    fn seed_pinned_register(
        world: &mut World,
        path: &str,
        target: PinnedValue,
    ) -> std::sync::Arc<str> {
        let key: std::sync::Arc<str> = std::sync::Arc::from(path);
        world.pinned_registers.insert(
            std::sync::Arc::clone(&key),
            crate::types::PinnedRegisterEntity::new(std::sync::Arc::clone(&key), target),
        );
        key
    }

    #[test]
    fn pinned_register_match_marks_confirmed_no_write() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let key = seed_pinned_register(
            &mut world,
            "com.victronenergy.settings:/Settings/CGwacs/Hub4Mode",
            PinnedValue::Int(1),
        );

        let effects = process(
            &Event::PinnedRegisterReading {
                path: key.as_ref().to_string(),
                value: PinnedValue::Int(1),
                at: c.naive(),
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );

        let entity = world.pinned_registers.get(&key).unwrap();
        assert_eq!(entity.status, PinnedStatus::Confirmed);
        assert_eq!(entity.drift_count, 0);
        assert_eq!(entity.actual, Some(PinnedValue::Int(1)));
        assert!(entity.last_check.is_some());
        // No corrective write on a match.
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::WriteDbusPinned { .. })),
            "match path must not emit WriteDbusPinned; got {effects:#?}",
        );
    }

    #[test]
    fn pinned_register_drift_emits_corrective_write_and_increments_counter() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let key = seed_pinned_register(
            &mut world,
            "com.victronenergy.settings:/Settings/CGwacs/Hub4Mode",
            PinnedValue::Int(1),
        );

        let effects = process(
            &Event::PinnedRegisterReading {
                path: key.as_ref().to_string(),
                value: PinnedValue::Int(0), // drifted
                at: c.naive(),
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );

        let entity = world.pinned_registers.get(&key).unwrap();
        assert_eq!(entity.status, PinnedStatus::Drifted);
        assert_eq!(entity.drift_count, 1);
        assert_eq!(entity.actual, Some(PinnedValue::Int(0)));
        assert_eq!(entity.last_drift_at, Some(c.naive()));
        // Exactly one corrective write, addressed to the right service+path
        // with the configured target value.
        let writes: Vec<_> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::WriteDbusPinned { service, path, value } => {
                    Some((service.as_str(), path.as_str(), value.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(writes.len(), 1, "expected one WriteDbusPinned; got {effects:#?}");
        assert_eq!(writes[0].0, "com.victronenergy.settings");
        assert_eq!(writes[0].1, "/Settings/CGwacs/Hub4Mode");
        assert_eq!(writes[0].2, PinnedValue::Int(1));
        // Honesty invariant: a Log effect explains old/new.
        assert!(
            effects
                .iter()
                .any(|e| matches!(
                    e,
                    Effect::Log { source: "pinned_registers", message, .. }
                        if message.contains("pinned_register_restored") && message.contains('0') && message.contains('1')
                )),
            "missing pinned_register_restored Log: {effects:#?}",
        );
    }

    #[test]
    fn pinned_register_bool_int_coercion_match() {
        // Victron returns Int(1) over the wire even when the user wrote
        // a Python boolean. The match must succeed.
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let key = seed_pinned_register(
            &mut world,
            "com.victronenergy.vebus.ttyS3:/Devices/0/Settings/PowerAssistEnabled",
            PinnedValue::Bool(true),
        );
        let effects = process(
            &Event::PinnedRegisterReading {
                path: key.as_ref().to_string(),
                value: PinnedValue::Int(1),
                at: c.naive(),
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        let entity = world.pinned_registers.get(&key).unwrap();
        assert_eq!(entity.status, PinnedStatus::Confirmed);
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::WriteDbusPinned { .. })),
            "bool/int(1) coercion must not emit a write",
        );
    }

    #[test]
    fn pinned_register_float_tolerance() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let key = seed_pinned_register(
            &mut world,
            "com.victronenergy.settings:/Settings/CGwacs/MaxFeedInPower",
            PinnedValue::Float(5000.0),
        );
        // Within tolerance.
        let effects = process(
            &Event::PinnedRegisterReading {
                path: key.as_ref().to_string(),
                value: PinnedValue::Float(5_000.000_001),
                at: c.naive(),
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        let entity = world.pinned_registers.get(&key).unwrap();
        assert_eq!(entity.status, PinnedStatus::Confirmed);
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::WriteDbusPinned { .. })),
        );
        // Far outside tolerance — drift.
        let effects = process(
            &Event::PinnedRegisterReading {
                path: key.as_ref().to_string(),
                value: PinnedValue::Float(100.0),
                at: c.naive(),
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        let entity = world.pinned_registers.get(&key).unwrap();
        assert_eq!(entity.status, PinnedStatus::Drifted);
        assert_eq!(entity.drift_count, 1);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::WriteDbusPinned { .. })),
        );
    }

    #[test]
    fn pinned_register_unknown_path_warns_and_drops() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // No seed: world.pinned_registers is empty.
        let effects = process(
            &Event::PinnedRegisterReading {
                path: "com.victronenergy.settings:/Settings/Foo".to_string(),
                value: PinnedValue::Int(1),
                at: c.naive(),
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        assert!(world.pinned_registers.is_empty());
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::WriteDbusPinned { .. })),
            "unknown path must not emit a corrective write",
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(
                    e,
                    Effect::Log { level: LogLevel::Warn, source: "pinned_registers", .. }
                )),
            "unknown path must produce a Warn log: {effects:#?}",
        );
    }

    // ------------------------------------------------------------------
    // PR-ZD-1: dispatch tests for the four new sensors.
    // ------------------------------------------------------------------

    /// PR-ZD-1: `Event::Sensor(HeatPumpPower, ...)` lands on
    /// `world.sensors.heat_pump_power` and the slot becomes `Fresh`.
    #[test]
    fn apply_sensor_reading_heat_pump_power_writes_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let event = Event::Sensor(SensorReading {
            id: SensorId::HeatPumpPower,
            value: 1200.0,
            at: c.monotonic,
        });
        let _ = process(&event, &mut world, &c, &Topology::defaults());
        assert_eq!(world.sensors.heat_pump_power.value, Some(1200.0));
        assert_eq!(world.sensors.heat_pump_power.freshness, Freshness::Fresh);
    }

    /// PR-ZD-1: `Event::Sensor(CookerPower, ...)` lands on
    /// `world.sensors.cooker_power` and the slot becomes `Fresh`.
    #[test]
    fn apply_sensor_reading_cooker_power_writes_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let event = Event::Sensor(SensorReading {
            id: SensorId::CookerPower,
            value: 2500.0,
            at: c.monotonic,
        });
        let _ = process(&event, &mut world, &c, &Topology::defaults());
        assert_eq!(world.sensors.cooker_power.value, Some(2500.0));
        assert_eq!(world.sensors.cooker_power.freshness, Freshness::Fresh);
    }

    /// PR-ZD-1: `Event::Sensor(Mppt0OperationMode, ...)` lands on
    /// `world.sensors.mppt_0_operation_mode` and the slot becomes `Fresh`.
    #[test]
    fn apply_sensor_reading_mppt_0_operation_mode_writes_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let event = Event::Sensor(SensorReading {
            id: SensorId::Mppt0OperationMode,
            value: 2.0,
            at: c.monotonic,
        });
        let _ = process(&event, &mut world, &c, &Topology::defaults());
        assert_eq!(world.sensors.mppt_0_operation_mode.value, Some(2.0));
        assert_eq!(world.sensors.mppt_0_operation_mode.freshness, Freshness::Fresh);
    }

    /// PR-ZD-1: `Event::Sensor(Mppt1OperationMode, ...)` lands on
    /// `world.sensors.mppt_1_operation_mode` and the slot becomes `Fresh`.
    #[test]
    fn apply_sensor_reading_mppt_1_operation_mode_writes_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let event = Event::Sensor(SensorReading {
            id: SensorId::Mppt1OperationMode,
            value: 2.0,
            at: c.monotonic,
        });
        let _ = process(&event, &mut world, &c, &Topology::defaults());
        assert_eq!(world.sensors.mppt_1_operation_mode.value, Some(2.0));
        assert_eq!(world.sensors.mppt_1_operation_mode.freshness, Freshness::Fresh);
    }

    /// PR-ZD-1 / D01: out-of-enum-range readings must be dropped; the slot
    /// stays Unknown (value == None) because `on_reading` is never called.
    #[test]
    fn mppt_operation_mode_out_of_enum_range_is_dropped() {
        for bad_value in [99.0_f64, -1.0, f64::NAN, f64::INFINITY, 5.5] {
            let c = clock_at(12, 0);
            let mut world = World::fresh_boot(c.monotonic);

            let event0 = Event::Sensor(SensorReading {
                id: SensorId::Mppt0OperationMode,
                value: bad_value,
                at: c.monotonic,
            });
            let _ = process(&event0, &mut world, &c, &Topology::defaults());
            assert!(
                world.sensors.mppt_0_operation_mode.value.is_none(),
                "Mppt0OperationMode: expected slot to remain None for value={bad_value}",
            );

            let event1 = Event::Sensor(SensorReading {
                id: SensorId::Mppt1OperationMode,
                value: bad_value,
                at: c.monotonic,
            });
            let _ = process(&event1, &mut world, &c, &Topology::defaults());
            assert!(
                world.sensors.mppt_1_operation_mode.value.is_none(),
                "Mppt1OperationMode: expected slot to remain None for value={bad_value}",
            );
        }
    }

    #[test]
    fn pinned_register_drift_count_does_not_reset_on_reconfirm() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let key = seed_pinned_register(
            &mut world,
            "com.victronenergy.settings:/Settings/CGwacs/Hub4Mode",
            PinnedValue::Int(1),
        );
        // Drift once.
        let _ = process(
            &Event::PinnedRegisterReading {
                path: key.as_ref().to_string(),
                value: PinnedValue::Int(0),
                at: c.naive(),
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        // Then a fresh confirmation. drift_count must stay at 1 — the
        // operator-facing counter is monotonic.
        let _ = process(
            &Event::PinnedRegisterReading {
                path: key.as_ref().to_string(),
                value: PinnedValue::Int(1),
                at: c.naive(),
            },
            &mut world,
            &c,
            &Topology::defaults(),
        );
        let entity = world.pinned_registers.get(&key).unwrap();
        assert_eq!(entity.status, PinnedStatus::Confirmed);
        assert_eq!(entity.drift_count, 1);
    }

    // ------------------------------------------------------------------
    // PR-ZD-2: apply_knob routing for the five compensated-drain knobs
    // ------------------------------------------------------------------

    fn send_knob(world: &mut World, id: KnobId, value: KnobValue) {
        let c = clock_at(12, 0);
        let _ = process(
            &Event::Command {
                command: Command::Knob { id, value },
                owner: Owner::HaMqtt,
                at: c.monotonic,
            },
            world,
            &c,
            &Topology::defaults(),
        );
    }

    #[test]
    fn apply_knob_zappi_battery_drain_threshold_w_routes_to_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        send_knob(&mut world, KnobId::ZappiBatteryDrainThresholdW, KnobValue::Uint32(2500));
        assert_eq!(world.knobs.zappi_battery_drain_threshold_w, 2500);
    }

    #[test]
    fn apply_knob_zappi_battery_drain_relax_step_w_routes_to_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        send_knob(&mut world, KnobId::ZappiBatteryDrainRelaxStepW, KnobValue::Uint32(250));
        assert_eq!(world.knobs.zappi_battery_drain_relax_step_w, 250);
    }

    #[test]
    fn apply_knob_zappi_battery_drain_kp_routes_to_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        send_knob(&mut world, KnobId::ZappiBatteryDrainKp, KnobValue::Float(0.5));
        assert!((world.knobs.zappi_battery_drain_kp - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn apply_knob_zappi_battery_drain_target_w_routes_to_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // target_w routes via Float (no KnobValue::Int32); controller rounds.
        send_knob(&mut world, KnobId::ZappiBatteryDrainTargetW, KnobValue::Float(-300.0));
        assert_eq!(world.knobs.zappi_battery_drain_target_w, -300);
    }

    #[test]
    fn apply_knob_zappi_battery_drain_hard_clamp_w_routes_to_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        send_knob(&mut world, KnobId::ZappiBatteryDrainHardClampW, KnobValue::Uint32(500));
        assert_eq!(world.knobs.zappi_battery_drain_hard_clamp_w, 500);
    }

    #[test]
    fn apply_knob_zappi_battery_drain_mppt_probe_w_routes_to_field() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        send_knob(&mut world, KnobId::ZappiBatteryDrainMpptProbeW, KnobValue::Uint32(750));
        assert_eq!(world.knobs.zappi_battery_drain_mppt_probe_w, 750);
    }

    // ------------------------------------------------------------------
    // PR-WSOC-EDIT-1: apply_knob routing for the 48 weather-SoC table
    // cell knobs (one programmatic arm, four field shapes).
    // ------------------------------------------------------------------

    #[test]
    fn apply_knob_weathersoc_table_cell_routes_export_soc_threshold() {
        use crate::weather_soc_addr::{CellField, EnergyBucket, TempCol};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // Sunny.warm export-soc-threshold: default 50 → write 42.
        send_knob(
            &mut world,
            KnobId::WeathersocTableCell {
                bucket: EnergyBucket::Sunny,
                temp: TempCol::Warm,
                field: CellField::ExportSocThreshold,
            },
            KnobValue::Float(42.0),
        );
        assert!(
            (world.knobs.weather_soc_table.sunny_warm.export_soc_threshold - 42.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn apply_knob_weathersoc_table_cell_routes_battery_soc_target() {
        use crate::weather_soc_addr::{CellField, EnergyBucket, TempCol};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // Low.cold battery-soc-target: default 100 → write 85.
        send_knob(
            &mut world,
            KnobId::WeathersocTableCell {
                bucket: EnergyBucket::Low,
                temp: TempCol::Cold,
                field: CellField::BatterySocTarget,
            },
            KnobValue::Float(85.0),
        );
        assert!(
            (world.knobs.weather_soc_table.low_cold.battery_soc_target - 85.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn apply_knob_weathersoc_table_cell_routes_discharge_soc_target() {
        use crate::weather_soc_addr::{CellField, EnergyBucket, TempCol};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // VeryDim.cold discharge-soc-target: default 30 → write 25.
        send_knob(
            &mut world,
            KnobId::WeathersocTableCell {
                bucket: EnergyBucket::VeryDim,
                temp: TempCol::Cold,
                field: CellField::DischargeSocTarget,
            },
            KnobValue::Float(25.0),
        );
        assert!(
            (world.knobs.weather_soc_table.very_dim_cold.discharge_soc_target - 25.0).abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn apply_knob_weathersoc_table_cell_routes_extended() {
        use crate::weather_soc_addr::{CellField, EnergyBucket, TempCol};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // VerySunny.warm extended: default false → write true.
        send_knob(
            &mut world,
            KnobId::WeathersocTableCell {
                bucket: EnergyBucket::VerySunny,
                temp: TempCol::Warm,
                field: CellField::Extended,
            },
            KnobValue::Bool(true),
        );
        assert!(world.knobs.weather_soc_table.very_sunny_warm.extended);
    }

    #[test]
    fn all_knob_publish_payloads_includes_48_weathersoc_table_cells() {
        use crate::weather_soc_addr::CellField;
        let k = crate::knobs::Knobs::safe_defaults();
        let payloads = all_knob_publish_payloads(&k);
        let cell_payloads: Vec<_> = payloads
            .iter()
            .filter_map(|p| match p {
                PublishPayload::Knob {
                    id: KnobId::WeathersocTableCell { bucket, temp, field },
                    value,
                } => Some((*bucket, *temp, *field, *value)),
                _ => None,
            })
            .collect();
        assert_eq!(cell_payloads.len(), 48, "expected 48 cell knob payloads");

        let floats = cell_payloads
            .iter()
            .filter(|(_, _, _, v)| matches!(v, KnobValue::Float(_)))
            .count();
        let bools = cell_payloads
            .iter()
            .filter(|(_, _, _, v)| matches!(v, KnobValue::Bool(_)))
            .count();
        // 12 cells × 3 float fields = 36; 12 cells × 1 bool field = 12.
        assert_eq!(floats, 36, "36 Float payloads (3 float fields × 12 cells)");
        assert_eq!(bools, 12, "12 Bool payloads (Extended × 12 cells)");

        // Spot-check: every (bucket, temp) pair appears exactly 4 times,
        // and every CellField appears exactly 12 times.
        for &field in CellField::ALL {
            let n = cell_payloads.iter().filter(|(_, _, f, _)| *f == field).count();
            assert_eq!(n, 12, "field {field:?}");
        }
    }

    // ------------------------------------------------------------------
    // PR-ZDP-1: mppt_curtailed helper tests
    // ------------------------------------------------------------------

    /// ZDP-H1: both mppt op-mode sensors Unknown → mppt_curtailed returns false
    /// (conservative — don't probe when sensors are stale/unknown).
    #[test]
    fn mppt_curtailed_helper_handles_stale() {
        let c = clock_at(12, 0);
        let world = World::fresh_boot(c.monotonic);
        // fresh_boot leaves sensors Unknown (no reading received).
        assert!(world.sensors.mppt_0_operation_mode.value.is_none(), "precondition");
        assert!(world.sensors.mppt_1_operation_mode.value.is_none(), "precondition");
        assert!(!mppt_curtailed(&world), "stale sensors must not trigger probe");
    }

    /// ZDP-H2: table-driven — mode 1 on either channel → true; mode 2 or 0 on
    /// both → false.
    #[test]
    fn mppt_curtailed_helper_returns_true_on_either_mode_1() {
        let cases: &[(Option<f64>, Option<f64>, bool)] = &[
            (Some(1.0), Some(2.0), true),   // mppt0 curtailed
            (Some(2.0), Some(1.0), true),   // mppt1 curtailed
            (Some(1.0), Some(1.0), true),   // both curtailed
            (Some(2.0), Some(2.0), false),  // both tracking
            (Some(0.0), Some(0.0), false),  // both off
            (Some(2.0), None,      false),  // mppt1 stale
            (None,      Some(2.0), false),  // mppt0 stale
            (None,      None,      false),  // both stale
        ];

        for &(mode0, mode1, expected) in cases {
            let c = clock_at(12, 0);
            let mut world = World::fresh_boot(c.monotonic);
            if let Some(v) = mode0 {
                world.sensors.mppt_0_operation_mode.on_reading(v, c.monotonic);
            }
            if let Some(v) = mode1 {
                world.sensors.mppt_1_operation_mode.on_reading(v, c.monotonic);
            }
            assert_eq!(
                mppt_curtailed(&world), expected,
                "mode0={mode0:?} mode1={mode1:?}: expected {expected}"
            );
        }
    }

    /// ZDP-H3: stale-with-cached-mode-1 — both channels received mode 1 then
    /// went Stale (no further updates). The helper must return false because
    /// Stale is not usable — `is_curtailed` gates on `is_usable()`.
    ///
    /// This is the production-realistic failure mode that D01 addresses:
    /// a sensor that read mode 1 once, stopped reporting, and decayed to
    /// Stale retains `value: Some(1.0)` with `freshness: Stale`. The
    /// old implementation (checking `.value` directly) would have returned
    /// true here.
    #[test]
    fn mppt_curtailed_helper_returns_false_on_stale_cached_mode_1() {
        use std::time::Duration;
        let c = clock_at(12, 0);
        let t0 = c.monotonic;
        let mut world = World::fresh_boot(t0);

        // Both channels receive mode 1 → Fresh, value Some(1.0).
        world.sensors.mppt_0_operation_mode.on_reading(1.0, t0);
        world.sensors.mppt_1_operation_mode.on_reading(1.0, t0);
        assert!(world.sensors.mppt_0_operation_mode.is_usable(), "precondition: fresh after reading");
        assert_eq!(world.sensors.mppt_0_operation_mode.value, Some(1.0), "precondition: value is mode 1");

        // Advance past the 30 s freshness threshold → both decay to Stale.
        let stale_at = t0 + Duration::from_secs(60);
        let threshold = crate::types::SensorId::Mppt0OperationMode.freshness_threshold();
        world.sensors.mppt_0_operation_mode.tick(stale_at, threshold);
        world.sensors.mppt_1_operation_mode.tick(stale_at, threshold);

        // Verify: slots are Stale but still hold the cached mode-1 value.
        assert!(!world.sensors.mppt_0_operation_mode.is_usable(), "must be Stale");
        assert_eq!(world.sensors.mppt_0_operation_mode.value, Some(1.0), "value must be preserved");
        assert!(!world.sensors.mppt_1_operation_mode.is_usable(), "must be Stale");
        assert_eq!(world.sensors.mppt_1_operation_mode.value, Some(1.0), "value must be preserved");

        // The helper must return false — stale sensors are not curtailed
        // (conservative: don't fire probe without live evidence).
        assert!(!mppt_curtailed(&world), "stale cached mode-1 must not trigger probe");
    }

    // ------------------------------------------------------------------
    // D02: multi-tick integration tests for the compensated-drain loop
    // ------------------------------------------------------------------

    /// Seed required sensors for a Zappi-active, battery-draining scenario
    /// at a given `Instant`. All other sensors stay at safe defaults.
    fn seed_zappi_drain_sensors(world: &mut World, at: Instant, battery_drain_w: f64) {
        // Re-feed all required sensors so none go stale.
        let ss = &mut world.sensors;
        ss.battery_soc.on_reading(90.0, at);
        ss.battery_soh.on_reading(95.0, at);
        ss.battery_installed_capacity.on_reading(100.0, at);
        ss.battery_dc_power.on_reading(-battery_drain_w, at); // negative = discharging
        ss.mppt_power_0.on_reading(0.0, at);
        ss.mppt_power_1.on_reading(0.0, at);
        ss.soltaro_power.on_reading(0.0, at);
        ss.power_consumption.on_reading(1200.0, at);
        ss.grid_power.on_reading(0.0, at);
        ss.grid_voltage.on_reading(230.0, at);
        ss.grid_current.on_reading(0.0, at);
        ss.consumption_current.on_reading(5.0, at);
        ss.offgrid_power.on_reading(0.0, at);
        ss.offgrid_current.on_reading(0.0, at);
        ss.vebus_input_current.on_reading(0.0, at);
        ss.evcharger_ac_power.on_reading(0.0, at);
        ss.evcharger_ac_current.on_reading(0.0, at);
        ss.ess_state.on_reading(10.0, at);
        ss.outdoor_temperature.on_reading(15.0, at);
    }

    /// D02: tighten trajectory — Zappi active, battery draining 2 kW (above
    /// threshold=1000 W), kp=1.0. Initial setpoint seeded at -3000 W (exporting).
    ///
    /// Expected per-tick setpoint trajectory (prev → raw new → prepare → actual):
    ///   tick 0: prev=-3000, drain=2000, excess=1000, new=-2000 → prepare: -2000
    ///   tick 1: prev=-2000, drain=2000, excess=1000, new=-1000 → prepare: -1000
    ///   tick 2: prev=-1000, drain=2000, excess=1000, new=0 → prepare promotes to 10
    ///   tick 3: prev=10, drain=2000, excess=1000, new=1010 → prepare promotes to 10
    ///   (stable at 10 W import once setpoint crosses zero)
    #[test]
    fn zappi_active_loop_multi_tick_trajectory() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);

        // Enable writes and set knobs.
        world.knobs.writes_enabled = true;
        world.knobs.allow_battery_to_car = false;
        world.knobs.zappi_battery_drain_threshold_w = 1000;
        world.knobs.zappi_battery_drain_relax_step_w = 100;
        world.knobs.zappi_battery_drain_kp = 1.0;
        world.knobs.grid_import_limit_w = 5000;

        // Seed an initial setpoint of -3000 W so the loop starts from an
        // exporting position. Use Owner::SetpointController to match the live path.
        world.grid_setpoint.propose_target(-3000, Owner::SetpointController, c.monotonic);

        // Zappi charging actively (Eco mode + DivertingOrCharging).
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Eco,
                zappi_plug_state: ZappiPlugState::Charging,
                zappi_status: ZappiStatus::DivertingOrCharging,
                zappi_last_change_signature: c.monotonic,
                session_kwh: 0.0,
            },
            c.monotonic,
        );

        // Tick 0: prev=-3000, drain=2000, excess=1000. new=-2000.
        seed_zappi_drain_sensors(&mut world, c.monotonic, 2000.0);
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let setpoint_0 = world.grid_setpoint.target.value.expect("setpoint set after tick 0");
        assert_eq!(setpoint_0, -2000, "tick 0: expected -2000, got {setpoint_0}");
        let decision = world.decisions.grid_setpoint.as_ref().expect("decision set");
        assert!(
            decision.summary.contains("tightening"),
            "tick 0 decision should say tightening, got: {}",
            decision.summary
        );

        // Tick 1: prev=-2000, drain=2000, excess=1000. new=-1000.
        let t1 = FixedClock::new(c.monotonic + StdDuration::from_secs(5), c.naive);
        seed_zappi_drain_sensors(&mut world, t1.monotonic, 2000.0);
        let _ = process(&Event::Tick { at: t1.monotonic }, &mut world, &t1, &Topology::defaults());
        let setpoint_1 = world.grid_setpoint.target.value.expect("setpoint set after tick 1");
        assert_eq!(setpoint_1, -1000, "tick 1: expected -1000, got {setpoint_1}");

        // Tick 2: prev=-1000, drain=2000, excess=1000. raw new=0 → prepare: 10.
        let t2 = FixedClock::new(c.monotonic + StdDuration::from_secs(10), c.naive);
        seed_zappi_drain_sensors(&mut world, t2.monotonic, 2000.0);
        let _ = process(&Event::Tick { at: t2.monotonic }, &mut world, &t2, &Topology::defaults());
        let setpoint_2 = world.grid_setpoint.target.value.expect("setpoint set after tick 2");
        assert_eq!(setpoint_2, 10, "tick 2: raw 0 promoted to idle 10, got {setpoint_2}");
    }

    /// D02: relax trajectory — Zappi active, battery drain BELOW threshold.
    /// prev starts at -100 (above target=-solar_export=0, since no PV at night).
    /// With relax_step=100, target=0: prev=-100 > target=0? No: -100 < 0.
    /// Wait — target=-solar_export. If mppt=0, target=0. prev=-100 < target=0 →
    /// step UP: (-100+100).min(0) = 0. Reaches target in one step.
    ///
    /// Use a daytime scenario with some PV to make the walk longer:
    /// mppt=2000 → target=-2000. prev=-100: prev > target → step DOWN.
    /// But mppt sensors need to be seeded per-tick too.
    #[test]
    fn zappi_active_relax_walks_toward_minus_solar_export() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);

        world.knobs.writes_enabled = true;
        world.knobs.allow_battery_to_car = false;
        world.knobs.zappi_battery_drain_threshold_w = 1000;
        world.knobs.zappi_battery_drain_relax_step_w = 100;
        world.knobs.zappi_battery_drain_kp = 1.0;
        world.knobs.grid_import_limit_w = 5000;

        // Zappi active.
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Eco,
                zappi_plug_state: ZappiPlugState::Charging,
                zappi_status: ZappiStatus::DivertingOrCharging,
                zappi_last_change_signature: c.monotonic,
                session_kwh: 0.0,
            },
            c.monotonic,
        );

        // Seed: battery drain = 500 W (below threshold=1000), mppt=2000 → solar_export=2000.
        // prev starts at 10 (idle_setpoint_w cold boot).
        // target = -solar_export = -2000. prev=10 > target=-2000 → step DOWN:
        //   (10 - 100).max(-2000) = -90
        // After tick 0: setpoint ≈ -100 (rounded to nearest 50 by prepare_setpoint → -100)
        {
            let ss = &mut world.sensors;
            ss.battery_soc.on_reading(90.0, c.monotonic);
            ss.battery_soh.on_reading(95.0, c.monotonic);
            ss.battery_installed_capacity.on_reading(100.0, c.monotonic);
            ss.battery_dc_power.on_reading(-500.0, c.monotonic);  // 500 W drain (below threshold)
            ss.mppt_power_0.on_reading(2000.0, c.monotonic);
            ss.mppt_power_1.on_reading(0.0, c.monotonic);
            ss.soltaro_power.on_reading(0.0, c.monotonic);
            ss.power_consumption.on_reading(1200.0, c.monotonic);
            ss.grid_power.on_reading(0.0, c.monotonic);
            ss.grid_voltage.on_reading(230.0, c.monotonic);
            ss.grid_current.on_reading(0.0, c.monotonic);
            ss.consumption_current.on_reading(5.0, c.monotonic);
            ss.offgrid_power.on_reading(0.0, c.monotonic);
            ss.offgrid_current.on_reading(0.0, c.monotonic);
            ss.vebus_input_current.on_reading(0.0, c.monotonic);
            ss.evcharger_ac_power.on_reading(0.0, c.monotonic);
            ss.evcharger_ac_current.on_reading(0.0, c.monotonic);
            ss.ess_state.on_reading(10.0, c.monotonic);
            ss.outdoor_temperature.on_reading(15.0, c.monotonic);
        }

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let setpoint_0 = world.grid_setpoint.target.value.expect("setpoint set");
        // prev=10 > target=-2000 → (10-100).max(-2000) = -90 → prepare: -100
        assert_eq!(setpoint_0, -100, "tick 0: prev=10, step DOWN, expected -100, got {setpoint_0}");

        // Confirm relaxing direction.
        let decision = world.decisions.grid_setpoint.as_ref().expect("decision set");
        assert!(
            decision.summary.contains("relaxing"),
            "tick 0 should say relaxing, got: {}",
            decision.summary
        );

        // Tick 1: prev=-100, target=-2000 → (-100-100).max(-2000) = -200 → prepare: -200
        let t1 = FixedClock::new(c.monotonic + StdDuration::from_secs(5), c.naive);
        {
            let ss = &mut world.sensors;
            ss.battery_soc.on_reading(90.0, t1.monotonic);
            ss.battery_soh.on_reading(95.0, t1.monotonic);
            ss.battery_installed_capacity.on_reading(100.0, t1.monotonic);
            ss.battery_dc_power.on_reading(-500.0, t1.monotonic);
            ss.mppt_power_0.on_reading(2000.0, t1.monotonic);
            ss.mppt_power_1.on_reading(0.0, t1.monotonic);
            ss.soltaro_power.on_reading(0.0, t1.monotonic);
            ss.power_consumption.on_reading(1200.0, t1.monotonic);
            ss.grid_power.on_reading(0.0, t1.monotonic);
            ss.grid_voltage.on_reading(230.0, t1.monotonic);
            ss.grid_current.on_reading(0.0, t1.monotonic);
            ss.consumption_current.on_reading(5.0, t1.monotonic);
            ss.offgrid_power.on_reading(0.0, t1.monotonic);
            ss.offgrid_current.on_reading(0.0, t1.monotonic);
            ss.vebus_input_current.on_reading(0.0, t1.monotonic);
            ss.evcharger_ac_power.on_reading(0.0, t1.monotonic);
            ss.evcharger_ac_current.on_reading(0.0, t1.monotonic);
            ss.ess_state.on_reading(10.0, t1.monotonic);
            ss.outdoor_temperature.on_reading(15.0, t1.monotonic);
        }
        let _ = process(&Event::Tick { at: t1.monotonic }, &mut world, &t1, &Topology::defaults());
        let setpoint_1 = world.grid_setpoint.target.value.expect("setpoint set after tick 1");
        assert_eq!(setpoint_1, -200, "tick 1: prev=-100, step DOWN, expected -200, got {setpoint_1}");
    }

    // ------------------------------------------------------------------
    // PR-ZD-4: Fast-mode hard clamp tests (tests 27–33)
    // ------------------------------------------------------------------

    /// Set up world for hard-clamp tests: required sensors seeded, Zappi
    /// actively charging (typed state = Eco/Charging/DivertingOrCharging so
    /// ZappiActiveCore derives `zappi_active = true`), battery draining at
    /// `battery_drain_w`.
    fn seed_hard_clamp_scenario(
        world: &mut World,
        at: Instant,
        battery_drain_w: f64,
    ) {
        world.knobs.writes_enabled = true;
        world.knobs.allow_battery_to_car = false;
        world.knobs.zappi_battery_drain_hard_clamp_w = 200;
        world.knobs.zappi_battery_drain_threshold_w = 1000;
        world.knobs.zappi_battery_drain_relax_step_w = 100;
        world.knobs.zappi_battery_drain_kp = 1.0;
        world.knobs.grid_import_limit_w = 5000;

        let ss = &mut world.sensors;
        ss.battery_soc.on_reading(80.0, at);
        ss.battery_soh.on_reading(95.0, at);
        ss.battery_installed_capacity.on_reading(100.0, at);
        ss.battery_dc_power.on_reading(-battery_drain_w, at); // negative = discharging
        ss.mppt_power_0.on_reading(0.0, at);
        ss.mppt_power_1.on_reading(0.0, at);
        ss.soltaro_power.on_reading(0.0, at);
        ss.power_consumption.on_reading(1200.0, at);
        ss.grid_power.on_reading(0.0, at);
        ss.grid_voltage.on_reading(230.0, at);
        ss.grid_current.on_reading(0.0, at);
        ss.consumption_current.on_reading(5.0, at);
        ss.offgrid_power.on_reading(0.0, at);
        ss.offgrid_current.on_reading(0.0, at);
        ss.vebus_input_current.on_reading(0.0, at);
        ss.evcharger_ac_power.on_reading(0.0, at);
        ss.evcharger_ac_current.on_reading(0.0, at);
        ss.ess_state.on_reading(10.0, at);
        ss.outdoor_temperature.on_reading(15.0, at);

        // Typed Zappi state: Eco + Charging + DivertingOrCharging
        // → ZappiActiveCore derives `zappi_active = true`.
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Eco,
                zappi_plug_state: ZappiPlugState::Charging,
                zappi_status: ZappiStatus::DivertingOrCharging,
                zappi_last_change_signature: at,
                session_kwh: 0.0,
            },
            at,
        );
    }

    /// Decode the `hard_clamp_engaged` decision factor if present.
    fn hard_clamp_engaged_factor(world: &World) -> Option<&str> {
        world
            .decisions
            .grid_setpoint
            .as_ref()
            .and_then(|d| d.factors.iter().find(|f| f.name == "hard_clamp_engaged"))
            .map(|f| f.value.as_str())
    }

    /// Decode the `hard_clamp_excess_W` decision factor if present.
    fn hard_clamp_excess_factor(world: &World) -> Option<&str> {
        world
            .decisions
            .grid_setpoint
            .as_ref()
            .and_then(|d| d.factors.iter().find(|f| f.name == "hard_clamp_excess_W"))
            .map(|f| f.value.as_str())
    }

    /// Test 27: Fast target + !allow + zappi_active + drain(500) > hard_clamp(200)
    /// → clamp fires, setpoint raised by 300 W vs evaluate_setpoint output.
    ///
    /// Setup: prev=-3000 (seeded). Soft loop: drain=500 < threshold=1000 →
    /// relax: prev=-3000 < target=0 → (-3000+100).min(0) = -2900.
    /// prepare_setpoint rounds: -2900/50=-58 → -2900. Output: -2900.
    /// Hard clamp: excess=500-200=300, raised=-2900+300=-2600.
    /// Grid cap (import=5000): -2600 within [-5000, 5000] → -2600.
    #[test]
    fn hard_clamp_engages_in_fast_mode_when_drain_exceeds_threshold() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 500.0);

        // Target = Fast (predictive arming — clamp reads target, not actual).
        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);
        // Seed initial setpoint for the soft loop's recurrence base.
        world.grid_setpoint.propose_target(-3000, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let setpoint = world.grid_setpoint.target.value.expect("setpoint set");

        // Soft loop: drain=500 < threshold=1000, relax: -3000 < 0 →
        // (-3000+100).min(0)=-2900; prepare: -2900.
        // Hard clamp: excess=300, raised=-2900+300=-2600.
        assert_eq!(
            setpoint, -2600,
            "hard clamp should raise setpoint by 300 W: expected -2600, got {setpoint}"
        );
        assert_eq!(
            hard_clamp_engaged_factor(&world),
            Some("true"),
            "hard_clamp_engaged should be 'true' in decision factors"
        );
        assert_eq!(
            hard_clamp_excess_factor(&world),
            Some("300"),
            "hard_clamp_excess_W should be '300' in decision factors"
        );
    }

    /// Test 28: Eco target — hard clamp must NOT fire regardless of drain.
    ///
    /// zappi_active=true, drain=500 > hard_clamp=200, BUT target=Eco → bypass.
    /// Soft loop fires (relax branch from -3000).
    #[test]
    fn hard_clamp_disengaged_in_eco_mode() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 500.0);

        // Target = Eco (not Fast) → hard clamp does not arm.
        world.zappi_mode.propose_target(ZappiMode::Eco, Owner::SetpointController, c.monotonic);
        world.grid_setpoint.propose_target(-3000, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let setpoint = world.grid_setpoint.target.value.expect("setpoint set");

        // Soft loop relax only (no hard clamp): -2900.
        assert_eq!(
            setpoint, -2900,
            "Eco mode: no hard clamp, soft relax only; expected -2900, got {setpoint}"
        );
        assert!(
            hard_clamp_engaged_factor(&world).is_none(),
            "hard_clamp_engaged must not appear in decision factors for Eco mode"
        );
    }

    /// Test 29: Off target — hard clamp must NOT fire.
    #[test]
    fn hard_clamp_disengaged_in_off_mode() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 500.0);

        // Target = Off → hard clamp does not arm.
        world.zappi_mode.propose_target(ZappiMode::Off, Owner::SetpointController, c.monotonic);
        world.grid_setpoint.propose_target(-3000, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        assert!(
            hard_clamp_engaged_factor(&world).is_none(),
            "hard_clamp_engaged must not appear in decision factors for Off target"
        );
    }

    /// Test 30: Fast target but allow_battery_to_car=true — hard clamp must NOT fire.
    #[test]
    fn hard_clamp_disengaged_when_allow_battery_to_car_true() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 500.0);

        // Operator opted in.
        world.knobs.allow_battery_to_car = true;
        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);
        world.grid_setpoint.propose_target(-3000, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        assert!(
            hard_clamp_engaged_factor(&world).is_none(),
            "hard_clamp_engaged must not appear when allow_battery_to_car=true"
        );
    }

    /// Test 31: Fast target, drain=100 < hard_clamp=200 — clamp does NOT fire.
    #[test]
    fn hard_clamp_disengaged_when_drain_below_clamp_threshold() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // drain=100, well below hard_clamp=200
        seed_hard_clamp_scenario(&mut world, c.monotonic, 100.0);

        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);
        world.grid_setpoint.propose_target(-3000, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        assert!(
            hard_clamp_engaged_factor(&world).is_none(),
            "hard_clamp_engaged must not appear when drain(100) < hard_clamp(200)"
        );
    }

    /// Test 32: Fast + drain=2000 + threshold=1000 + kp=1.0 + hard_clamp=200.
    ///
    /// Soft loop: drain=2000 > threshold=1000, excess=1000.
    ///   prev=-3000, new=-3000+1.0*1000=-2000.
    ///   prepare_setpoint: -2000/50=-40 → -2000. Output=-2000.
    /// Hard clamp: excess=2000-200=1800, raised=-2000+1800=-200.
    /// Grid cap (5000): -200 within bounds → -200.
    /// Combined raise relative to prev: +2800 W. Both clamps engaged.
    #[test]
    fn hard_clamp_combines_with_soft_loop() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 2000.0);

        world.knobs.zappi_battery_drain_threshold_w = 1000;
        world.knobs.zappi_battery_drain_kp = 1.0;
        world.knobs.zappi_battery_drain_hard_clamp_w = 200;

        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);
        world.grid_setpoint.propose_target(-3000, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let setpoint = world.grid_setpoint.target.value.expect("setpoint set");

        // Soft: -3000 + 1000 = -2000. Hard: -2000 + 1800 = -200.
        assert_eq!(
            setpoint, -200,
            "soft + hard combined raise of +2800: expected -200, got {setpoint}"
        );
        assert_eq!(
            hard_clamp_engaged_factor(&world),
            Some("true"),
            "hard_clamp_engaged should be 'true'"
        );
        assert_eq!(
            hard_clamp_excess_factor(&world),
            Some("1800"),
            "hard_clamp_excess_W should be '1800'"
        );
    }

    /// Test 33: Fast + drain=20000 + hard_clamp=200 + grid_import_limit_w=5000.
    ///
    /// Soft loop: drain=20000 > threshold=1000, excess=19000.
    ///   prev=10 (cold boot), new=10+1.0*19000=19010.
    ///   prepare_setpoint: >= 0 → returns idle (10).
    /// Hard clamp: excess=20000-200=19800, raised=10+19800=19810.
    /// Grid cap (import=5000): 19810.clamp(-5000, 5000)=5000.
    /// Final setpoint: 5000. Decision has hard_clamp_engaged + grid_cap factors.
    #[test]
    fn hard_clamp_respects_grid_import_cap() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 20000.0);

        world.knobs.zappi_battery_drain_hard_clamp_w = 200;
        world.knobs.zappi_battery_drain_threshold_w = 1000;
        world.knobs.zappi_battery_drain_kp = 1.0;
        world.knobs.grid_import_limit_w = 5000;

        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);
        // Do NOT seed a prior setpoint — starts from cold-boot idle (10 W).

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let setpoint = world.grid_setpoint.target.value.expect("setpoint set");

        // Grid cap clips at +5000.
        assert_eq!(
            setpoint, 5000,
            "grid_import_cap should clip to 5000: got {setpoint}"
        );
        assert_eq!(
            hard_clamp_engaged_factor(&world),
            Some("true"),
            "hard_clamp_engaged should be 'true'"
        );
        // Verify grid-cap factors also present.
        let decision = world.decisions.grid_setpoint.as_ref().expect("decision present");
        assert!(
            decision.factors.iter().any(|f| f.name == "grid_cap_pre_W"),
            "grid_cap_pre_W should be present when grid cap fired: {decision:#?}"
        );
    }

    /// Test 34: Fast target + !allow + drain(500) > hard_clamp(200), BUT
    /// `zappi_active = false` (EV disconnected) → hard clamp must NOT fire.
    ///
    /// This covers the `world.derived.zappi_active` branch of the gate:
    ///   `target=Fast && !allow && zappi_active && drain > hard_clamp`
    /// Setting `ZappiPlugState::EvDisconnected` causes `classify_zappi_active`
    /// to return `false`, so `world.derived.zappi_active` is written as `false`
    /// on the tick, and the clamp condition short-circuits.
    ///
    /// When `zappi_active=false`, `evaluate_setpoint` also bypasses the
    /// Zappi drain-control branch, so the setpoint falls through to idle (10 W).
    /// The battery-drain reading still exceeds the hard-clamp threshold, but the
    /// gate rejects it because the Zappi is not the source of the draw.
    #[test]
    fn hard_clamp_disengaged_when_zappi_active_false() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 500.0);

        // Override the typed Zappi state: EV disconnected → zappi_active = false.
        // `classify_zappi_active` returns false immediately on EvDisconnected
        // (crates/core/src/controllers/zappi_active.rs line ~57-61).
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Fast,
                zappi_plug_state: ZappiPlugState::EvDisconnected,
                zappi_status: ZappiStatus::DivertingOrCharging,
                zappi_last_change_signature: c.monotonic,
                session_kwh: 0.0,
            },
            c.monotonic,
        );
        // evcharger_ac_power was seeded at 0.0 W by seed_hard_clamp_scenario —
        // well below the 500 W power-based fallback, so the power path also
        // classifies as inactive.

        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);
        world.grid_setpoint.propose_target(-3000, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let setpoint = world.grid_setpoint.target.value.expect("setpoint set");

        // zappi_active=false → evaluate_setpoint falls through to idle (10 W).
        // Hard clamp also does not fire (zappi_active gate is false).
        assert_eq!(
            setpoint, 10,
            "zappi_active=false: no zappi branch, idle setpoint; expected 10, got {setpoint}"
        );
        assert!(
            hard_clamp_engaged_factor(&world).is_none(),
            "hard_clamp_engaged must not appear in decision factors when zappi_active=false"
        );
    }

    // ------------------------------------------------------------------
    // PR-ZDO-1: Compensated-drain observability capture tests
    // ------------------------------------------------------------------

    /// PR-ZDO-1.T1: Fast + !allow + battery=-2500 + HP=0 + cooker=0 +
    /// threshold=1000. Run setpoint. Assert captured `compensated_drain_w == 2500.0`.
    #[test]
    fn zappi_drain_capture_records_compensated_w_matching_controller() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // battery_drain_w=2500 → battery_dc_power=-2500.
        // compensated_drain = max(0, 2500 - 0 - 0) = 2500.
        seed_hard_clamp_scenario(&mut world, c.monotonic, 2500.0);
        world.knobs.zappi_battery_drain_threshold_w = 1000;
        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let snap = world
            .zappi_drain_state
            .latest
            .expect("snapshot captured after run_setpoint");
        assert_eq!(
            snap.compensated_drain_w, 2500.0,
            "compensated_drain_w must match battery_dc_power magnitude"
        );
    }

    /// PR-ZDO-1.T2: Push 130 synthetic snapshots via `push()`; assert
    /// `samples.len() == 120` and the oldest 10 are evicted. Each push
    /// is spaced `SAMPLE_INTERVAL_MS + 1` apart so the time gate passes
    /// for every snapshot.
    #[test]
    fn zappi_drain_capture_ring_buffer_evicts_at_120() {
        use crate::world::{ZappiDrainSnapshot, ZappiDrainState};
        let mut state = crate::world::ZappiDrainState::default();
        let base_ms: i64 = 1_777_503_600_000;

        for i in 0_i32..130 {
            state.push(ZappiDrainSnapshot {
                compensated_drain_w: 0.0,
                branch: ZappiDrainBranch::Relax,
                hard_clamp_engaged: false,
                hard_clamp_excess_w: 0.0,
                threshold_w: 1000,
                hard_clamp_w: 200,
                captured_at_ms: base_ms + i64::from(i) * (ZappiDrainState::SAMPLE_INTERVAL_MS + 1),
            });
        }

        assert_eq!(
            state.samples.len(),
            120,
            "ring buffer must cap at RING_CAPACITY=120"
        );
        // Samples 0..=9 were evicted; oldest remaining is sample 10.
        let expected_oldest = base_ms + 10 * (ZappiDrainState::SAMPLE_INTERVAL_MS + 1);
        assert_eq!(
            state.samples.front().unwrap().captured_at_ms,
            expected_oldest,
            "oldest retained sample must be index 10 (samples 0..=9 evicted)"
        );
    }

    /// PR-ZDO-1.T2b: Time-gate: pushes closer than `SAMPLE_INTERVAL_MS` apart
    /// must update `latest` but not grow `samples`.
    #[test]
    fn zappi_drain_capture_buffer_time_gated_to_15s_intervals() {
        use crate::world::{ZappiDrainSnapshot, ZappiDrainState};
        let mut state = crate::world::ZappiDrainState::default();
        let base_ms: i64 = 1_777_503_600_000;

        let snap_at = |ms: i64, drain: f64| ZappiDrainSnapshot {
            compensated_drain_w: drain,
            branch: ZappiDrainBranch::Relax,
            hard_clamp_engaged: false,
            hard_clamp_excess_w: 0.0,
            threshold_w: 1000,
            hard_clamp_w: 200,
            captured_at_ms: ms,
        };

        // First push always appends; latest reflects the pushed drain.
        state.push(snap_at(base_ms, 100.0));
        assert_eq!(state.samples.len(), 1);
        assert!((state.latest.unwrap().compensated_drain_w - 100.0).abs() < f64::EPSILON);
        // Same-ms push: latest updates to 200, samples does not grow.
        state.push(snap_at(base_ms, 200.0));
        assert_eq!(state.samples.len(), 1);
        assert!((state.latest.unwrap().compensated_drain_w - 200.0).abs() < f64::EPSILON);
        // 14_999 ms later: still gated; latest updates to 300.
        state.push(snap_at(base_ms + 14_999, 300.0));
        assert_eq!(state.samples.len(), 1);
        assert!((state.latest.unwrap().compensated_drain_w - 300.0).abs() < f64::EPSILON);
        // Exactly 15_000 ms after the last sample: appends; latest updates to 400.
        state.push(snap_at(base_ms + ZappiDrainState::SAMPLE_INTERVAL_MS, 400.0));
        assert_eq!(state.samples.len(), 2);
        assert!((state.latest.unwrap().compensated_drain_w - 400.0).abs() < f64::EPSILON);
    }

    /// PR-ZDO-1.T3: Table-driven branch classification — 5 sub-scenarios
    /// confirming the classifier mirrors evaluate_setpoint's branch ladder.
    #[test]
    fn branch_classification_matches_evaluate_setpoint_branch_ladder() {
        struct Case {
            label: &'static str,
            force_disable_export: bool,
            zappi_active: bool,
            allow_battery_to_car: bool,
            battery_drain_w: f64, // positive = discharging
            threshold_w: u32,
            expected_branch: ZappiDrainBranch,
        }
        let cases = [
            Case {
                label: "Tighten: drain>threshold, zappi_active=true, !allow",
                force_disable_export: false,
                zappi_active: true,
                allow_battery_to_car: false,
                battery_drain_w: 2500.0,
                threshold_w: 1000,
                expected_branch: ZappiDrainBranch::Tighten,
            },
            Case {
                label: "Relax: drain<=threshold, zappi_active=true, !allow",
                force_disable_export: false,
                zappi_active: true,
                allow_battery_to_car: false,
                battery_drain_w: 500.0,
                threshold_w: 1000,
                expected_branch: ZappiDrainBranch::Relax,
            },
            Case {
                label: "Bypass-via-allow: allow_battery_to_car=true",
                force_disable_export: false,
                zappi_active: true,
                allow_battery_to_car: true,
                battery_drain_w: 2500.0,
                threshold_w: 1000,
                expected_branch: ZappiDrainBranch::Bypass,
            },
            Case {
                label: "Bypass-via-force: force_disable_export=true",
                force_disable_export: true,
                zappi_active: true,
                allow_battery_to_car: false,
                battery_drain_w: 2500.0,
                threshold_w: 1000,
                expected_branch: ZappiDrainBranch::Bypass,
            },
            Case {
                label: "Disabled: zappi_active=false",
                force_disable_export: false,
                zappi_active: false,
                allow_battery_to_car: false,
                battery_drain_w: 2500.0,
                threshold_w: 1000,
                expected_branch: ZappiDrainBranch::Disabled,
            },
        ];

        for case in &cases {
            let c = clock_at(12, 0);
            let mut world = World::fresh_boot(c.monotonic);
            seed_hard_clamp_scenario(&mut world, c.monotonic, case.battery_drain_w);
            world.knobs.zappi_battery_drain_threshold_w = case.threshold_w;
            world.knobs.force_disable_export = case.force_disable_export;
            world.knobs.allow_battery_to_car = case.allow_battery_to_car;
            // ZappiActiveCore runs each tick and writes derived.zappi_active;
            // override explicitly after the tick to test the classifier.
            // We run the tick first to populate the snapshot, then read it.
            if !case.zappi_active {
                // Override typed sensor to produce zappi_active=false.
                world.typed_sensors.zappi_state.on_reading(
                    ZappiState {
                        zappi_mode: ZappiMode::Fast,
                        zappi_plug_state: ZappiPlugState::EvDisconnected,
                        zappi_status: ZappiStatus::Paused,
                        zappi_last_change_signature: c.monotonic,
                        session_kwh: 0.0,
                    },
                    c.monotonic,
                );
            }
            world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);

            let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

            let snap = world
                .zappi_drain_state
                .latest
                .unwrap_or_else(|| panic!("snapshot missing for case: {}", case.label));
            assert_eq!(
                snap.branch,
                case.expected_branch,
                "case '{}': expected {:?}, got {:?}",
                case.label,
                case.expected_branch,
                snap.branch
            );
        }
    }

    /// PR-ZDO-1.T4: Observer mode (`writes_enabled=false`) does NOT
    /// short-circuit capture. Snapshot still records Tighten + drain=2500.
    #[test]
    fn zappi_drain_capture_honest_under_observer_mode() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 2500.0);
        world.knobs.zappi_battery_drain_threshold_w = 1000;
        // Observer mode.
        world.knobs.writes_enabled = false;
        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let snap = world
            .zappi_drain_state
            .latest
            .expect("snapshot captured even in observer mode");
        assert_eq!(
            snap.branch,
            ZappiDrainBranch::Tighten,
            "observer mode must not suppress capture; expected Tighten"
        );
        assert_eq!(
            snap.compensated_drain_w, 2500.0,
            "observer mode: compensated_drain_w must equal 2500.0"
        );
    }

    /// PR-ZDO-1.T5: Fast + !allow + drain=500 + hard_clamp=200 + threshold=100.
    /// drain(500) > hard_clamp(200) → hard_clamp_engaged=true, excess=300.
    /// drain(500) > threshold(100) → branch=Tighten.
    #[test]
    fn zappi_drain_capture_lockstep_with_hard_clamp_engagement() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 500.0);
        // Override threshold and hard_clamp so drain(500) > threshold(100)
        // and drain(500) > hard_clamp(200) both hold.
        world.knobs.zappi_battery_drain_threshold_w = 100;
        world.knobs.zappi_battery_drain_hard_clamp_w = 200;
        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        let snap = world
            .zappi_drain_state
            .latest
            .expect("snapshot must be captured");
        assert!(
            snap.hard_clamp_engaged,
            "hard_clamp_engaged must be true when drain(500) > hard_clamp(200)"
        );
        assert!(
            (snap.hard_clamp_excess_w - 300.0).abs() < 1e-6,
            "hard_clamp_excess_w must be 300.0 (500-200), got {}",
            snap.hard_clamp_excess_w
        );
        assert_eq!(
            snap.branch,
            ZappiDrainBranch::Tighten,
            "drain(500) > threshold(100) → Tighten"
        );
    }

    /// PR-ZDO-1.T6: Mutating `zappi_drain_state` after the first tick must
    /// NOT affect the setpoint computed on the second tick. The ring buffer
    /// is read-only from the controller's perspective.
    #[test]
    fn zappi_drain_capture_does_not_feed_back_into_setpoint() {
        use crate::world::ZappiDrainSnapshot;
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 2500.0);
        world.knobs.zappi_battery_drain_threshold_w = 1000;
        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);

        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let setpoint_1 = world.grid_setpoint.target.value.expect("setpoint after tick 1");

        // Poison the observability state with garbage values.
        world.zappi_drain_state.latest = None;
        world.zappi_drain_state.push(ZappiDrainSnapshot {
            compensated_drain_w: 99_999.0,
            branch: ZappiDrainBranch::Relax,
            hard_clamp_engaged: false,
            hard_clamp_excess_w: 0.0,
            threshold_w: 0,
            hard_clamp_w: 0,
            captured_at_ms: 0,
        });

        // Run again with identical inputs.
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        let setpoint_2 = world.grid_setpoint.target.value.expect("setpoint after tick 2");

        assert_eq!(
            setpoint_1, setpoint_2,
            "zappi_drain_state mutation must not affect setpoint (no feedback invariant)"
        );
    }

    /// PR-ZDO-1.T7: Push 50 samples (spaced `SAMPLE_INTERVAL_MS + 1` apart),
    /// then call `World::fresh_boot`; the new world must have
    /// `samples.len() == 0` and `latest == None`.
    #[test]
    fn zappi_drain_capture_buffer_resets_on_fresh_boot() {
        use crate::world::{ZappiDrainSnapshot, ZappiDrainState};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        let base_ms: i64 = 1_777_503_600_000;

        for i in 0..50_i64 {
            world.zappi_drain_state.push(ZappiDrainSnapshot {
                compensated_drain_w: i as f64,
                branch: ZappiDrainBranch::Relax,
                hard_clamp_engaged: false,
                hard_clamp_excess_w: 0.0,
                threshold_w: 1000,
                hard_clamp_w: 200,
                captured_at_ms: base_ms + i * (ZappiDrainState::SAMPLE_INTERVAL_MS + 1),
            });
        }
        assert_eq!(world.zappi_drain_state.samples.len(), 50);

        let new_world = World::fresh_boot(c.monotonic);

        assert_eq!(
            new_world.zappi_drain_state.samples.len(),
            0,
            "fresh_boot must reset ring buffer to empty"
        );
        assert!(
            new_world.zappi_drain_state.latest.is_none(),
            "fresh_boot must reset latest to None"
        );
    }

    // =========================================================================
    // PR-ZDO-2: HA broadcast sensor tests for controller-derived observables.
    // =========================================================================

    /// Run `SensorBroadcastCore` directly on a pre-seeded world and return
    /// the `Effect::Publish` variants emitted (all other effects dropped).
    fn run_sensor_broadcast(world: &mut World, c: &FixedClock) -> Vec<Effect> {
        use crate::core_dag::cores::SensorBroadcastCore;
        use crate::core_dag::Core;
        let mut effects: Vec<Effect> = Vec::new();
        SensorBroadcastCore.run(world, c, &Topology::defaults(), &mut effects);
        effects
    }

    /// Filter only `Effect::Publish(PublishPayload::ControllerNumeric{..})`
    /// and `Effect::Publish(PublishPayload::ControllerBool{..})` from a vec.
    fn controller_observable_effects(effects: &[Effect]) -> Vec<&Effect> {
        effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::Publish(
                        PublishPayload::ControllerNumeric { .. }
                            | PublishPayload::ControllerBool { .. }
                    )
                )
            })
            .collect()
    }

    /// PR-ZDO-2.T1: fresh World, run_setpoint once with Tighten scenario, then
    /// run SensorBroadcastCore. Assert exactly 3 controller-observable publishes,
    /// with tighten=true, hard-clamp=false, compensated-w=2500.
    #[test]
    fn controller_observables_publish_on_first_tick() {
        use crate::types::{ControllerObservableId, PublishPayload};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // Tighten scenario: battery drain = 2500 W, threshold = 1000 W.
        seed_hard_clamp_scenario(&mut world, c.monotonic, 2500.0);
        world.knobs.zappi_battery_drain_threshold_w = 1000;
        world.knobs.zappi_battery_drain_hard_clamp_w = 5000; // no hard clamp
        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);

        // Run the full tick so zappi_drain_state.latest is populated.
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());

        // Verify snapshot was captured.
        let snap = world.zappi_drain_state.latest.expect("snapshot must be present");
        assert_eq!(snap.branch, ZappiDrainBranch::Tighten);

        // Now run broadcast in isolation (cache is fresh after process call).
        // Clear cache to force publish (simulate first-ever tick).
        world.published_cache.controller_numeric.clear();
        world.published_cache.controller_bool.clear();

        let effects = run_sensor_broadcast(&mut world, &c);
        let obs = controller_observable_effects(&effects);
        assert_eq!(obs.len(), 3, "expected 3 controller observable publishes, got {}", obs.len());

        // Find each by id.
        let find_numeric = |id: ControllerObservableId| {
            effects.iter().find_map(|e| {
                if let Effect::Publish(PublishPayload::ControllerNumeric {
                    id: eid,
                    value,
                    freshness,
                }) = e
                {
                    if *eid == id { Some((*value, *freshness)) } else { None }
                } else {
                    None
                }
            })
        };
        let find_bool = |id: ControllerObservableId| {
            effects.iter().find_map(|e| {
                if let Effect::Publish(PublishPayload::ControllerBool { id: eid, value }) = e {
                    if *eid == id { Some(*value) } else { None }
                } else {
                    None
                }
            })
        };

        let (drain_val, drain_fresh) =
            find_numeric(ControllerObservableId::ZappiDrainCompensatedW)
                .expect("compensated-w must be published");
        assert!(
            (drain_val - 2500.0).abs() < 1.0,
            "compensated-w: expected ~2500.0, got {drain_val}"
        );
        assert_eq!(drain_fresh, Freshness::Fresh);

        let tighten =
            find_bool(ControllerObservableId::ZappiDrainTightenActive)
                .expect("tighten-active must be published");
        assert!(tighten, "tighten-active must be true for Tighten branch");

        let clamp =
            find_bool(ControllerObservableId::ZappiDrainHardClampActive)
                .expect("hard-clamp-active must be published");
        assert!(!clamp, "hard-clamp-active must be false when hard_clamp_w=5000 >> drain");
    }

    /// PR-ZDO-2.T2: run twice with identical inputs — second run emits 0
    /// controller observable publishes (dedup on wire body).
    #[test]
    fn controller_observables_dedup_on_unchanged_state() {
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        seed_hard_clamp_scenario(&mut world, c.monotonic, 2500.0);
        world.knobs.zappi_battery_drain_threshold_w = 1000;
        world.knobs.zappi_battery_drain_hard_clamp_w = 5000;
        world.zappi_mode.propose_target(ZappiMode::Fast, Owner::SetpointController, c.monotonic);

        // First tick + broadcast.
        let _ = process(&Event::Tick { at: c.monotonic }, &mut world, &c, &Topology::defaults());
        world.published_cache.controller_numeric.clear();
        world.published_cache.controller_bool.clear();
        let effects1 = run_sensor_broadcast(&mut world, &c);
        let obs1 = controller_observable_effects(&effects1);
        assert_eq!(obs1.len(), 3, "first run: expected 3 publishes");

        // Second run — cache is now populated; same snapshot. Zero new publishes.
        let effects2 = run_sensor_broadcast(&mut world, &c);
        let obs2 = controller_observable_effects(&effects2);
        assert_eq!(obs2.len(), 0, "second run with same state: expected 0 publishes (dedup)");
    }

    /// PR-ZDO-2.T3: fresh World with no run_setpoint call. Broadcast publishes
    /// compensated-w as "unavailable"; both booleans publish false.
    #[test]
    fn controller_observables_compensated_w_unavailable_when_no_capture() {
        use crate::types::{ControllerObservableId, PublishPayload, encode_sensor_body};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);

        assert!(world.zappi_drain_state.latest.is_none(), "precondition: no snapshot");

        let effects = run_sensor_broadcast(&mut world, &c);
        let obs = controller_observable_effects(&effects);
        // All three must be published (first tick, cache empty).
        assert_eq!(obs.len(), 3, "expected 3 publishes on fresh world, got {}", obs.len());

        // compensated-w must encode as "unavailable".
        let drain_effect = effects.iter().find(|e| {
            matches!(
                e,
                Effect::Publish(PublishPayload::ControllerNumeric {
                    id: ControllerObservableId::ZappiDrainCompensatedW,
                    ..
                })
            )
        });
        if let Some(Effect::Publish(PublishPayload::ControllerNumeric {
            value,
            freshness,
            ..
        })) = drain_effect
        {
            let body = encode_sensor_body(Some(*value), *freshness);
            assert_eq!(body, "unavailable", "no-snapshot compensated-w must encode as 'unavailable'");
        } else {
            panic!("compensated-w publish not found in effects");
        }

        // Both booleans must be false.
        for id in [
            ControllerObservableId::ZappiDrainTightenActive,
            ControllerObservableId::ZappiDrainHardClampActive,
        ] {
            let val = effects.iter().find_map(|e| {
                if let Effect::Publish(PublishPayload::ControllerBool { id: eid, value }) = e {
                    if *eid == id { Some(*value) } else { None }
                } else {
                    None
                }
            });
            assert_eq!(
                val,
                Some(false),
                "no-snapshot {id:?} must publish false"
            );
        }
    }

    /// PR-ZDO-2.D05: Disabled-branch snapshot must publish "unavailable" for
    /// compensated-w and "false" for both booleans — mirrors T3 but with
    /// `latest = Some(snap with branch=Disabled)` instead of `latest = None`.
    #[test]
    fn controller_observables_disabled_branch_yields_unavailable_and_false_bools() {
        use crate::types::{ControllerObservableId, PublishPayload, ZappiDrainBranch, encode_sensor_body};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);

        // Seed a snapshot with branch=Disabled (apply_setpoint_safety fallback shape).
        world.zappi_drain_state.latest = Some(crate::world::ZappiDrainSnapshot {
            compensated_drain_w: 0.0,
            branch: ZappiDrainBranch::Disabled,
            hard_clamp_engaged: false,
            hard_clamp_excess_w: 0.0,
            threshold_w: 1000,
            hard_clamp_w: 200,
            captured_at_ms: 0,
        });

        let effects = run_sensor_broadcast(&mut world, &c);
        let obs = controller_observable_effects(&effects);
        assert_eq!(obs.len(), 3, "expected 3 publishes on fresh world, got {}", obs.len());

        // compensated-w must encode as "unavailable" (Disabled branch → Stale).
        let drain_effect = effects.iter().find(|e| {
            matches!(
                e,
                Effect::Publish(PublishPayload::ControllerNumeric {
                    id: ControllerObservableId::ZappiDrainCompensatedW,
                    ..
                })
            )
        });
        if let Some(Effect::Publish(PublishPayload::ControllerNumeric {
            value,
            freshness,
            ..
        })) = drain_effect
        {
            let body = encode_sensor_body(Some(*value), *freshness);
            assert_eq!(
                body, "unavailable",
                "Disabled-branch compensated-w must encode as 'unavailable', not fake zero"
            );
        } else {
            panic!("compensated-w publish not found in effects");
        }

        // Both booleans must be false (Disabled is neither Tighten nor HardClamp).
        for id in [
            ControllerObservableId::ZappiDrainTightenActive,
            ControllerObservableId::ZappiDrainHardClampActive,
        ] {
            let val = effects.iter().find_map(|e| {
                if let Effect::Publish(PublishPayload::ControllerBool { id: eid, value }) = e {
                    if *eid == id { Some(*value) } else { None }
                } else {
                    None
                }
            });
            assert_eq!(val, Some(false), "Disabled-branch {id:?} must publish false");
        }
    }

    /// PR-ZDO-2.T4: round-trip each ControllerObservableId through
    /// encode_publish_payload (tested in serialize.rs). This test validates
    /// the core-side invariant: the three dedup bodies stored in the cache
    /// match what encode_sensor_body / bool formatting would produce.
    #[test]
    fn controller_observables_cache_body_matches_encode_sensor_body() {
        use crate::types::{ControllerObservableId, encode_sensor_body};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);
        // Fresh snapshot: drain=1500, Tighten, no hard clamp.
        world.zappi_drain_state.latest = Some(crate::world::ZappiDrainSnapshot {
            compensated_drain_w: 1500.0,
            branch: ZappiDrainBranch::Tighten,
            hard_clamp_engaged: false,
            hard_clamp_excess_w: 0.0,
            threshold_w: 1000,
            hard_clamp_w: 200,
            captured_at_ms: 1_777_503_600_000,
        });

        let _ = run_sensor_broadcast(&mut world, &c);

        // Cache must store the encoded wire body for the numeric.
        let cached_drain = world
            .published_cache
            .controller_numeric
            .get(&ControllerObservableId::ZappiDrainCompensatedW)
            .expect("cache must contain compensated-w after broadcast");
        let expected_drain = encode_sensor_body(Some(1500.0), Freshness::Fresh);
        assert_eq!(cached_drain, &expected_drain, "cache body mismatch for compensated-w");

        // Cache must store the booleans.
        let cached_tighten = world
            .published_cache
            .controller_bool
            .get(&ControllerObservableId::ZappiDrainTightenActive)
            .copied()
            .expect("cache must contain tighten-active");
        assert!(cached_tighten, "Tighten branch → tighten-active=true in cache");

        let cached_clamp = world
            .published_cache
            .controller_bool
            .get(&ControllerObservableId::ZappiDrainHardClampActive)
            .copied()
            .expect("cache must contain hard-clamp-active");
        assert!(!cached_clamp, "hard_clamp_engaged=false → hard-clamp-active=false in cache");
    }

    /// PR-ZDO-2.T5: freshness-aware publish for compensated-w.
    /// Snapshot drain=1500 → publishes "1500". Clear snapshot → re-broadcast
    /// produces "unavailable" (cache invalidates because wire body changed).
    #[test]
    fn freshness_aware_publish_for_compensated_w() {
        use crate::types::{ControllerObservableId, PublishPayload, encode_sensor_body};
        let c = clock_at(12, 0);
        let mut world = World::fresh_boot(c.monotonic);

        // Seed snapshot.
        world.zappi_drain_state.latest = Some(crate::world::ZappiDrainSnapshot {
            compensated_drain_w: 1500.0,
            branch: ZappiDrainBranch::Relax,
            hard_clamp_engaged: false,
            hard_clamp_excess_w: 0.0,
            threshold_w: 1000,
            hard_clamp_w: 200,
            captured_at_ms: 1_777_503_600_000,
        });
        let effects1 = run_sensor_broadcast(&mut world, &c);
        let drain_body1 = effects1.iter().find_map(|e| {
            if let Effect::Publish(PublishPayload::ControllerNumeric {
                id: ControllerObservableId::ZappiDrainCompensatedW,
                value,
                freshness,
            }) = e
            {
                Some(encode_sensor_body(Some(*value), *freshness))
            } else {
                None
            }
        });
        assert_eq!(
            drain_body1.as_deref(),
            Some("1500"),
            "first broadcast: expected '1500', got {drain_body1:?}"
        );

        // Clear snapshot → unavailable.
        world.zappi_drain_state.latest = None;
        let effects2 = run_sensor_broadcast(&mut world, &c);
        let drain_body2 = effects2.iter().find_map(|e| {
            if let Effect::Publish(PublishPayload::ControllerNumeric {
                id: ControllerObservableId::ZappiDrainCompensatedW,
                value,
                freshness,
            }) = e
            {
                Some(encode_sensor_body(Some(*value), *freshness))
            } else {
                None
            }
        });
        assert_eq!(
            drain_body2.as_deref(),
            Some("unavailable"),
            "after snapshot cleared: expected 'unavailable', got {drain_body2:?}"
        );
    }
}
