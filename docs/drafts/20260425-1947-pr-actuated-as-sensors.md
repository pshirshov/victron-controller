# PR-actuated-as-sensors — implementation plan

Drafted 2026-04-25 19:47 local.

## §1 Goal

Unify the actuated-readback ingestion pipeline with the sensor ingestion pipeline. Today there are two structurally identical paths:

- **Sensor path.** `Route::Sensor(SensorId)` → `Event::Sensor(SensorReading{ id, value: f64, at })` → `apply_sensor_reading` → `world.sensors.<field>.on_reading(v, at)`. Cadence and freshness keyed off `SensorId`.
- **Actuated readback path.** `Route::GridSetpointReadback / Route::CurrentLimitReadback / Route::ScheduleField{...}` → `Event::Readback(ActuatedReadback)` → `apply_readback` → `world.<entity>.actual.on_reading(...)` followed by `confirm_if(...)` and a `Publish(ActuatedPhase)`. Cadence lives in `ACTUATED_RESEED_*` constants in `subscriber.rs`; freshness lives in `ActuatedId::freshness_threshold()`.

The split is historical and adds maintenance cost: per-service cadence is computed twice, the NaN/Inf filter risk has two code paths, schedule-accumulator semantics live in two places. Collapse to one ingestion path and a small `SensorId → Option<ActuatedId>` post-hook.

Out of scope: myenergi-sourced (Zappi/Eddi) actuateds. They never enter the D-Bus pipeline — `crates/shell/src/myenergi/mod.rs` emits `Event::TypedSensor(TypedReading::{Zappi,Eddi})` directly, and the readback variants `ActuatedReadback::{ZappiMode,EddiMode}` are presently only constructed in tests. They stay as-is.

## §2 Audit

### 2.1 Consumers of `ActuatedReadback::*`

Single dispatch:

- `crates/core/src/process.rs:212-283` — `apply_readback` is the only function that destructures `ActuatedReadback`. Six arms: `GridSetpoint`, `InputCurrentLimit`, `ZappiMode`, `EddiMode`, `Schedule0`, `Schedule1`. Each does `world.<entity>.on_reading(...)` followed by `confirm_if(...)` followed by a `PublishPayload::ActuatedPhase` push on phase transition.
- `crates/core/src/process.rs:97` — the only call site, in `apply_event`.
- `crates/core/src/process.rs:2371, 2398, 2423` — three test-only constructions inside the `#[cfg(test)]` module (two `GridSetpoint`, one `ZappiMode`).
- `crates/core/tests/property_process.rs:121-122` — proptest constructor for `GridSetpoint`.
- `crates/core/src/lib.rs:26` — re-export.
- `crates/core/src/types.rs:637-644` — definition of the enum.

Producers (shell side):

- `crates/shell/src/dbus/subscriber.rs:1073-1099` — only emit site for D-Bus-sourced readbacks. The `route_to_event` free function turns `Route::GridSetpointReadback`, `Route::CurrentLimitReadback`, and the post-accumulator emit out of `Route::ScheduleField{...}` into `Event::Readback(ActuatedReadback::*)`.
- `crates/shell/src/dbus/subscriber.rs:1050-1052` — thin `Subscriber::route_to_event` wrapper around the free function.

The myenergi crate does **not** emit `Event::Readback`. ZappiMode/EddiMode readback flow uses `Event::TypedSensor(TypedReading::{Zappi,Eddi})` (`shell/src/myenergi/mod.rs:441,482`). The two `ActuatedReadback::{ZappiMode,EddiMode}` arms in `apply_readback` are dead in production today; they remain only because the type was defined symmetrically. They MUST stay (per non-goals: don't touch myenergi).

### 2.2 Sites that use `Route::*Readback` / `Route::ScheduleField`

All in `crates/shell/src/dbus/subscriber.rs`:

- `Route` enum definition: lines 87-95.
- `ScheduleSpecField` enum definition: lines 98-105.
- Routing-table construction: `routing_table()` (lines 109-181) — the four sites that insert non-`Sensor` routes are:
  - `s.vebus, "/Ac/In/1/CurrentLimit"` → `Route::CurrentLimitReadback` (line 145).
  - `s.settings, "/Settings/CGwacs/AcPowerSetPoint"` → `Route::GridSetpointReadback` (line 156).
  - `s.settings, ".../Schedule/Charge/{0,1}/{Start,Duration,Soc,Day,AllowDischarge}"` → `Route::ScheduleField{...}` (loop lines 163-178).
- Per-service cadence: `cadence_for_route` (lines 187-194) inspects every `Route` variant; the `Sensor` arm defers to `id.reseed_cadence()`, the three actuated arms read the static `ACTUATED_RESEED_*` constants.
- Module-level constants: `ACTUATED_RESEED_CURRENT_LIMIT`, `ACTUATED_RESEED_GRID_SETPOINT`, `ACTUATED_RESEED_SCHEDULE_FIELD` (lines 77-79).
- Schedule accumulator: `SchedulePartial` struct + `apply()` + `take_spec()` + `as_spec()` (lines 227-286). Called from `route_to_event` (subscriber.rs:1084-1099). The accumulators themselves live on `Subscriber::schedule_accumulators: [SchedulePartial; 2]` (line 310, init at 494, persistent across reconnects).
- `route_to_event` free fn signature takes `schedule_accumulators: &mut [SchedulePartial; 2]` (line 1063).

Subscriber tests: search the `#[cfg(test)] mod tests` block for assertions on `Route::GridSetpointReadback`, `Route::CurrentLimitReadback`, `Route::ScheduleField`, `ScheduleSpecField`, `SchedulePartial`, `take_spec`, `route_to_event`. Each is a candidate for either deletion or refactor (see §6).

### 2.3 Sites that read `ActuatedId::freshness_threshold()`

- `crates/core/src/process.rs:627, 630, 635, 638` — four calls in `apply_tick` for `GridSetpoint`, `InputCurrentLimit`, `Schedule0`, `Schedule1`.
- `crates/core/src/types.rs:365-379` — definition (panic arm for ZappiMode/EddiMode).

The Zappi/Eddi tick-decay path uses `params.freshness_myenergi` (process.rs:631-632). That stays.

## §3 New `SensorId` variants

The unification pulls four new variants into `SensorId`. Schedules are leaf-level (10 paths × 1 enum each — see §5 for why we go this way) — the spec mentions only `Schedule0Actual` / `Schedule1Actual`, but those readings cannot be represented as a single f64 since `ScheduleSpec` is a 5-field struct. The closest thing to "a sensor reading" for the schedule path is each leaf D-Bus field. The sensor table therefore gets 4 + 10 = 14 new variants in the simplest shape, OR 4 + 2 in a shape that adds a side channel (see §5). Pick (b) — leaf-level — recommended below.

### 3.1 Recommended set (leaf-level for schedules)

| New SensorId variant | D-Bus path (service / path) | Cadence | Staleness | Regime | Corresponds to ActuatedId |
|---|---|---|---|---|---|
| `GridSetpointActual` | settings / `/Settings/CGwacs/AcPowerSetPoint` | 5 s | 15 s | SlowSignalled (per user 2026-04-25: 5/15 to match controller-write cadence; settings-service per-service min becomes 5 s) | `GridSetpoint` |
| `InputCurrentLimitActual` | vebus / `/Ac/In/1/CurrentLimit` | 5 s | 15 s | SlowSignalled (organic on every IL change; accept fast-organic cadence so the per-service `min` for vebus stays at 5 s) | `InputCurrentLimit` |
| `Schedule0StartActual` | settings / `/Settings/CGwacs/BatteryLife/Schedule/Charge/0/Start` | 60 s | 180 s | ReseedDriven | (combined into `Schedule0`, see §5) |
| `Schedule0DurationActual` | settings / `.../Charge/0/Duration` | 60 s | 180 s | ReseedDriven | (combined into `Schedule0`) |
| `Schedule0SocActual` | settings / `.../Charge/0/Soc` | 60 s | 180 s | ReseedDriven | (combined into `Schedule0`) |
| `Schedule0DaysActual` | settings / `.../Charge/0/Day` | 60 s | 180 s | ReseedDriven | (combined into `Schedule0`) |
| `Schedule0AllowDischargeActual` | settings / `.../Charge/0/AllowDischarge` | 60 s | 180 s | ReseedDriven | (combined into `Schedule0`) |
| `Schedule1StartActual` … `Schedule1AllowDischargeActual` | settings / `.../Charge/1/{Start,Duration,Soc,Day,AllowDischarge}` | 60 s | 180 s | ReseedDriven | (combined into `Schedule1`) |

Cadence rationale: spec mandates 60 s reseed / 180 s staleness for grid setpoint and the schedule fields, and 5 s reseed / 15 s staleness for input current limit. All satisfy the universal `staleness ≥ 2 × reseed_cadence` invariant (180 ≥ 120, 15 ≥ 10).

Staleness invariant impact:
- `vebus` per-service `min`: today it's 5 s (driven by `OffgridPower / OffgridCurrent / VebusInputCurrent`). `InputCurrentLimitActual` at 5 s changes nothing.
- `settings` per-service `min`: today it's 300 s (`EssState`). After this PR it becomes 5 s — driven by `GridSetpointActual` (per user 2026-04-25). The schedule field variants stay at 60 s but no longer drive the per-service `min`. That's a 60× reseed increase on the settings service. `dbus-flashmq` republish budget is enforced per service; settings carries 12 paths after the PR (1 ESS + 1 grid setpoint + 10 schedule fields). At 5 s GetItems × 12 paths = ~2.4 republish/s — under the 3 republish/s ceiling but tight. **Risk: re-introduce the t≈15 s eviction signature PR-CADENCE was designed to avoid.** Mitigation: deploy and watch the heartbeat log; if eviction reappears, fall back to 30 s/90 s on `GridSetpointActual` (still well above the 300 s previous value) — captured as a follow-up risk in §9.

The `SensorId::ALL` array, `freshness_threshold()`, `regime()`, and `reseed_cadence()` impl arms each grow by 14 entries.

### 3.2 Alternative (rejected): a typed sensor variant for schedules

Add a new variant to `Event::TypedSensor` carrying the post-accumulator `ScheduleSpec`. Pros: only 4 new sensor IDs, schedule readback travels as one event. Cons: the accumulator output still needs a freshness-and-cadence story; it would *not* be a `SensorId`, so the per-service `min(reseed_cadence)` machinery wouldn't apply and we'd be back to a parallel mechanism — exactly what this PR is supposed to remove. Reject.

## §4 `SensorId → ActuatedId` table

The post-update hook maps the subset of sensor IDs that mirror an actuated readback. Drives `confirm_if(...)` and the `Publish(ActuatedPhase)` push. For non-actuated sensors (`BatterySoc`, etc.) the lookup returns `None` and the post-hook is a no-op.

```text
SensorId::GridSetpointActual           -> ActuatedId::GridSetpoint
SensorId::InputCurrentLimitActual      -> ActuatedId::InputCurrentLimit
SensorId::Schedule{0,1}{Start,Duration,Soc,Days,AllowDischarge}Actual
                                        -> (handled separately — see §5)
all other variants                      -> None
```

Implemented as a `const fn SensorId::actuated_id(self) -> Option<ActuatedId>` on the core type, with an explicit per-variant match (no `_ =>` arm) so future additions force a classification call.

**Decision (per user 2026-04-25): the `on_reading` + `confirm_if` block lives inside the sensor handler in `apply_sensor_reading`, not in a sibling post-hook.** Cleanest because there's exactly one place that handles a sensor reading, and any actuated mirror is a property of the sensor (the `actuated_id()` lookup), not a separate concern.

Concrete shape inside `apply_sensor_reading`:

```rust
fn apply_sensor_reading(reading: SensorReading, world: &mut World, ..., effects) {
    world.sensors.on_reading(reading.id, reading.value, reading.at);
    match reading.id.actuated_id() {
        None => {}
        Some(ActuatedId::GridSetpoint) => {
            #[allow(clippy::cast_possible_truncation)]
            let v = reading.value as i32;
            world.grid_setpoint.on_reading(v, reading.at);
            let tol = topology.controller_params.setpoint_confirm_tolerance_w;
            if world.grid_setpoint.confirm_if(|t,a| (*t - *a).abs() <= tol, reading.at) {
                effects.push(Effect::Publish(PublishPayload::ActuatedPhase {
                    id: ActuatedId::GridSetpoint,
                    phase: world.grid_setpoint.target.phase,
                }));
            }
        }
        Some(ActuatedId::InputCurrentLimit) => { /* same shape, f64, tol_a */ }
        Some(other) => {
            // ZappiMode/EddiMode/Schedule0/Schedule1 are not driven through
            // the SensorId pipeline — myenergi has its own path, and
            // schedules are handled by Event::ScheduleReadback (§5).
            debug_assert!(matches!(other,
                ActuatedId::ZappiMode | ActuatedId::EddiMode
                | ActuatedId::Schedule0 | ActuatedId::Schedule1));
        }
    }
}
```

Note the `world.<entity>.actual` storage is **separate** from `world.sensors.<field>.actual`. The unification is *ingestion*, not *storage*: the reading lands in both `world.sensors.grid_setpoint_actual` (new field) and `world.<entity>.actual` (existing). Two writes, one event. This avoids any change to controller code that reads `world.grid_setpoint.actual`.

Schedule fields do not have a 1:1 ActuatedId mapping — see §5.

## §5 Schedule accumulator strategy

The 5-field accumulator must continue to work because `Actuated<ScheduleSpec>::confirm_if` compares against a complete `ScheduleSpec`. Two reasonable shapes; recommend (B).

### 5.1 Shape A — accumulator inside the core (rejected)

Keep `SchedulePartial` accumulator in the core. Each leaf `Schedule{0,1}{Field}Actual` sensor reading lands in `world.sensors.<field>` and *also* updates a core-side `SchedulePartial` per index; on completion (all 5 present), call `world.schedule_<n>.on_reading(spec, at)` + `confirm_if`.

Pros: one ingestion path inside core; subscriber becomes purely "f64 in, sensor out".
Cons: introduces in-core mutable accumulator state and the `take_spec` semantics (defect A-12 — only emit a spec built from a coherent re-observation of all 5 fields). The Victron settings reseed delivers all 5 fields per `GetItems`, which means inside one tick we'd land 5 separate `Event::Sensor` events — accumulator drains cleanly. But a partial `ItemsChanged` (firmware quirk) would arrive as fewer events; the core accumulator must persist across events without resetting until either (a) a complete spec is observed or (b) a stale-out timeout fires. This drags accumulator persistence into the world struct and complicates the world's PartialEq snapshot.

### 5.2 Shape B — accumulator stays in subscriber + side-channel event (recommended)

Subscriber routing for the schedule fields becomes:
- Each leaf path is routed `Route::Sensor(Schedule0StartActual)` etc.
- The subscriber, after dispatching the `Event::Sensor(...)` for the leaf, *additionally* updates its existing `schedule_accumulators[idx]: SchedulePartial`. When `take_spec()` returns `Some(spec)`, the subscriber emits a *second* event: a new lightweight side channel that carries the completed `ScheduleSpec`.

Two implementations of the side channel:
- **B.1** Add `Event::ScheduleReadback { index: u8, value: ScheduleSpec, at: Instant }`. New event variant; `apply_event` gains one arm that calls `world.schedule_<n>.on_reading(spec, at)` + `confirm_if`. Schedule readback handling stays out of `apply_sensor_reading`. **Recommended.**
- **B.2** Reuse `Event::TypedSensor(TypedReading::Schedule { ... })`. Adds an arm to `TypedReading`. Slightly tidier (no top-level event variant) but conflates "actuated readback" with "non-scalar sensor" semantically.

Shape B keeps the accumulator wiring already proven against defect A-12 and avoids contaminating the world snapshot with partial state. The subscriber emits two events for every leaf field update: the per-field `Event::Sensor(...)` (which lands in `world.sensors.<field>` and feeds the per-field freshness/cadence machinery) and, on completion, the rolled-up `Event::ScheduleReadback` (which feeds `world.schedule_<n>.actual` + `confirm_if`).

The 14 leaf SensorIds give us the *cadence* unification for free (per-service `min` over routes drives the settings service to 60 s); the rolled-up event gives us the *storage + confirm_if* unification. Together: route table speaks only `Route::Sensor`, `apply_readback` is deleted, but a single new event variant remains for the schedule-spec rollup.

If the user judges the new variant ugly: collapse both `GridSetpoint` and `InputCurrentLimit` (which *are* expressible as f64) into the new `apply_event` flow via `actuated_id()`, and treat `Event::ScheduleReadback` as a deliberate, isolated affordance for "actuated whose payload doesn't fit a scalar". One special-case event is cheaper than refusing to touch what's working.

### 5.3 Decision: Shape B.1

Subscriber retains `schedule_accumulators` and `SchedulePartial` exactly as today. `Route::ScheduleField` and `ScheduleSpecField` are deleted; their work is split into:
- 10 new `Route::Sensor(Schedule{0,1}{Field}Actual)` rows.
- A subscriber-internal lookup table `SensorId → Option<(idx, ScheduleSpecField)>` that mirrors the deleted `Route::ScheduleField` cases. After the routed `Event::Sensor(...)` is sent, the subscriber consults this table; if it hits, it calls `acc.apply(field, value)` and on completion pushes `Event::ScheduleReadback`. This is the only place in the subscriber that knows about the schedule rollup.

## §6 Test plan

### 6.1 Tests to migrate

- `apply_readback` direct-construction tests at `process.rs:2371, 2398` (two `GridSetpoint` cases): rewrite as `Event::Sensor(SensorReading{ id: SensorId::GridSetpointActual, value, at })` and assert the same world transitions (`world.grid_setpoint.target.phase == Confirmed`, `Publish(ActuatedPhase)` effect produced).
- `apply_readback` test at `process.rs:2423` (`ZappiMode`): keep verbatim. ZappiMode/EddiMode readback variants stay (non-goal: no myenergi changes). Test must still compile and pass.
- `tests/property_process.rs:121-122`: `Event::Readback(ActuatedReadback::GridSetpoint{...})` strategy → `Event::Sensor(SensorReading{ id: GridSetpointActual, value: f64::from(v), at })`. Adjust the value range (proptest currently generates `i32` in `-5000..5000`; map through `f64::from` for the SensorReading).

### 6.2 New tests

- Subscriber: `routing_table_routes_grid_setpoint_as_sensor` — assert the (settings, AcPowerSetPoint) entry resolves to `Route::Sensor(SensorId::GridSetpointActual)`.
- Subscriber: `routing_table_routes_current_limit_as_sensor` — same for vebus path.
- Subscriber: `routing_table_routes_each_schedule_field_as_sensor` — 10 rows.
- Subscriber: `schedule_accumulator_emits_rollup_after_five_fields` — drive the per-field `Event::Sensor(...)`s and assert exactly one `Event::ScheduleReadback{ index: 0, value: spec, ... }` follows the fifth distinct leaf. Already-extant `take_spec` semantics test (defect A-12) ports verbatim against the new emit shape.
- Subscriber: `schedule_accumulator_resets_after_emit` — after a successful rollup, the next single-field update does **not** re-emit a rollup using the previously-observed values. Mirrors the defect A-12 test.
- Core: `sensor_id_to_actuated_id_mapping` — assert `actuated_id()` returns the right `Option<ActuatedId>` for every variant. Explicit match in the test, paralleling the impl, so a new variant fails compile until classified.
- Core: `apply_event_grid_setpoint_actual_confirms_target` — integration-style. Send the matching sensor reading after a grid setpoint write; assert phase transitions Pending → Commanded → Confirmed and an `ActuatedPhase` publish is produced.
- Core: `apply_event_current_limit_actual_confirms_target` — same shape for current limit.
- Core: `apply_event_schedule_readback_confirms_target` — drive an `Event::ScheduleReadback` and assert the `world.schedule_0` phase transitions and publish.
- Core: `freshness_threshold_invariant_holds_for_every_sensor` (already exists in `types.rs`) — extends to cover the 14 new variants. Each new variant's expected cadence/regime added to the per-variant match table inside the test (this test deliberately mirrors the impl to catch silent drift; cf. PR-cadence-per-sensor §5).
- Core: `apply_tick_decays_grid_setpoint_actuated_freshness` — replace the test that expected `ActuatedId::GridSetpoint.freshness_threshold()` to drive decay. After this PR, decay is driven by `SensorId::GridSetpointActual.freshness_threshold()` (180 s); the existing `Actuated<i32>::actual` decay path no longer needs an explicit `apply_tick` arm (see §7 cleanup). The test asserts that `world.grid_setpoint.actual` goes Fresh → Stale at 180 s.

### 6.3 Tests to delete

- Anything in `subscriber.rs` tests that asserts `Route::GridSetpointReadback` / `Route::CurrentLimitReadback` / `Route::ScheduleField` shape directly (instead of behaviour).
- Any direct test of `cadence_for_route` for the actuated arms — replaced by the per-`SensorId` cadence test machinery already in `types.rs`.
- The `ACTUATED_RESEED_*` constants vanish; any test that imports them fails compile and is deleted.

## §7 Code-level deletions and additions

Deletions:

- `crates/core/src/types.rs:637-644` — `ActuatedReadback` enum (after migration; see §8 sequencing — this can land last).
- `crates/core/src/types.rs:664` — `Event::Readback(ActuatedReadback)` variant.
- `crates/core/src/types.rs:349-380` — `ActuatedId::freshness_threshold()` impl. Callers in `apply_tick` (process.rs:627, 630, 635, 638) are deleted along with the explicit per-actuated `tick(...)` calls; freshness for the storage-side `Actuated<V>::actual` is driven by the universal sensor-decay loop (already iterating every `SensorId` via the matching `world.sensors.<field>.tick(...)` call), and a parallel iteration over the actuated `world.<entity>.actual` slots uses the *same* `SensorId::freshness_threshold()` looked up via the new `SensorId → ActuatedId` map, in reverse: for each actuated entity, find the SensorId that mirrors it, and use its freshness threshold. Single source of truth.
- `crates/core/src/process.rs:212-283` — `apply_readback` function.
- `crates/core/src/process.rs:97` — call site.
- `crates/shell/src/dbus/subscriber.rs:77-79` — `ACTUATED_RESEED_*` constants.
- `crates/shell/src/dbus/subscriber.rs:87-105` — `Route::GridSetpointReadback`, `Route::CurrentLimitReadback`, `Route::ScheduleField`, and the `ScheduleSpecField` enum.
- `crates/shell/src/dbus/subscriber.rs:1071-1099` — readback emit branches in `route_to_event` (replaced by sensor-emit + accumulator update for the schedule case).
- `crates/shell/src/dbus/subscriber.rs:187-194` — the actuated arms of `cadence_for_route`. Function reduces to `route.sensor_id().reseed_cadence()`.

Additions:

- 14 new arms in `SensorId` + matching arms in `freshness_threshold()`, `regime()`, `reseed_cadence()`, and `ALL`.
- `const fn SensorId::actuated_id(self) -> Option<ActuatedId>`.
- New `Event::ScheduleReadback { index: u8, value: ScheduleSpec, at: Instant }` and matching arm in `apply_event`.
- `world.sensors` gains `grid_setpoint_actual: Actual<f64>`, `input_current_limit_actual: Actual<f64>`, and 10 `schedule_*_actual: Actual<f64>` fields (so `Sensors::by_id` covers them; pure observability — controllers don't read these). Cleaner alternative: skip those fields and have `Sensors::by_id` return `Actual::unknown(now)` for the actuated-mirror variants, with a comment that the storage of truth is `world.<entity>.actual`. Recommend the cleaner alternative — fewer dead fields. The dashboard's `SensorBroadcastCore` should also be configured to skip these in publishes (or to publish them, if HA discovery should include them — open design question, default skip).
- A small dispatch in `apply_event` after `apply_sensor_reading`: if `id.actuated_id()` is `Some`, call the corresponding `world.<entity>.on_reading + confirm_if` block. Pure code reorganisation — the logic is lifted verbatim from `apply_readback`.

## §8 Migration sequence

Recommend three PRs.

**PR-A (infra).** Add `SensorId::actuated_id()`, the new `SensorId` variants, the new `Event::ScheduleReadback` variant, and the `apply_event` post-hook. **Do NOT** delete `ActuatedReadback`/`Event::Readback`/`apply_readback` yet. Both pipelines run in parallel; if a reading arrives both as the new `Event::Sensor(...)` *and* as the old `Event::Readback(...)`, the world is updated twice idempotently — `Actual::on_reading` is fine to call repeatedly. This PR doesn't change subscriber routing — old routes continue to emit `Event::Readback`. Pure additive change to the core.

**PR-B (subscriber routing).** Switch the routing table over: delete `Route::GridSetpointReadback`, `Route::CurrentLimitReadback`, `Route::ScheduleField`, `ScheduleSpecField`. New rows emit `Route::Sensor(...)` plus, for schedule paths, a side-channel rollup. Delete `ACTUATED_RESEED_*`. Per-service `min` cadence falls out of the per-`SensorId` cadence machinery. After this PR, `Event::Readback` is no longer constructed by the D-Bus subscriber; only myenergi (still doesn't use it) and tests do.

**PR-C (cleanup).** Delete `apply_readback`, the `Event::Readback` variant, the `ActuatedReadback` enum, `ActuatedId::freshness_threshold`, and the four explicit `apply_tick` actuated decay calls (replaced by the universal sensor decay loop driving the actuated `actual` via the reverse lookup). Migrate the three remaining tests in `process.rs` and the proptest. The Zappi/Eddi `ActuatedReadback::{ZappiMode,EddiMode}` arms in `apply_readback` are dead code today; we either keep them (in which case `apply_readback` survives, just shrinks to two arms) or — cleaner — also migrate the ZappiMode test in `process.rs:2423` to use a TypedReading, which is already the production code path. Cleaner option recommended.

Splitting matters because PR-B is the only behavioural change with a possibility of regressing the post-write reseed kick (new emit shape on the same channel) — landing it alone, after PR-A has bedded in the parallel infrastructure, makes a revert trivial. PR-C is a bottom-line dead-code purge once nothing else references the old types.

If the appetite is for a single PR: doable, but the testing surface is larger and a regression is harder to diagnose. Recommend the three-PR split.

## §9 Risks

1. **Schedule accumulator semantics drift.** Defect A-12's `take_spec` discipline (only emit a spec on a *complete fresh re-observation* of all 5 fields) is load-bearing. Shape B.1 keeps the accumulator inside the subscriber, in the same place and with the same lifecycle, so the risk is bounded — but the test that pins the discipline must port to the new `Event::ScheduleReadback` shape, not silently drop. Mitigation: explicit test in §6.2.

2. **Post-write reseed integration.** `ReseedTrigger` keys by service well-known name and is independent of `Route`/`Event` — should compose naturally. Confirm by inspection: `crates/shell/src/dbus/writer.rs:130-135` calls `trigger.kick(&svc)` after a successful `SetValue`; the receiving end (`subscriber.rs:1010-1038`) calls `seed_service`, which in turn calls `route_to_event` per path. After PR-B, `route_to_event` emits `Event::Sensor` for the actuated paths and the side-channel `Event::ScheduleReadback` for schedule rollups — the write-side caller's contract (kick by service) is unchanged. Test: `post_write_kick_emits_grid_setpoint_actual_sensor_event` — write to `GridSetpoint`, kick, verify the next subscriber emit is `Event::Sensor(SensorReading{ id: GridSetpointActual, ... })` rather than `Event::Readback(...)`.

3. **Freshness-floor invariant breaks.** All four (or 14) new SensorId variants must satisfy `staleness ≥ 2 × reseed_cadence`. Pre-checked: 180 ≥ 120, 15 ≥ 10. The unit test `freshness_threshold_invariant_holds_for_every_sensor` enforces this on every build.

4. **Settings-service cadence change.** Today's settings reseed is 300 s (driven by `EssState`). After PR, settings reseeds at **5 s** — driven by `GridSetpointActual` (per user choice 2026-04-25). That's a 60× increase. `dbus-flashmq`'s 3 republish/s ceiling is per-service; settings carries ~12 paths after the PR, putting us at ~2.4 republish/s — under but tight. Watch the heartbeat log on initial deploy for the t≈15 s eviction signature PR-CADENCE/PR-URGENT-20 was designed to avoid; if it appears, fall back to 30 s/90 s on `GridSetpointActual` (still much faster than the previous 300 s ceiling and the post-write reseed kick from commit `282c70e` already covers same-tick latency).

5. **Dead `ActuatedReadback::{ZappiMode,EddiMode}` arms.** They survive into PR-C unless the ZappiMode test at `process.rs:2423` is also migrated. Migrating that test to `Event::TypedSensor(TypedReading::Zappi{...})` is the cleanest path; it does not touch the myenergi production code (the test was already a fiction — production never constructs the `ActuatedReadback::ZappiMode` variant). Acceptable change to the non-goal "don't touch the myenergi ingestion path", because the *production* myenergi path is genuinely untouched.

6. **`Sensors::by_id` semantics.** The 14 new variants logically have `Actual<f64>` values backed by `world.<entity>.actual`, not by an additional `world.sensors.<field>`. The `Sensors::by_id` lookup must return *something*. Two options: (a) add stub `Actual<f64>` fields to `Sensors` and write to them in parallel (wasteful storage; risk of drift); (b) special-case in `Sensors::by_id` to return the actuated-side actual (cast i32 → f64 for `GridSetpoint`; for schedule leaf fields, project the right field of `Actuated<ScheduleSpec>::actual`). Option (b) is the better source-of-truth design; option (a) is the simpler patch. Recommend (b) with a clear doc comment. If `SensorBroadcastCore` consumes `by_id` and we don't want to publish actuated-mirror sensors over MQTT, gate them inside `SensorBroadcastCore` (or its core registration) rather than inside `by_id`.

## §10 Acceptance criteria for the resulting PR(s)

The PR-A acceptance criteria:
- New `SensorId` variants present with arms in `ALL`, `freshness_threshold`, `regime`, `reseed_cadence`, `actuated_id`.
- `freshness_threshold_invariant_holds_for_every_sensor` passes.
- New `apply_event` post-hook covers `GridSetpointActual`, `InputCurrentLimitActual`, `Schedule0/1*Actual` (latter via `Event::ScheduleReadback`).
- All existing tests pass.
- No subscriber changes; no production ingestion-path changes.

The PR-B acceptance criteria:
- `Route::*Readback` and `Route::ScheduleField` deleted; `ScheduleSpecField` deleted; `ACTUATED_RESEED_*` deleted.
- Subscriber emits `Event::Sensor(SensorReading{ id: <new variant>, ... })` for the four actuated paths.
- Subscriber emits `Event::ScheduleReadback{...}` after each complete 5-field rollup; `take_spec` resets between rollups (defect A-12 invariant).
- Per-service `min` cadence on `settings` is 60 s; on `vebus` is 5 s.
- Post-write reseed kick (writer → trigger → subscriber → emit) end-to-end test passes for `GridSetpoint`, `InputCurrentLimit`, and a schedule field.
- `Event::Readback` is still defined but no longer constructed in production code.

The PR-C acceptance criteria:
- `ActuatedReadback`, `Event::Readback`, `apply_readback`, `ActuatedId::freshness_threshold` all deleted.
- The 4 explicit per-actuated `apply_tick` decay calls replaced by the universal sensor-decay loop.
- All `world.<entity>.actual` slots decay on the same threshold table as before (no behaviour change).
- All tests migrated; no `#[allow(dead_code)]` shimming.
