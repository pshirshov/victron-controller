//! Unit tests for `CoreRegistry` construction.
//!
//! These tests build their own registries rather than going through
//! any `OnceLock` singleton so they can freely construct malformed
//! graphs (cycles, missing deps, duplicates).

use crate::Clock;
use crate::process::DerivedView;
use crate::topology::Topology;
use crate::types::Effect;
use crate::world::World;

use super::cores::production_cores;
use super::{Core, CoreGraphError, CoreId, CoreRegistry};

// -----------------------------------------------------------------------------
// Stub cores for negative tests.
// -----------------------------------------------------------------------------

struct StubCore {
    id: CoreId,
    deps: &'static [CoreId],
}

impl Core for StubCore {
    fn id(&self) -> CoreId {
        self.id
    }
    fn depends_on(&self) -> &'static [CoreId] {
        self.deps
    }
    fn run(
        &self,
        _world: &mut World,
        _derived: &DerivedView,
        _clock: &dyn Clock,
        _topology: &Topology,
        _effects: &mut Vec<Effect>,
    ) {
    }
}

fn stub(id: CoreId, deps: &'static [CoreId]) -> Box<dyn Core> {
    Box::new(StubCore { id, deps })
}

// -----------------------------------------------------------------------------
// Production-graph tests.
// -----------------------------------------------------------------------------

/// Snapshot of the topological order the production registry must
/// produce. If this changes, the runtime order of `run_*` has
/// changed — pause and confirm that's intentional.
const EXPECTED_PRODUCTION_ORDER: &[CoreId] = &[
    CoreId::Setpoint,
    CoreId::CurrentLimit,
    CoreId::Schedules,
    CoreId::ZappiMode,
    CoreId::EddiMode,
    CoreId::WeatherSoc,
];

#[test]
fn build_succeeds_for_production_registry() {
    let reg = CoreRegistry::build(production_cores())
        .expect("production DAG must be statically valid");
    assert_eq!(reg.order(), EXPECTED_PRODUCTION_ORDER);
}

#[test]
fn topo_order_is_deterministic() {
    let a = CoreRegistry::build(production_cores()).unwrap();
    let b = CoreRegistry::build(production_cores()).unwrap();
    assert_eq!(a.order(), b.order());
    assert_eq!(a.order(), EXPECTED_PRODUCTION_ORDER);
}

// -----------------------------------------------------------------------------
// Negative tests.
// -----------------------------------------------------------------------------

#[test]
fn rejects_cycle() {
    // Setpoint -> CurrentLimit -> Setpoint (2-cycle).
    let cores: Vec<Box<dyn Core>> = vec![
        stub(CoreId::Setpoint, &[CoreId::CurrentLimit]),
        stub(CoreId::CurrentLimit, &[CoreId::Setpoint]),
    ];
    let err = CoreRegistry::build(cores).unwrap_err();
    match err {
        CoreGraphError::Cycle { involving } => {
            assert!(involving.contains(&CoreId::Setpoint));
            assert!(involving.contains(&CoreId::CurrentLimit));
        }
        other => panic!("expected Cycle, got {other:?}"),
    }
}

#[test]
fn rejects_missing_dependency() {
    // Setpoint declares a dep on ZappiActive, but ZappiActive is NOT
    // in the registry.
    let cores: Vec<Box<dyn Core>> = vec![stub(CoreId::Setpoint, &[CoreId::ZappiActive])];
    let err = CoreRegistry::build(cores).unwrap_err();
    match err {
        CoreGraphError::MissingDependency { from, missing } => {
            assert_eq!(from, CoreId::Setpoint);
            assert_eq!(missing, CoreId::ZappiActive);
        }
        other => panic!("expected MissingDependency, got {other:?}"),
    }
}

#[test]
fn rejects_duplicate_core() {
    let cores: Vec<Box<dyn Core>> = vec![
        stub(CoreId::Setpoint, &[]),
        stub(CoreId::Setpoint, &[]),
    ];
    let err = CoreRegistry::build(cores).unwrap_err();
    match err {
        CoreGraphError::DuplicateCore(id) => assert_eq!(id, CoreId::Setpoint),
        other => panic!("expected DuplicateCore, got {other:?}"),
    }
}

// -----------------------------------------------------------------------------
// PR-DAG-A-D03 — tie-break determinism.
// -----------------------------------------------------------------------------

/// Kahn's tie-break must be stable and follow `CoreId` discriminant
/// order. With two roots (`ZappiActive` and `WeatherSoc`) feeding a
/// single dependent (`EddiMode`), the emitted order must start with
/// the smaller-discriminant root regardless of registration order —
/// `ZappiActive` comes before `WeatherSoc` in the enum definition.
#[test]
fn tie_break_follows_coreid_discriminant_order() {
    // Register in reverse discriminant order to prove tie-break is
    // doing the work, not registration order.
    let cores: Vec<Box<dyn Core>> = vec![
        stub(CoreId::EddiMode, &[CoreId::ZappiActive, CoreId::WeatherSoc]),
        stub(CoreId::WeatherSoc, &[]),
        stub(CoreId::ZappiActive, &[]),
    ];
    let reg = CoreRegistry::build(cores).expect("valid DAG");
    assert_eq!(
        reg.order(),
        vec![CoreId::ZappiActive, CoreId::WeatherSoc, CoreId::EddiMode],
    );
}

// -----------------------------------------------------------------------------
// PR-DAG-A-D02 — `compute_derived_view` runs exactly once per tick.
// -----------------------------------------------------------------------------
//
// Regression harness for the reintroduced A-05 hazard: when each core
// recomputes `DerivedView` independently, `classify_zappi_active` can
// straddle the `WAIT_TIMEOUT_MIN = 5 min` boundary between two
// uncached `clock.naive()` reads — setpoint sees "active" and
// current-limit sees "inactive" (or vice versa). The test uses an
// `AdvancingClock` that returns a DIFFERENT naive datetime on every
// call, straddling the boundary, and then asserts that the setpoint
// decision's `zappi_active` factor and the `bookkeeping.zappi_active`
// value current-limit wrote are CONSISTENT. With the D01 fix they
// must match because both reads came from a single `DerivedView`.

mod d02_boundary_consistency {
    use std::cell::Cell;
    use std::time::{Duration as StdDuration, Instant};

    use chrono::{NaiveDate, NaiveDateTime};

    use crate::Clock;
    use crate::myenergi::{ZappiMode, ZappiPlugState, ZappiState, ZappiStatus};
    use crate::process::process;
    use crate::topology::Topology;
    use crate::types::Event;
    use crate::world::World;

    /// A `Clock` whose `naive()` ADVANCES by `step` every call.
    /// `monotonic()` stays fixed (TASS freshness arithmetic must not
    /// drift during one tick).
    struct AdvancingClock {
        monotonic: Instant,
        naive: Cell<NaiveDateTime>,
        step: chrono::Duration,
        naive_calls: Cell<u32>,
    }

    impl Clock for AdvancingClock {
        fn monotonic(&self) -> Instant {
            self.monotonic
        }
        fn naive(&self) -> NaiveDateTime {
            let cur = self.naive.get();
            self.naive.set(cur + self.step);
            self.naive_calls.set(self.naive_calls.get() + 1);
            cur
        }
    }

    fn seed_required_sensors(world: &mut World, at: Instant) {
        world.knobs.writes_enabled = true;
        let ss = &mut world.sensors;
        ss.battery_soc.on_reading(90.0, at); // above export threshold
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
    }

    #[test]
    fn setpoint_and_current_limit_agree_across_wait_timeout_boundary() {
        // Straddle the 5 min WAIT_TIMEOUT boundary between two
        // hypothetical `compute_derived_view` calls.
        //
        // Clock starts at 12:04:59.990; Zappi entered `WaitingForEv`
        // at 12:00:00.000 (4 min 59.990 s prior). AdvancingClock steps
        // `naive()` forward by 1 s on every call. With the pre-D01
        // code each actuator core recomputes the view independently,
        // so classify runs twice: call 1 sees delta=4:59.990
        // (active=true), call 2 (after several intervening controller
        // `naive()` reads) sees delta well over 5 min (active=false)
        // — the two cores then disagree. With the D01 fix, classify
        // runs exactly ONCE and both cores read the same value.
        let base_naive = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_milli_opt(12, 4, 59, 990)
            .unwrap();
        let last_change = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_milli_opt(12, 0, 0, 0)
            .unwrap();

        let mono = Instant::now() + StdDuration::from_secs(60);
        let clk = AdvancingClock {
            monotonic: mono,
            naive: Cell::new(base_naive),
            // 1 s per naive() call — large enough that any second
            // classifier call lands comfortably past the 5 min
            // boundary, regardless of how many intervening `naive()`
            // reads the controllers make.
            step: chrono::Duration::seconds(1),
            naive_calls: Cell::new(0),
        };

        let mut world = World::fresh_boot(mono);
        seed_required_sensors(&mut world, mono);
        // Live Zappi state: WaitingForEv with last_change 4:59.990
        // before the clock's initial naive(). The classifier's
        // `delta_min > WAIT_TIMEOUT_MIN` check is the boundary we
        // want to straddle.
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Eco,
                zappi_plug_state: ZappiPlugState::WaitingForEv,
                zappi_status: ZappiStatus::DivertingOrCharging,
                zappi_last_change_signature: last_change,
            },
            mono,
        );
        // Bookkeeping starts at the cold-boot default so the two
        // controllers can't accidentally agree via the stale latch.
        world.bookkeeping.zappi_active = false;

        let _ = process(
            &Event::Tick { at: mono },
            &mut world,
            &clk,
            &Topology::defaults(),
        );

        // --- Observation 1: setpoint recorded a factor "zappi_active"
        //     with the value DerivedView gave it.
        let decision = world
            .decisions
            .grid_setpoint
            .as_ref()
            .expect("grid_setpoint decision recorded");
        let setpoint_saw_active = decision
            .factors
            .iter()
            .any(|f| f.name == "zappi_active" && f.value == "true");

        // --- Observation 2: current-limit wrote bookkeeping.zappi_active
        //     from its own DerivedView input.
        let current_limit_wrote_active = world.bookkeeping.zappi_active;

        assert_eq!(
            setpoint_saw_active, current_limit_wrote_active,
            "PR-DAG-A-D01 regression: setpoint (factor zappi_active={}) \
             and current_limit (bookkeeping.zappi_active={}) disagreed \
             across the WAIT_TIMEOUT_MIN boundary — they must both \
             read from a single per-tick DerivedView. \
             (naive() was called {} times this tick)",
            setpoint_saw_active,
            current_limit_wrote_active,
            clk.naive_calls.get(),
        );

        // Stronger claim: at the boundary constructed above, the
        // FIRST naive() read is under the timeout so the classifier
        // must return `true`. If the test observed `false` for both
        // it would silently pass without exercising the hazard.
        assert!(
            setpoint_saw_active,
            "test mis-configured: the first naive() read should place \
             delta_min under WAIT_TIMEOUT_MIN (zappi_active=true). Got \
             false — the boundary wasn't straddled, so this test is \
             not exercising D01.",
        );
    }
}
