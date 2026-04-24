# PR-DAG — TASS cores as a validated DAG with topological orchestrator

**Milestone:** M-AUDIT-2
**Status:** planned → ready to execute
**Driving feedback:** if two TASS cores need to agree on a classifier, the derivation should be its own core; cores form a DAG executed in topological order; graph validated at registry construction.

---

## 0. Background

`process.rs`'s `run_controllers` drives six free functions in hand-maintained fixed order. PR-04 (commit `e04bba6`) patched A-05 by introducing a `DerivedView { zappi_active }` computed at the top of the tick and threaded into `run_setpoint` and `run_current_limit`. That scales poorly — every new cross-core derivation grows `DerivedView`, and the implicit coupling between cores writing/reading the same `bookkeeping` field remains fragile.

PR-DAG reframes controllers as nodes in a validated dependency DAG. Derivations read by ≥ 2 cores are lifted into their own cores whose output lives in a pure `world.derived` struct recomputed each tick.

---

## 1. `Core` trait design

Module: `crates/core/src/core_dag/mod.rs`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreId {
    // Derivation cores (pure — write to world.derived, never to bookkeeping).
    ZappiActive,

    // Actuator cores (propose targets, update bookkeeping, emit effects).
    Setpoint,
    CurrentLimit,
    Schedules,
    ZappiMode,
    EddiMode,
    WeatherSoc,
}

pub trait Core: Send + Sync {
    fn id(&self) -> CoreId;
    fn depends_on(&self) -> &'static [CoreId];
    fn run(
        &self,
        world: &mut World,
        clock: &dyn Clock,
        topology: &Topology,
        effects: &mut Vec<Effect>,
    );
}
```

Each existing `run_*` becomes a zero-sized-struct impl. No per-core input generics; each core continues to build its own local `*Input` from `world`. `classify_zappi_active` already lives in `crates/core/src/controllers/zappi_active.rs`; `ZappiActiveCore` wraps it.

`depends_on` is `&'static [CoreId]` — a compile-time artifact the `CoreRegistry` reads once at startup.

---

## 2. Registry + topological ordering

```rust
pub struct CoreRegistry {
    cores: Vec<Box<dyn Core>>,
    order: Vec<usize>,
}

impl CoreRegistry {
    pub fn build(cores: Vec<Box<dyn Core>>) -> Result<Self, CoreGraphError> { ... }
    pub fn run_all(
        &self,
        world: &mut World,
        clock: &dyn Clock,
        topology: &Topology,
        effects: &mut Vec<Effect>,
    ) {
        for &idx in &self.order {
            self.cores[idx].run(world, clock, topology, effects);
        }
    }
}

pub enum CoreGraphError {
    MissingDependency { from: CoreId, missing: CoreId },
    Cycle { involving: Vec<CoreId> },
    DuplicateCore(CoreId),
}
```

- **Algorithm:** Kahn's with deterministic tie-break (by `CoreId` discriminant).
- **Where constructed:** `OnceLock<CoreRegistry>` inside `core_dag::registry()`, initialized lazily; `run_controllers` replaced by `registry().run_all(...)`.
- **Registry unit tests:**
  - `topo_order_is_deterministic`
  - `rejects_missing_dependency`
  - `rejects_cycle`
  - `rejects_duplicate`
  - `production_registry_builds`

---

## 3. Derivation cores — `world.derived`

New struct `DerivedState` on `World`, distinct from `Bookkeeping`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DerivedState {
    pub zappi_active: bool,
    // Future migrations land here.
}
```

**Why not `Bookkeeping`?** Bookkeeping is persisted to retained MQTT; mixing per-tick recomputations into it would blur that contract and create phantom retained publishes. `DerivedState` is recomputed each tick; never retained.

`ZappiActiveCore`:

```rust
struct ZappiActiveCore;
impl Core for ZappiActiveCore {
    fn id(&self) -> CoreId { CoreId::ZappiActive }
    fn depends_on(&self) -> &'static [CoreId] { &[] }  // roots on sensors
    fn run(&self, world, clock, _topology, _effects) {
        world.derived.zappi_active = classify_zappi_active(world, clock);
    }
}
```

`SetpointCore` / `CurrentLimitCore` / `SchedulesCore` read `world.derived.zappi_active`. `bookkeeping.zappi_active` is **deleted**; the retained MQTT topic is drained via an empty retained publish on the shell side during boot (one-line change).

---

## 4. Audit of shared bookkeeping within a tick

| Field | Writer | Reader (same tick) | Decision |
|---|---|---|---|
| `zappi_active` | `run_current_limit` | `run_setpoint` + `run_schedules` (@ `:804`) | **Derivation core** `ZappiActiveCore`. Delete from bookkeeping. Flagship migration. |
| `charge_to_full_required` | `run_setpoint` (`:647`) | `run_current_limit` (`:693`), `run_schedules` (`:789`), `run_weather_soc` (`:1126`) | **Stays in bookkeeping** (persists across ticks — weekly Sunday-17:00). Encode within-tick ordering via `depends_on`: `CurrentLimit.depends_on += [Setpoint]`, `Schedules.depends_on += [Setpoint]`, `WeatherSoc.depends_on += [Setpoint]`. |
| `battery_selected_soc_target` | `run_schedules` (`:829`) | `run_current_limit` (`:695`) | **Previously unflagged ordering hazard.** Today current-limit runs BEFORE schedules → reads yesterday's value. Option (a): `CurrentLimit.depends_on += [Schedules]` — single line, solves it. Option (b): extract a dedicated derivation core — larger refactor of `evaluate_schedules`. **Recommend (a).** |
| `charge_battery_extended_today` | `run_weather_soc` (`:1168`) | `run_schedules` (`:790`) | **Stays in bookkeeping** (persists across ticks). Today schedules runs first → one-tick latency at 01:55. Declare `Schedules.depends_on += [WeatherSoc]` to eliminate. |
| `above_soc_date`, `eddi_last_transition_at`, `prev_ess_state`, `next_full_charge`, `soc_end_of_day_target`, `effective_export_soc_threshold` | — | No within-tick cross-reader (self only) | No action. |

**Conclusion:** one derivation core (`ZappiActiveCore`) is forced. Everything else is solved by declaring `depends_on` correctly.

---

## 5. Migration path / PR sequence

Split into three sub-PRs. **PR-DAG is "done" after A + B;** C is recommended but deferrable.

### PR-DAG-A — Infrastructure (mandatory, zero behavior change)
- Add `core_dag/{mod.rs,tests.rs}`: `Core` trait, `CoreId`, `CoreRegistry`, `CoreGraphError`.
- Wrap existing `run_*` as zero-sized-struct `Core` impls. Each declares `depends_on` matching today's hand-rolled order.
- `ZappiActive` is NOT yet a core at this stage — `DerivedView` still plumbs through unchanged.
- Replace `run_controllers` body with `registry().run_all(...)`.
- Expected diff: ~+400/-50 lines additive. No snapshot-test churn.
- **Acceptance:** `cargo test --all` green; `clippy -D warnings` green; ARMv7 release builds.

### PR-DAG-B — Migrate `zappi_active` (mandatory for PR-DAG "done")
- Introduce `DerivedState` on `World`.
- Promote `classify_zappi_active` into `ZappiActiveCore` with `depends_on = []`.
- Add `ZappiActive` edge to `Setpoint`, `CurrentLimit`, `Schedules`.
- Delete `bookkeeping.zappi_active`, `DerivedView` struct, `compute_derived_view`, and `zappi_active` fields from every `*InputGlobals`.
- Update ~5 test sites in `process.rs` to read `world.derived.zappi_active`.
- Shell-side: empty retained publish on the old `bookkeeping/zappi_active` topic during boot.
- **Tear-down invariant:** `rg "DerivedView|bookkeeping\.zappi_active"` returns empty inside `crates/core`.

### PR-DAG-C — Remaining `depends_on` wiring (recommended, deferrable)
- `CurrentLimit.depends_on += [Setpoint, Schedules]` (charge_to_full_required, battery_selected_soc_target).
- `Schedules.depends_on += [Setpoint, WeatherSoc]` (charge_to_full_required, charge_battery_extended_today).
- `WeatherSoc.depends_on += [Setpoint]` (charge_to_full_required).
- Each edge commented with the bookkeeping field that motivates it.
- Adds one regression test per edge.

---

## 6. Test strategy

1. **Graph well-formedness (PR-DAG-A).** Build-success, cycle, missing-dep, duplicate-id unit tests.
2. **Stable topological order (PR-DAG-A).** Snapshot the order; any change surfaces as a test diff.
3. **Behavioral equivalence per migrated derivation (PR-DAG-B).** Parameterized test running a battery of `World` states through both the pre- and post-refactor pipelines; assert identical `zappi_active` values. Delete the shim after landing.
4. **Existing regression tests preserved.** PR-04 A-05 tests (`process.rs:2192`, `:2243`) stay green with updated reads against `world.derived.zappi_active`.
5. **Observer-mode preservation (PR-DAG-B).** For each actuator core, assert under `knobs.writes_enabled=false`: no target mutation, a Log effect emitted, Decision still populated.
6. **Honesty invariant.** 3–4 branches per core with output changes; assert `world.decisions.<field>` is populated post-`process()`.

---

## 7. Risks

- **`run_*` signature churn rippling into tests.** Contained: external tests call `process()`, not individual cores. PR-DAG-B's `bookkeeping.zappi_active` removal ripples into ~5 test sites — grep identifies them.
- **Two sources of truth between PR-DAG-A and PR-DAG-B.** `DerivedView` + `world.derived.zappi_active` + `bookkeeping.zappi_active` could coexist if -B is left unfinished. PR-DAG-B's tear-down invariant check (§5.B) enforces cleanup.
- **Observer mode (PR-05).** Mechanical-only body migration preserves the `maybe_propose_*` early-returns. §6.5 locks it in.
- **Decision emission (honesty invariant).** Mechanical migration preserves. §6.6 locks it in.
- **Three-layer actuation safety chain.** Unaffected; safety chain is below the scheduling layer.
- **`OnceLock` + test isolation.** `CoreRegistry` tests that need to mutate the graph must construct their own, not go through `registry()`. Documented in `core_dag/tests.rs`.
- **`Box<dyn Core>` vtable cost.** One indirection per core per tick; negligible vs. tick budget.
- **Retained MQTT orphan after -B.** Empty retained publish drains the topic on boot; one-line shell change.

---

## Acceptance criteria

- `cargo test --all` green after each of -A, -B, -C.
- `cargo clippy --all-targets -- -D warnings` green.
- ARMv7 cross-compile green.
- `rg "DerivedView"` in `crates/core` empty after -B.
- `rg "bookkeeping\.zappi_active"` in `crates/core` empty after -B.
- Production `CoreRegistry::build()` succeeds; topological order matches snapshot.
- Observer-mode tests pass for all six actuator cores after -B.
- PR-04 A-05 regression tests stay green, updated to `world.derived.zappi_active`.
- `defects.md` entries updated per sub-PR.
