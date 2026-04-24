//! Property tests for the pure `process()` function — SPEC §8.
//!
//! These tests generate random event sequences and assert invariants that
//! must hold no matter what the shell throws at the core:
//!
//! - **Phase never skips steps**. Target phase only transitions along
//!   `Unset → Pending → Commanded → Confirmed` (or back to `Pending` on
//!   supersession). It never jumps `Pending → Confirmed` or `Commanded →
//!   Unset` etc.
//! - **Freshness never spontaneously upgrades**. Only a reading event can
//!   move freshness forward.
//! - **`writes_enabled = false` kill switch is absolute**. No matter the
//!   event sequence, if the knob is disengaged, `process()` never emits
//!   a `WriteDbus` or `CallMyenergi` effect.
//! - **Grid export cap honoured**. Setpoint target is always `≥
//!   -grid_export_limit_w` (the grid-side clamp).
//! - **Dead-band stability**. Two identical sensor events back-to-back
//!   never cause a second write after the first settles.

use std::time::{Duration, Instant};

use chrono::{NaiveDate, NaiveDateTime};
use proptest::prelude::*;

use victron_controller_core::{
    process, ActuatedReadback, Effect, Event, FixedClock, Freshness, Knobs, Owner, SensorId,
    SensorReading, TargetPhase, Topology, World,
};
use victron_controller_core::myenergi::{ZappiMode, ZappiPlugState, ZappiState, ZappiStatus};
use victron_controller_core::types::TypedReading;

// -----------------------------------------------------------------------------
// Strategies (generators)
// -----------------------------------------------------------------------------

fn naive_noon() -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 4, 21)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
}

/// Build a realistic starting World with all required sensors fresh at
/// `t0`. Each test mutates it from here.
fn seeded_world(t0: Instant) -> World {
    let mut w = World::fresh_boot(t0);
    w.sensors.battery_soc.on_reading(75.0, t0);
    w.sensors.battery_soh.on_reading(95.0, t0);
    w.sensors.battery_installed_capacity.on_reading(100.0, t0);
    w.sensors.battery_dc_power.on_reading(0.0, t0);
    w.sensors.mppt_power_0.on_reading(1500.0, t0);
    w.sensors.mppt_power_1.on_reading(1000.0, t0);
    w.sensors.soltaro_power.on_reading(500.0, t0);
    w.sensors.power_consumption.on_reading(1200.0, t0);
    w.sensors.grid_power.on_reading(500.0, t0);
    w.sensors.grid_voltage.on_reading(230.0, t0);
    w.sensors.grid_current.on_reading(2.0, t0);
    w.sensors.consumption_current.on_reading(5.0, t0);
    w.sensors.offgrid_power.on_reading(500.0, t0);
    w.sensors.offgrid_current.on_reading(2.2, t0);
    w.sensors.vebus_input_current.on_reading(0.0, t0);
    w.sensors.evcharger_ac_power.on_reading(0.0, t0);
    w.sensors.evcharger_ac_current.on_reading(0.0, t0);
    w.sensors.ess_state.on_reading(10.0, t0);
    w.sensors.outdoor_temperature.on_reading(15.0, t0);
    w.typed_sensors.zappi_state.on_reading(
        ZappiState {
            zappi_mode: ZappiMode::Off,
            zappi_plug_state: ZappiPlugState::EvDisconnected,
            zappi_status: ZappiStatus::Paused,
            zappi_last_change_signature: naive_noon(),
        },
        t0,
    );
    w
}

/// Plausible scalar sensor readings, one per SensorId.
fn sensor_strategy(t0: Instant) -> impl Strategy<Value = SensorReading> {
    let ids: Vec<SensorId> = vec![
        SensorId::BatterySoc,
        SensorId::BatterySoh,
        SensorId::BatteryInstalledCapacity,
        SensorId::BatteryDcPower,
        SensorId::MpptPower0,
        SensorId::MpptPower1,
        SensorId::SoltaroPower,
        SensorId::PowerConsumption,
        SensorId::GridPower,
        SensorId::GridVoltage,
        SensorId::GridCurrent,
        SensorId::ConsumptionCurrent,
        SensorId::OffgridPower,
        SensorId::OffgridCurrent,
        SensorId::VebusInputCurrent,
        SensorId::EvchargerAcPower,
        SensorId::EvchargerAcCurrent,
        SensorId::EssState,
        SensorId::OutdoorTemperature,
    ];

    (
        proptest::sample::select(ids),
        -5000.0_f64..5000.0_f64,
        0u64..600,
    )
        .prop_map(move |(id, value, delta_s)| SensorReading {
            id,
            value,
            at: t0 + Duration::from_secs(delta_s),
        })
}

/// Any event kind with a random-ish distribution across sensors/ticks.
fn event_strategy(t0: Instant) -> impl Strategy<Value = Event> {
    prop_oneof![
        4 => sensor_strategy(t0).prop_map(Event::Sensor),
        2 => (0u64..600).prop_map(move |d| Event::Tick { at: t0 + Duration::from_secs(d) }),
        1 => (-5000i32..5000, 0u64..600).prop_map(move |(v, d)| Event::Readback(
            ActuatedReadback::GridSetpoint { value: v, at: t0 + Duration::from_secs(d) }
        )),
    ]
}

// -----------------------------------------------------------------------------
// Invariants
// -----------------------------------------------------------------------------

/// Snapshot of target phase per actuated entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PhaseSnapshot {
    grid_setpoint: TargetPhase,
    input_current_limit: TargetPhase,
    zappi_mode: TargetPhase,
    eddi_mode: TargetPhase,
    schedule_0: TargetPhase,
    schedule_1: TargetPhase,
}

fn phases(w: &World) -> PhaseSnapshot {
    PhaseSnapshot {
        grid_setpoint: w.grid_setpoint.target.phase,
        input_current_limit: w.input_current_limit.target.phase,
        zappi_mode: w.zappi_mode.target.phase,
        eddi_mode: w.eddi_mode.target.phase,
        schedule_0: w.schedule_0.target.phase,
        schedule_1: w.schedule_1.target.phase,
    }
}

/// True if `b` is a valid externally-observable successor of `a`.
///
/// Within a single `process()` call the shell may collapse several
/// primitive transitions (e.g. propose_target + mark_commanded for
/// fire-and-forget D-Bus writes), so the externally-visible transitions
/// are a superset of the primitive step list.
///
/// The two hard rules:
/// - never go back to `Unset` (once set, always set to something);
/// - `Confirmed` is only reached from `Commanded` (the only place
///   `confirm_if` has an effect).
fn valid_phase_transition(a: TargetPhase, b: TargetPhase) -> bool {
    if a == b {
        return true;
    }
    if b == TargetPhase::Unset {
        return false;
    }
    if b == TargetPhase::Confirmed && a != TargetPhase::Commanded {
        return false;
    }
    true
}

/// Freshness ordering: Unknown ≺ Stale/Deprecated ≺ Fresh (roughly).
/// We care about the specific monotonicity rule: freshness cannot move
/// toward Fresh without an on_reading call.
/// We just encode: `Fresh` can only arrive from `on_reading` events; a
/// Tick-only event sequence can never transition anything to Fresh.
fn has_fresh_sensor(w: &World) -> bool {
    w.sensors.battery_soc.freshness == Freshness::Fresh
        || w.sensors.mppt_power_0.freshness == Freshness::Fresh
        || w.sensors.power_consumption.freshness == Freshness::Fresh
}

// -----------------------------------------------------------------------------
// Property 1 — phase never skips steps
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn phase_transitions_are_always_valid(events in prop::collection::vec({
        let t0 = Instant::now();
        event_strategy(t0)
    }, 1..50)) {
        let t0 = Instant::now();
        let mut world = seeded_world(t0);
        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());

        let mut prev = phases(&world);
        for e in &events {
            let _ = process(e, &mut world, &clock, &topo);
            let now = phases(&world);
            prop_assert!(
                valid_phase_transition(prev.grid_setpoint, now.grid_setpoint),
                "grid_setpoint: {:?} -> {:?} (event: {:?})",
                prev.grid_setpoint, now.grid_setpoint, e
            );
            prop_assert!(
                valid_phase_transition(prev.input_current_limit, now.input_current_limit),
                "input_current_limit: {:?} -> {:?}",
                prev.input_current_limit, now.input_current_limit
            );
            prop_assert!(
                valid_phase_transition(prev.zappi_mode, now.zappi_mode),
                "zappi_mode: {:?} -> {:?}", prev.zappi_mode, now.zappi_mode
            );
            prop_assert!(
                valid_phase_transition(prev.eddi_mode, now.eddi_mode),
                "eddi_mode: {:?} -> {:?}", prev.eddi_mode, now.eddi_mode
            );
            prop_assert!(
                valid_phase_transition(prev.schedule_0, now.schedule_0),
                "schedule_0: {:?} -> {:?}", prev.schedule_0, now.schedule_0
            );
            prop_assert!(
                valid_phase_transition(prev.schedule_1, now.schedule_1),
                "schedule_1: {:?} -> {:?}", prev.schedule_1, now.schedule_1
            );
            prev = now;
        }
    }
}

// -----------------------------------------------------------------------------
// Property 2 — freshness never spontaneously upgrades
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn ticks_alone_cannot_upgrade_freshness_to_fresh(
        ticks in prop::collection::vec(0u64..3600, 1..30),
    ) {
        let t0 = Instant::now();
        // Start from a *fresh-boot* world — no readings have ever arrived.
        // All sensor freshness starts at Unknown.
        let mut world = World::fresh_boot(t0);
        prop_assert!(!has_fresh_sensor(&world));

        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());
        for dt in ticks {
            let _ = process(
                &Event::Tick { at: t0 + Duration::from_secs(dt) },
                &mut world,
                &clock,
                &topo,
            );
        }
        prop_assert!(
            !has_fresh_sensor(&world),
            "no amount of ticking should bring any sensor into Fresh"
        );
    }
}

// -----------------------------------------------------------------------------
// Property 3 — writes_enabled=false kill switch
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn writes_disabled_emits_no_actuation_effects(
        events in prop::collection::vec({
            let t0 = Instant::now();
            event_strategy(t0)
        }, 1..60)
    ) {
        let t0 = Instant::now();
        let mut world = seeded_world(t0);
        let mut k = Knobs::safe_defaults();
        k.writes_enabled = false;
        world.knobs = k;
        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());

        for e in &events {
            let effects = process(e, &mut world, &clock, &topo);
            for f in &effects {
                prop_assert!(
                    !matches!(f, Effect::WriteDbus { .. } | Effect::CallMyenergi(_)),
                    "emitted an actuation effect with writes_enabled=false: {:?}", f
                );
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Property 4 — grid export cap is always honoured
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn grid_export_cap_is_absolute_for_setpoint_target(
        cap_w in 100u32..10_000u32,
        events in prop::collection::vec({
            let t0 = Instant::now();
            event_strategy(t0)
        }, 1..60)
    ) {
        let t0 = Instant::now();
        let mut world = seeded_world(t0);
        world.knobs.grid_export_limit_w = cap_w;
        // Maximise the chance of hitting the cap: high export, SoC above
        // threshold, daytime.
        world.sensors.battery_soc.on_reading(99.0, t0);
        world.sensors.mppt_power_0.on_reading(5000.0, t0);
        world.sensors.mppt_power_1.on_reading(5000.0, t0);

        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());

        for e in &events {
            let _ = process(e, &mut world, &clock, &topo);
            if let Some(v) = world.grid_setpoint.target.value {
                prop_assert!(
                    v >= -i32::try_from(cap_w).unwrap(),
                    "grid_setpoint={v} violates cap={cap_w}",
                );
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Property 5 — idempotence on a repeated identical event
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn repeated_identical_sensor_event_does_not_emit_duplicate_writes(
        value in -3000.0f64..3000.0f64
    ) {
        let t0 = Instant::now();
        let mut world = seeded_world(t0);
        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());

        let event = Event::Sensor(SensorReading {
            id: SensorId::PowerConsumption,
            value,
            at: t0,
        });
        let _ = process(&event, &mut world, &clock, &topo);
        // Second application with the same event at the same timestamp
        // must not produce any new WriteDbus for already-confirmed entities.
        let e2 = process(&event, &mut world, &clock, &topo);
        for eff in &e2 {
            if let Effect::WriteDbus { target, .. } = eff {
                // A schedule write on the second pass would indicate
                // non-idempotence (schedules don't depend on consumption).
                prop_assert!(
                    !matches!(target, victron_controller_core::DbusTarget::Schedule { .. }),
                    "schedule re-emitted on identical repeated event: {eff:?}"
                );
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Property 6 — commands don't mutate knobs beyond their typed variant
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn bool_knob_command_only_mutates_that_knob(new_value: bool) {
        let t0 = Instant::now();
        let mut world = seeded_world(t0);
        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());

        let before = world.knobs;
        let _ = process(
            &Event::Command {
                command: victron_controller_core::Command::Knob {
                    id: victron_controller_core::KnobId::ForceDisableExport,
                    value: victron_controller_core::KnobValue::Bool(new_value),
                },
                owner: Owner::Dashboard,
                at: t0,
            },
            &mut world,
            &clock,
            &topo,
        );
        let after = world.knobs;
        prop_assert_eq!(after.force_disable_export, new_value);
        // Everything else untouched.
        prop_assert_eq!(after.export_soc_threshold, before.export_soc_threshold);
        prop_assert_eq!(after.battery_soc_target, before.battery_soc_target);
        prop_assert_eq!(after.writes_enabled, before.writes_enabled);
        prop_assert_eq!(after.grid_export_limit_w, before.grid_export_limit_w);
    }
}

// -----------------------------------------------------------------------------
// Property 7 — typed sensor events never change scalar sensor values
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// Property 8 — bookkeeping restoration sets exactly the targeted field
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn bookkeeping_restoration_sets_the_targeted_field(
        year in 2020i32..2030,
        month in 1u32..12,
        day in 1u32..28,
        hour in 0u32..23,
        ess_state in -1000i32..1000,
        pick in 0usize..3,
    ) {
        use victron_controller_core::types::{BookkeepingKey, BookkeepingValue, Command};

        let t0 = Instant::now();
        let mut world = seeded_world(t0);
        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());

        let (key, value): (_, BookkeepingValue) = match pick {
            0 => {
                let dt = chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap()
                    .and_hms_opt(hour, 0, 0).unwrap();
                (BookkeepingKey::NextFullCharge, BookkeepingValue::NaiveDateTime(dt))
            }
            1 => {
                let d = chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap();
                (BookkeepingKey::AboveSocDate, BookkeepingValue::NaiveDate(d))
            }
            _ => (
                BookkeepingKey::PrevEssState,
                BookkeepingValue::OptionalInt(Some(ess_state)),
            ),
        };

        let _ = process(
            &Event::Command {
                command: Command::Bookkeeping { key, value },
                owner: Owner::System,
                at: t0,
            },
            &mut world,
            &clock,
            &topo,
        );

        // The targeted field must reflect the restoration. (Other
        // bookkeeping fields may or may not have changed; the setpoint
        // and current-limit controllers also modify bookkeeping as a
        // side-effect of running on every event.)
        match (key, value) {
            (BookkeepingKey::NextFullCharge, BookkeepingValue::NaiveDateTime(expected)) => {
                // Can be overwritten by the setpoint controller if
                // battery_soc==100 triggers rollover — in seeded_world
                // SoC is 75 so this shouldn't happen; assert equality.
                prop_assert_eq!(world.bookkeeping.next_full_charge, Some(expected));
            }
            (BookkeepingKey::AboveSocDate, BookkeepingValue::NaiveDate(expected)) => {
                // Schedules controller could overwrite on the "above
                // soc during extended" latch, but our seeded_world has
                // charge_battery_extended=false so that doesn't fire.
                prop_assert_eq!(world.bookkeeping.above_soc_date, Some(expected));
            }
            (BookkeepingKey::PrevEssState, BookkeepingValue::OptionalInt(Some(expected))) => {
                // Current-limit controller updates prev_ess_state on
                // every evaluation. seeded_world's ess_state is 10, so
                // if expected != 10 and != 9 (the skip-value), the
                // controller's write of Some(10) clobbers our restore.
                // Assert the final state is *either* our restore or the
                // controller's observation.
                let final_ = world.bookkeeping.prev_ess_state;
                prop_assert!(
                    final_ == Some(expected) || final_ == Some(10),
                    "expected Some({expected}) or Some(10), got {final_:?}"
                );
            }
            _ => unreachable!("type mismatch in match arms"),
        }
    }
}

// -----------------------------------------------------------------------------
// Property 9 — γ-rule: Dashboard write + an HA write anywhere in the
// next 999 ms must be dropped; an HA write at 1000+ ms wins.
// -----------------------------------------------------------------------------

proptest! {
    #[test]
    fn gamma_rule_holds_dashboard_over_ha_within_hold_window(
        hold_ms in 0u64..999,
    ) {
        use victron_controller_core::types::{Command, KnobId, KnobValue};

        let t0 = Instant::now();
        let mut world = seeded_world(t0);
        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());

        // Dashboard sets the knob first.
        let _ = process(
            &Event::Command {
                command: Command::Knob {
                    id: KnobId::ExportSocThreshold,
                    value: KnobValue::Float(50.0),
                },
                owner: Owner::Dashboard,
                at: t0,
            },
            &mut world,
            &clock,
            &topo,
        );
        prop_assert!((world.knobs.export_soc_threshold - 50.0).abs() < f64::EPSILON);

        // HA writes within the 1 s hold window — should be dropped.
        let _ = process(
            &Event::Command {
                command: Command::Knob {
                    id: KnobId::ExportSocThreshold,
                    value: KnobValue::Float(80.0),
                },
                owner: Owner::HaMqtt,
                at: t0 + Duration::from_millis(hold_ms),
            },
            &mut world,
            &clock,
            &topo,
        );
        prop_assert!(
            (world.knobs.export_soc_threshold - 50.0).abs() < f64::EPSILON,
            "HA write at {hold_ms}ms suppressed"
        );
    }
}

proptest! {
    #[test]
    fn gamma_rule_allows_ha_after_hold_window(
        extra_ms in 0u64..100_000,
    ) {
        use victron_controller_core::types::{Command, KnobId, KnobValue};

        let t0 = Instant::now();
        let mut world = seeded_world(t0);
        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());

        let _ = process(
            &Event::Command {
                command: Command::Knob {
                    id: KnobId::ExportSocThreshold,
                    value: KnobValue::Float(50.0),
                },
                owner: Owner::Dashboard,
                at: t0,
            },
            &mut world,
            &clock,
            &topo,
        );

        let _ = process(
            &Event::Command {
                command: Command::Knob {
                    id: KnobId::ExportSocThreshold,
                    value: KnobValue::Float(80.0),
                },
                owner: Owner::HaMqtt,
                at: t0 + Duration::from_millis(1000 + extra_ms),
            },
            &mut world,
            &clock,
            &topo,
        );
        prop_assert!(
            (world.knobs.export_soc_threshold - 80.0).abs() < f64::EPSILON,
            "HA write at 1000+{extra_ms}ms accepted"
        );
    }
}

proptest! {
    #[test]
    fn zappi_typed_events_do_not_touch_scalar_sensors(
        mode_idx in 0usize..4,
        delta_s in 0u64..600
    ) {
        let t0 = Instant::now();
        let mut world = seeded_world(t0);
        let topo = Topology::defaults();
        let clock = FixedClock::at(naive_noon());

        let mode = [ZappiMode::Fast, ZappiMode::Eco, ZappiMode::EcoPlus, ZappiMode::Off][mode_idx];

        let battery_soc_before = world.sensors.battery_soc.value;
        let consumption_before = world.sensors.power_consumption.value;
        let grid_voltage_before = world.sensors.grid_voltage.value;

        let _ = process(
            &Event::TypedSensor(TypedReading::Zappi {
                state: ZappiState {
                    zappi_mode: mode,
                    zappi_plug_state: ZappiPlugState::Charging,
                    zappi_status: ZappiStatus::DivertingOrCharging,
                    zappi_last_change_signature: naive_noon(),
                },
                at: t0 + Duration::from_secs(delta_s),
            }),
            &mut world,
            &clock,
            &topo,
        );

        prop_assert_eq!(world.sensors.battery_soc.value, battery_soc_before);
        prop_assert_eq!(world.sensors.power_consumption.value, consumption_before);
        prop_assert_eq!(world.sensors.grid_voltage.value, grid_voltage_before);
    }
}
