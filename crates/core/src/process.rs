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
use std::time::Instant;


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
    ActuatedId, BookkeepingKey, BookkeepingValue, Command, DbusTarget,
    DbusValue, Decision, Effect, Event, ForecastProvider, KnobId, KnobValue, LogLevel,
    MyenergiAction, PinnedStatus, PinnedValue, PublishPayload, ScheduleField, SensorId,
    SensorReading, TimerId, TimerStatus, TypedReading,
};
use crate::world::TimerEntry;
use crate::world::{ForecastSnapshot, World};

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
        TypedReading::Zappi { state, at } => {
            world.typed_sensors.zappi_state.on_reading(state, at);
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
        TypedReading::Eddi { mode, at } => {
            world.typed_sensors.eddi_mode.on_reading(mode, at);
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
    vec![
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
    ]
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
#[must_use]
pub(crate) fn build_setpoint_input(world: &World) -> Option<SetpointInput> {
    if !world.sensors.battery_soc.is_usable()
        || !world.sensors.battery_soh.is_usable()
        || !world.sensors.battery_installed_capacity.is_usable()
        || !world.sensors.mppt_power_0.is_usable()
        || !world.sensors.mppt_power_1.is_usable()
        || !world.sensors.soltaro_power.is_usable()
        || !world.sensors.power_consumption.is_usable()
        || !world.sensors.evcharger_ac_power.is_usable()
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
        },
        power_consumption: world.sensors.power_consumption.value.unwrap(),
        battery_soc: world.sensors.battery_soc.value.unwrap(),
        soh: world.sensors.battery_soh.value.unwrap(),
        mppt_power_0: world.sensors.mppt_power_0.value.unwrap(),
        mppt_power_1: world.sensors.mppt_power_1.value.unwrap(),
        soltaro_power: world.sensors.soltaro_power.value.unwrap(),
        evcharger_ac_power: world.sensors.evcharger_ac_power.value.unwrap(),
        capacity: world.sensors.battery_installed_capacity.value.unwrap(),
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
    let Some(input) = build_setpoint_input(world) else {
        apply_setpoint_safety(world, clock, topology, effects);
        return;
    };

    let k = &world.knobs;

    let out = evaluate_setpoint(&input, clock, &topology.hardware);

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
    let pre_clamp = out.setpoint_target;
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
    let decision = if pre_clamp == capped {
        out.decision.clone()
    } else {
        out.decision
            .clone()
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
    if let Some(current_target) = world.grid_setpoint.target.value {
        let delta = (i64::from(current_target) - i64::from(value)).abs();
        if delta < i64::from(params.setpoint_retarget_deadband_w) {
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
    if !changed {
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
        effects.push(Effect::Log {
            level: LogLevel::Info,
            source: "observer",
            message: format!(
                "GridSetpoint would be set to {value} W (owner={owner:?}); suppressed by writes_enabled=false"
            ),
        });
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

    if let Some(current_target) = world.input_current_limit.target.value {
        if (current_target - value).abs() < params.current_limit_retarget_deadband_a {
            return;
        }
    }

    // Propose target unconditionally (PR-SCHED0): see
    // `maybe_propose_setpoint`. The KillSwitch false→true edge still
    // resets every target.
    let changed = world
        .input_current_limit
        .propose_target(value, Owner::CurrentLimitController, now);
    if !changed {
        return;
    }

    // PR-SCHED0-D03: publish phase unconditionally; see `maybe_propose_setpoint`.
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::InputCurrentLimit,
        phase: world.input_current_limit.target.phase,
    }));

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
    if !changed {
        return;
    }

    // PR-SCHED0-D03: publish phase unconditionally; see `maybe_propose_setpoint`.
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id,
        phase: actuated.target.phase,
    }));

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
    if !changed {
        return;
    }

    // PR-SCHED0-D03: publish phase unconditionally; see `maybe_propose_setpoint`.
    effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
        id: ActuatedId::ZappiMode,
        phase: world.zappi_mode.target.phase,
    }));

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
    // for `Set` AND the target value actually changed (idempotent dedup
    // — repeat ticks with the same target don't re-fire).
    if !out.action.should_actuate() || !changed {
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
        effects.push(Effect::Log {
            level: LogLevel::Info,
            source: "observer",
            message: format!(
                "EddiMode would be set to {desired:?}; suppressed by writes_enabled=false"
            ),
        });
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
    let d = evaluate_weather_soc(&input, clock);
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
}
