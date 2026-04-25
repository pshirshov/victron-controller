//! Unit tests for `CoreRegistry` construction.
//!
//! These tests build their own registries rather than going through
//! any `OnceLock` singleton so they can freely construct malformed
//! graphs (cycles, missing deps, duplicates).

use crate::Clock;
use crate::topology::Topology;
use crate::types::Effect;
use crate::world::World;

use super::cores::production_cores;
use super::{Core, CoreGraphError, CoreId, CoreRegistry, DepEdge};

// -----------------------------------------------------------------------------
// Stub cores for negative tests.
// -----------------------------------------------------------------------------

struct StubCore {
    id: CoreId,
    deps: &'static [DepEdge],
}

impl Core for StubCore {
    fn id(&self) -> CoreId {
        self.id
    }
    fn depends_on(&self) -> &'static [DepEdge] {
        self.deps
    }
    fn run(
        &self,
        _world: &mut World,
        _clock: &dyn Clock,
        _topology: &Topology,
        _effects: &mut Vec<Effect>,
    ) {
    }
}

fn stub(id: CoreId, deps: &'static [DepEdge]) -> Box<dyn Core> {
    Box::new(StubCore { id, deps })
}

/// Convenience builder for tests that don't care about the field-name
/// metadata on a `DepEdge`.
const fn ord(from: CoreId) -> DepEdge {
    DepEdge { from, fields: &[] }
}

// -----------------------------------------------------------------------------
// Production-graph tests.
// -----------------------------------------------------------------------------

/// Snapshot of the topological order the production registry must
/// produce. If this changes, the runtime order of `run_*` has
/// changed — pause and confirm that's intentional.
///
/// PR-DAG-C order: ZappiActive → Setpoint → ZappiMode → EddiMode →
/// WeatherSoc → Schedules → CurrentLimit → SensorBroadcast.
/// `ZappiMode` / `EddiMode` shifted earlier (they have no real cross-
/// core reads, so Kahn's `[ZappiMode, EddiMode] both at in-degree 0
/// after popping ZappiActive` placement is correct). `CurrentLimit`
/// shifted last among actuators because it now depends on `Schedules`
/// for `battery_selected_soc_target` (zero-tick latency, was one tick
/// pre-PR-DAG-C).
const EXPECTED_PRODUCTION_ORDER: &[CoreId] = &[
    CoreId::ZappiActive,
    CoreId::Setpoint,
    CoreId::ZappiMode,
    CoreId::EddiMode,
    CoreId::WeatherSoc,
    CoreId::Schedules,
    CoreId::CurrentLimit,
    CoreId::SensorBroadcast,
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
    const SP_DEPS: &[DepEdge] = &[ord(CoreId::CurrentLimit)];
    const CL_DEPS: &[DepEdge] = &[ord(CoreId::Setpoint)];
    let cores: Vec<Box<dyn Core>> = vec![
        stub(CoreId::Setpoint, SP_DEPS),
        stub(CoreId::CurrentLimit, CL_DEPS),
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
    const DEPS: &[DepEdge] = &[ord(CoreId::ZappiActive)];
    let cores: Vec<Box<dyn Core>> = vec![stub(CoreId::Setpoint, DEPS)];
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
    const EM_DEPS: &[DepEdge] = &[ord(CoreId::ZappiActive), ord(CoreId::WeatherSoc)];
    let cores: Vec<Box<dyn Core>> = vec![
        stub(CoreId::EddiMode, EM_DEPS),
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
// PR-DAG-C — semantic-edges + per-edge field surface.
// -----------------------------------------------------------------------------

/// `CurrentLimit` must run AFTER `Schedules` so it reads the freshly-
/// written `battery_selected_soc_target` from this tick rather than
/// last tick. The PR-DAG-A linear chain put CurrentLimit before
/// Schedules; PR-DAG-C flips it via the new
/// `CurrentLimit.depends_on += [Schedules]` edge.
#[test]
fn current_limit_runs_after_schedules_post_pr_dag_c() {
    let reg = CoreRegistry::build(production_cores()).expect("valid DAG");
    let order = reg.order();
    let cl = order.iter().position(|&c| c == CoreId::CurrentLimit).expect("CL present");
    let sch = order.iter().position(|&c| c == CoreId::Schedules).expect("Schedules present");
    assert!(
        sch < cl,
        "Schedules must run before CurrentLimit so the latter reads same-tick \
         battery_selected_soc_target. Order was {order:?}",
    );
}

/// `WeatherSoc` must run AFTER `Setpoint` (reads
/// `bookkeeping.charge_to_full_required` written by Setpoint). The
/// PR-DAG-A linear chain put WeatherSoc *after* EddiMode and
/// transitively after Setpoint by accident; PR-DAG-C records the
/// real edge directly.
#[test]
fn weather_soc_runs_after_setpoint_post_pr_dag_c() {
    let reg = CoreRegistry::build(production_cores()).expect("valid DAG");
    let order = reg.order();
    let ws = order.iter().position(|&c| c == CoreId::WeatherSoc).expect("WS present");
    let sp = order.iter().position(|&c| c == CoreId::Setpoint).expect("Setpoint present");
    assert!(
        sp < ws,
        "Setpoint must run before WeatherSoc so the latter reads same-tick \
         charge_to_full_required. Order was {order:?}",
    );
}

/// User-facing wire-format check: `CoreState.depends_on` strings now
/// carry the per-edge field names so the dashboard can show *why* an
/// edge exists, not just that it does.
#[test]
fn dashboard_depends_on_strings_carry_field_names() {
    use crate::clock::FixedClock;
    use crate::topology::Topology;
    use crate::types::Effect;
    use crate::world::World;

    let mono = std::time::Instant::now();
    let naive = chrono::NaiveDate::from_ymd_opt(2026, 4, 25)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap();
    let clk = FixedClock::new(mono, naive);

    let mut world = World::fresh_boot(mono);
    let reg = CoreRegistry::build(production_cores()).expect("valid DAG");
    let mut effects: Vec<Effect> = Vec::new();
    reg.run_all(&mut world, &clk, &Topology::defaults(), &mut effects);

    // WeatherSoc → Setpoint via charge_to_full_required.
    let ws = world
        .cores_state
        .cores
        .iter()
        .find(|c| c.id == CoreId::WeatherSoc.name())
        .expect("WeatherSoc state present");
    assert_eq!(
        ws.depends_on,
        vec!["setpoint via bookkeeping.charge_to_full_required".to_string()],
        "WeatherSoc dashboard string must surface the field that motivates \
         the edge, not just the producing core name",
    );

    // CurrentLimit → ZappiActive + Setpoint + Schedules, each with fields.
    let cl = world
        .cores_state
        .cores
        .iter()
        .find(|c| c.id == CoreId::CurrentLimit.name())
        .expect("CurrentLimit state present");
    assert_eq!(
        cl.depends_on,
        vec![
            "evcharger.active via derived.zappi_active".to_string(),
            "setpoint via bookkeeping.charge_to_full_required".to_string(),
            "schedules via bookkeeping.battery_selected_soc_target".to_string(),
        ],
        "CurrentLimit must surface the three fields that motivate its edges",
    );

    // SensorBroadcast keeps fields-empty edges (pure ordering); strings
    // must omit the " via ..." suffix in that case.
    let br = world
        .cores_state
        .cores
        .iter()
        .find(|c| c.id == CoreId::SensorBroadcast.name())
        .expect("SensorBroadcast state present");
    assert!(
        br.depends_on.iter().all(|s| !s.contains(" via ")),
        "SensorBroadcast deps are ordering-only; the rendered strings \
         should not have a fake 'via ...' tail. Got: {:?}",
        br.depends_on,
    );
}

// -----------------------------------------------------------------------------
// PR-DAG-B-D02 — `classify_zappi_active` runs exactly once per tick.
// -----------------------------------------------------------------------------
//
// Successor to the PR-DAG-A-D01 regression guard. The original hazard:
// `classify_zappi_active` runs more than once per tick and straddles the
// `WAIT_TIMEOUT_MIN = 5 min` boundary between `clock.naive()` reads,
// so setpoint sees "active" and a downstream actuator sees "inactive"
// (or vice versa).
//
// PR-DAG-B moves ownership of that classification into `ZappiActiveCore`,
// which writes `world.derived.zappi_active` once at the top of the tick.
// Consumers then read the struct field — no further calls into the
// classifier occur within the same tick.
//
// The check: the `zappi_active` factor recorded on the grid-setpoint
// decision must match the value sitting in `world.derived.zappi_active`
// at the end of the tick. If any consumer ever starts re-deriving
// independently they would diverge here.

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
    fn zappi_active_drops_to_false_when_both_sensor_paths_unusable() {
        // Regression guard for the PR-DAG-B semantic change: the old
        // bookkeeping-backed path latched the last-known `zappi_active`
        // across sensor loss because `run_current_limit` early-returned
        // on the freshness gate. The new DAG-resident derivation re-runs
        // the classifier every tick and returns `false` when neither
        // input is usable — no latching. This test locks that choice.
        use std::time::Instant;
        use chrono::NaiveDate;

        use crate::clock::FixedClock;
        use crate::core_dag::Core;
        use crate::core_dag::cores::ZappiActiveCore;
        use crate::topology::Topology;
        use crate::types::Effect;
        use crate::world::World;

        let mono = Instant::now();
        let naive = NaiveDate::from_ymd_opt(2026, 4, 24)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let clk = FixedClock::new(mono, naive);

        let mut world = World::fresh_boot(mono);

        // Both inputs unusable: typed_sensors.zappi_state stays at the
        // fresh-boot `Unknown`, and evcharger_ac_power likewise. No
        // `on_reading` call — both remain `Freshness::Unknown`.
        assert!(!world.typed_sensors.zappi_state.is_usable());
        assert!(!world.sensors.evcharger_ac_power.is_usable());

        // Pre-seed `derived.zappi_active = true` — the value a prior
        // tick would have published (and which the old bookkeeping
        // path would have "latched" into this tick).
        world.derived.zappi_active = true;

        let mut effects: Vec<Effect> = Vec::new();
        ZappiActiveCore.run(&mut world, &clk, &Topology::defaults(), &mut effects);

        assert!(
            !world.derived.zappi_active,
            "zappi_active must drop to false when both the typed state \
             and evcharger_ac_power sensor are unusable — no cross-tick \
             latching. Got true, which would mean the classifier (or a \
             new bookkeeping path) is latching through sensor loss.",
        );
    }

    #[test]
    fn zappi_active_uses_power_fallback_when_typed_state_is_stale() {
        // Companion to the drops-to-false test: when typed state is
        // unusable but evcharger_ac_power is fresh and above the
        // fallback threshold, the classifier must fire `true`. This
        // documents that the derivation is not a blanket "stale ⇒
        // false" guard — it genuinely falls back to power (SPEC §5.8 /
        // §5.11) when that signal is available.
        use std::time::Instant;
        use chrono::NaiveDate;

        use crate::clock::FixedClock;
        use crate::core_dag::Core;
        use crate::core_dag::cores::ZappiActiveCore;
        use crate::topology::Topology;
        use crate::types::Effect;
        use crate::world::World;

        let mono = Instant::now();
        let naive = NaiveDate::from_ymd_opt(2026, 4, 24)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let clk = FixedClock::new(mono, naive);

        let mut world = World::fresh_boot(mono);

        // typed_sensors.zappi_state: untouched → Unknown/unusable.
        assert!(!world.typed_sensors.zappi_state.is_usable());

        // evcharger_ac_power: 800 W fresh, comfortably above the 500 W
        // SPEC §5.8 fallback threshold.
        world.sensors.evcharger_ac_power.on_reading(800.0, mono);
        assert!(world.sensors.evcharger_ac_power.is_usable());

        // Pre-set derived.zappi_active = false to prove the positive
        // transition is actually produced by the classifier, not
        // inherited from the prior tick.
        world.derived.zappi_active = false;

        let mut effects: Vec<Effect> = Vec::new();
        ZappiActiveCore.run(&mut world, &clk, &Topology::defaults(), &mut effects);

        assert!(
            world.derived.zappi_active,
            "zappi_active must be true when typed state is unusable but \
             evcharger_ac_power is fresh above the 500 W fallback \
             threshold. Got false — the power-based fallback path is \
             broken.",
        );
    }

    #[test]
    fn setpoint_decision_matches_world_derived_zappi_active_across_boundary() {
        // Straddle the 5 min WAIT_TIMEOUT boundary.
        //
        // Zappi entered `WaitingForEv` at `mono - 4m59.990s` — just
        // under the timeout. With PR-DAG-B, `ZappiActiveCore` is the
        // sole classifier caller: it classifies once and every consumer
        // reads from `world.derived.zappi_active`. Even if a consumer
        // re-classified (hazard), the monotonic clock is fixed per
        // tick so both observations agree — but the contract we're
        // defending is no re-derivation at all.
        //
        // PR-03: `zappi_last_change_signature` is now a monotonic
        // `Instant`, so we no longer need the `AdvancingClock` to
        // simulate naive()-drift across classifier calls; the stamp
        // is compared against a fixed `clock.monotonic()`.
        let base_naive = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_milli_opt(12, 4, 59, 990)
            .unwrap();

        let mono = Instant::now() + StdDuration::from_secs(600);
        let last_change_mono = mono
            .checked_sub(StdDuration::from_millis(4 * 60 * 1000 + 59 * 1000 + 990))
            .unwrap();
        let clk = AdvancingClock {
            monotonic: mono,
            naive: Cell::new(base_naive),
            step: chrono::Duration::seconds(1),
            naive_calls: Cell::new(0),
        };

        let mut world = World::fresh_boot(mono);
        seed_required_sensors(&mut world, mono);
        world.typed_sensors.zappi_state.on_reading(
            ZappiState {
                zappi_mode: ZappiMode::Eco,
                zappi_plug_state: ZappiPlugState::WaitingForEv,
                zappi_status: ZappiStatus::DivertingOrCharging,
                zappi_last_change_signature: last_change_mono,
                session_kwh: 0.0,
            },
            mono,
        );

        let _ = process(
            &Event::Tick { at: mono },
            &mut world,
            &clk,
            &Topology::defaults(),
        );

        // Setpoint's `zappi_active` factor must match the single
        // derivation `ZappiActiveCore` published at the top of the tick.
        let decision = world
            .decisions
            .grid_setpoint
            .as_ref()
            .expect("grid_setpoint decision recorded");
        let setpoint_saw_active = decision
            .factors
            .iter()
            .any(|f| f.name == "zappi_active" && f.value == "true");
        let derived_active = world.derived.zappi_active;

        assert_eq!(
            setpoint_saw_active, derived_active,
            "PR-DAG-B regression: setpoint factor zappi_active={} \
             disagreed with world.derived.zappi_active={} — the \
             derivation core's single-write-per-tick contract is \
             violated. (naive() was called {} times)",
            setpoint_saw_active,
            derived_active,
            clk.naive_calls.get(),
        );

        // Stronger claim: at the boundary constructed above, the
        // FIRST naive() read is under the timeout so the classifier
        // must return `true`. If both observations showed `false` the
        // test would silently pass without exercising the hazard.
        assert!(
            setpoint_saw_active,
            "test mis-configured: the first naive() read should place \
             delta_min under WAIT_TIMEOUT_MIN (zappi_active=true). Got \
             false — the boundary wasn't straddled, so this test is \
             not exercising the regression.",
        );
    }
}

// -----------------------------------------------------------------------------
// PR-ha-discovery-expand — SensorBroadcastCore behaviour.
// -----------------------------------------------------------------------------

mod sensor_broadcast {
    use std::time::Instant;

    use chrono::NaiveDate;

    use crate::clock::FixedClock;
    use crate::core_dag::Core;
    use crate::core_dag::cores::SensorBroadcastCore;
    use crate::topology::Topology;
    use crate::types::{Effect, PublishPayload, SensorId};
    use crate::world::World;

    // `SensorId::ALL.len()` (32 after PR-actuated-as-sensors PR-AS-A
    // — 20 original + 14 actuated-mirror sensors) sensor publishes
    // plus 3 numeric + 3 boolean bookkeeping publishes on the first
    // run with a fresh-boot world (every cache slot is absent →
    // first-write emits the value). The 14 new actuated-mirror
    // sensors emit "unavailable" placeholders since the subscriber
    // doesn't yet route to them; that's still a publish, dedup'd on
    // subsequent runs by the body cache.
    const EXPECTED_FIRST_RUN_EFFECTS: usize = 32 + 3 + 3;

    fn fixed_clock() -> FixedClock {
        let mono = Instant::now();
        let naive = NaiveDate::from_ymd_opt(2026, 4, 25)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        FixedClock::new(mono, naive)
    }

    #[test]
    fn first_run_emits_one_publish_per_published_id() {
        let clk = fixed_clock();
        let mut world = World::fresh_boot(clk.monotonic);
        let topo = Topology::defaults();
        let mut effects: Vec<Effect> = Vec::new();

        SensorBroadcastCore.run(&mut world, &clk, &topo, &mut effects);

        let publishes = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::Publish(
                        PublishPayload::Sensor { .. }
                            | PublishPayload::BookkeepingNumeric { .. }
                            | PublishPayload::BookkeepingBool { .. },
                    )
                )
            })
            .count();
        assert_eq!(
            publishes, EXPECTED_FIRST_RUN_EFFECTS,
            "first run should emit one publish per sensor + per published \
             bookkeeping field; got {publishes}, expected {EXPECTED_FIRST_RUN_EFFECTS}",
        );
        // SensorId::ALL coverage check: every sensor must have at least
        // one Publish(Sensor{id}) for its id.
        for &id in SensorId::ALL {
            let hit = effects.iter().any(|e| {
                matches!(
                    e,
                    Effect::Publish(PublishPayload::Sensor { id: i, .. }) if *i == id
                )
            });
            assert!(hit, "missing Publish(Sensor) for {id:?}");
        }
    }

    #[test]
    fn second_run_with_unchanged_world_emits_zero_publishes() {
        // Dedup contract: a republish is only triggered by a change in
        // (value, freshness) for sensors or value for bookkeeping.
        // Two consecutive runs against the same world MUST produce
        // zero publishes on the second run, otherwise FlashMQ's
        // republish ceiling triggers under steady-state ticks.
        let clk = fixed_clock();
        let mut world = World::fresh_boot(clk.monotonic);
        let topo = Topology::defaults();

        let mut first: Vec<Effect> = Vec::new();
        SensorBroadcastCore.run(&mut world, &clk, &topo, &mut first);
        assert!(!first.is_empty(), "first run should not be empty");

        let mut second: Vec<Effect> = Vec::new();
        SensorBroadcastCore.run(&mut world, &clk, &topo, &mut second);
        let publishes = second
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::Publish(
                        PublishPayload::Sensor { .. }
                            | PublishPayload::BookkeepingNumeric { .. }
                            | PublishPayload::BookkeepingBool { .. },
                    )
                )
            })
            .count();
        assert_eq!(
            publishes, 0,
            "second run with unchanged world must emit zero publishes; got {publishes}",
        );
    }

    #[test]
    fn changed_sensor_value_triggers_one_republish() {
        // A single sensor value change must produce exactly one
        // Publish(Sensor) — confirms the dedup is per-id, not all-or-
        // nothing.
        let clk = fixed_clock();
        let mut world = World::fresh_boot(clk.monotonic);
        let topo = Topology::defaults();

        let mut first: Vec<Effect> = Vec::new();
        SensorBroadcastCore.run(&mut world, &clk, &topo, &mut first);

        // Change a single sensor.
        world
            .sensors
            .battery_soc
            .on_reading(75.0, clk.monotonic);

        let mut second: Vec<Effect> = Vec::new();
        SensorBroadcastCore.run(&mut world, &clk, &topo, &mut second);
        let sensor_publishes: Vec<_> = second
            .iter()
            .filter_map(|e| match e {
                Effect::Publish(PublishPayload::Sensor { id, .. }) => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(
            sensor_publishes,
            vec![SensorId::BatterySoc],
            "exactly one sensor publish for BatterySoc expected; got {sensor_publishes:?}",
        );
    }
}
