# M-ZAPPI-DRAIN-OBS — observability for the compensated-drain loop

**Milestone:** M-ZAPPI-DRAIN-OBS
**Status:** planned → ready to execute
**Driving feedback (user, 2026-04-30):** "we have a major control loop in
production. I wonder if we can track its details in real time. Could we
add charts/stats on the details page?"

This milestone ships **observability only** for the M-ZAPPI-DRAIN soft
loop + Fast-mode hard clamp. Two surfaces are added in lockstep:

- **Option A — HA broadcast**: three new derived sensors
  (`controller.zappi-drain.compensated-w`,
  `controller.zappi-drain.tighten-active`,
  `controller.zappi-drain.hard-clamp-active`) flowing through
  `SensorBroadcastCore` and HA discovery. Operator can wire them into
  HA dashboards / automations.
- **Option C — in-dashboard chart**: a new Detail-tab section showing
  current snapshot (3 big-number widgets + branch tag), 30-minute
  sparkline of `compensated_drain_w` colour-coded by branch, with
  reference lines at `threshold_w` (orange dashed) and `hard_clamp_w`
  (red dashed).

**Read-only invariant (locked):** the new observables consume what
`evaluate_setpoint` and the `run_setpoint` hard-clamp block already
computed; no re-derivation, no feedback into any controller decision.
Snapshot capture is folded into `run_setpoint` itself so wire-output and
broadcast-input cannot drift.

**Orchestrator-locked decisions** (open questions from §7 resolved
2026-04-30):
1. MQTT topic prefix → `controller/<name>/state` (new namespace).
2. `tighten-active` during Bypass / Disabled → `false` (boolean,
   not tri-state).
3. Ring buffer depth → N=120 (30 min).
4. Per-sample timestamps → wall-clock epoch_ms; chart sorts at render.
5. Polyline interpolation → linear (matches SoC chart convention).

---

## 1. Goal

Surface, in real time, what the M-ZAPPI-DRAIN compensated-drain loop is
doing on every controller tick:

- the input signal (compensated_drain_w),
- which branch fired this tick (Tighten / Relax / Bypass / Disabled),
- whether the Fast-mode hard clamp engaged,
- a 30-minute history of (1) so the operator can see drift / oscillation
  / steady-state behaviour without scraping logs.

The control loop itself is unchanged. This is pure observability.

---

## 2. Out-of-scope (locked)

- **Persistence across restarts.** The 120-sample ring buffer resets on
  `World::fresh_boot`. Operator can scrape HA's MQTT recorder for
  multi-restart history if needed; the in-dashboard chart is a 30 min
  rolling window only.
- **Historical query API.** No `GET /history`, no time-range selectors,
  no zoom or pan in the chart. The HA broadcast feeds HA's recorder for
  longer-term retention; that's the supported answer.
- **Alerting / threshold-violation detection.** Operator can build HA
  automations on the broadcast sensors. Controller-side does not raise
  alerts.
- **Drill-down per-tick decision factors in the chart.** The existing
  Decisions table on the Control tab already shows `hard_clamp_engaged`
  / `compensated_drain_W` factors for the latest tick; no per-sample
  factor history.
- **Frontend buffer reseeding from MQTT replay.** On a dashboard reload
  the wire-format buffer rehydrates from the next snapshot — there is
  no client-side persistence either.
- **Wire-format version bump.** Per CLAUDE.md "Deployment topology",
  additive within v0.3.0. New `ZappiDrainState` block + new
  `WorldSnapshot.zappi_drain_state` field; no migrations.
- **Coupling the soft-loop or hard-clamp to the buffer.** Reviewer will
  probe this: the buffer is *write-only* from the controller's
  perspective and *read-only* from observability's perspective. Tests
  PR-ZDO-1.T6 and PR-ZDO-1.T7 lock this in.

---

## 3. Locked design decisions (cross-cutting, do not relitigate)

| Decision | Value | Rationale |
|---|---|---|
| Ring-buffer depth `N` | 120 | 30 min @ 15 s tick; ~32 B/sample × 120 ≈ 4 KB; trivial. |
| Buffer policy | FIFO append, evict oldest at `len == N` | `VecDeque<ZappiDrainSample>` with `pop_front` on overflow. |
| Buffer reset | `World::fresh_boot` only | No persistence. |
| Snapshot capture point | Inside `run_setpoint`, after `evaluate_setpoint` returns and after the hard-clamp block decides `(hard_clamped_target, hard_clamp_engaged, hard_clamp_excess)`, BEFORE `update_bookkeeping_from_setpoint` and before `maybe_propose_setpoint`. | Lockstep — capture sees exactly what the controller emitted this tick. |
| Branch classification | Pure helper `classify_zappi_drain_branch(world)` returning `ZappiDrainBranch`. Mirrors the `if/else if` ladder in `evaluate_setpoint`. `// LOCKSTEP:` comment on both sites. | Locked by test PR-ZDO-1.T3. |
| Branch enum (4 values) | `Tighten` / `Relax` / `Bypass` / `Disabled` | `Tighten`: drain > threshold + zappi_active + !allow. `Relax`: drain ≤ threshold + zappi_active + !allow. `Bypass`: allow_battery_to_car=true OR force_disable_export. `Disabled`: zappi_active=false. |
| Sensor naming | `controller.zappi-drain.compensated-w` / `.tighten-active` / `.hard-clamp-active` | New `controller/<name>/state` MQTT topic root. |
| Stale-as-unavailable | Reuses `encode_sensor_body` for `compensated-w` numeric. Booleans are always-meaningful (`false` is honest). | HA recogniser unchanged. |
| Honesty under observer mode | `writes_enabled=false` does NOT short-circuit capture. | Locked by test PR-ZDO-1.T4. |
| Per-sample timestamp | `clock.wall_clock_epoch_ms()`. Renderer sorts at draw time. | Matches SoC-chart convention. |
| Polyline interpolation | Linear between samples. | Matches SoC-chart visual convention. |

---

## 4. PR breakdown

Four PRs, sequenced. PR-ZDO-1 is pure backend capture (no wire surface
yet). PR-ZDO-2 ships the HA broadcast (Option A). PR-ZDO-3 ships the
dashboard wire-format (Option C, backend half). PR-ZDO-4 ships the
frontend chart (Option C, frontend half). Only PR-ZDO-1 is a hard
prerequisite for the others.

---

### 4.1 PR-ZDO-1 — Capture pipeline (Core)

**Scope.** Add `ZappiDrainState` + `ZappiDrainSnapshot` + `ZappiDrainSample`
+ `ZappiDrainBranch` types in `core`. Hook capture into `run_setpoint`
in `process.rs` immediately after the hard-clamp block. Append to a
`VecDeque<ZappiDrainSample>` (capacity 120, FIFO eviction). Pure backend;
no broadcast or wire-format yet.

**Files touched.**

- `crates/core/src/world.rs` — new types: `ZappiDrainBranch`,
  `ZappiDrainSnapshot`, `ZappiDrainSample`, `ZappiDrainState` with
  `RING_CAPACITY: usize = 120` and a `push(&mut self, snap: ZappiDrainSnapshot)`
  helper that mirrors `latest` and FIFO-evicts on the deque. New
  `pub zappi_drain_state: ZappiDrainState` field on `World`. Init in
  `fresh_boot`.
- `crates/core/src/types.rs` — `pub enum ZappiDrainBranch { Tighten,
  Relax, Bypass, Disabled }` with `Display`/`Debug`/`Copy`/`Clone`/`Eq`/`Hash`
  derives plus `name(&self) -> &'static str`. Place near sibling enums.
- `crates/core/src/process.rs` —
  - New helper `fn classify_zappi_drain_branch(world: &World) -> ZappiDrainBranch`
    near `compensated_drain_w` (`process.rs:1159`). Mirrors the
    `evaluate_setpoint` if/else ladder in this exact order:
    1. `force_disable_export` → Bypass
    2. `!zappi_active` → Disabled
    3. `allow_battery_to_car` → Bypass
    4. `compensated_drain > threshold` → Tighten
    5. else → Relax
  - In `run_setpoint`, after the hard-clamp block (`process.rs:1327-1338`)
    and BEFORE the grid-cap clamp (`process.rs:1340`), insert the capture:
    ```rust
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
    ```
  - Capture also runs from `apply_setpoint_safety` (idle-fallback): records
    `branch = Disabled`, `compensated_drain_w = 0.0`, `hard_clamp_engaged = false`.
    Honest signal that the loop didn't run.
  - Honesty: capture happens unconditionally of `world.knobs.writes_enabled`.

**Acceptance criteria.**

- `cargo test --workspace` green; new tests pass.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo build --target armv7-unknown-linux-gnueabihf --release` green.

**Risks / failure modes.**

- **Lockstep drift between `classify_zappi_drain_branch` and `evaluate_setpoint`.**
  A future PR adds a new branch to `evaluate_setpoint` but forgets the
  mirror update → branch label becomes wrong silently. Mitigation:
  PR-ZDO-1.T3 covers all four branches; `// LOCKSTEP:` cross-reference
  comments on both sites.
- **Clock-skew on `captured_at_ms`.** GX clock jumps backwards →
  ring-buffer monotonicity assumption breaks. Renderer must
  tolerate non-monotonic samples (sort by `captured_at_ms` at render).
- **Memory growth on `fresh_boot` storm.** `VecDeque::with_capacity(120)`
  amortised O(1). No risk.
- **Capture firing when controller didn't run.** `apply_setpoint_safety`
  also captures, recording `branch = Disabled`. Operator sees flat
  lines during safety fallback rather than stale frozen value.

**Test plan (≥ 6 new tests in `crates/core/src/process.rs::tests`).**

- **PR-ZDO-1.T1** `zappi_drain_capture_records_compensated_w_matching_controller`
  — Fast + !allow + battery=-2500 + HP=0 + cooker=0 + threshold=1000.
  Run `run_setpoint`. Assert `world.zappi_drain_state.latest.unwrap().compensated_drain_w == 2500.0`.
- **PR-ZDO-1.T2** `zappi_drain_capture_ring_buffer_evicts_at_120` — push
  130 samples; assert `samples.len() == 120` and oldest 10 evicted.
- **PR-ZDO-1.T3** `branch_classification_matches_evaluate_setpoint_branch_ladder`
  — table-driven, 5 sub-scenarios (one per code path: Tighten, Relax,
  Bypass-via-allow, Bypass-via-force, Disabled). Each builds a `World`
  matching the precondition; runs `run_setpoint`; asserts branch label.
- **PR-ZDO-1.T4** `zappi_drain_capture_honest_under_observer_mode` —
  `writes_enabled = false`, run with Tighten precondition. Assert
  snapshot still records `branch == Tighten` and
  `compensated_drain_w == 2500.0`.
- **PR-ZDO-1.T5** `zappi_drain_capture_lockstep_with_hard_clamp_engagement`
  — Fast + !allow + drain=500 + hard_clamp=200. Run `run_setpoint`.
  Assert `latest.hard_clamp_engaged == true`,
  `latest.hard_clamp_excess_w == 300.0`, `branch == Tighten` (drain >
  threshold). Same tick.
- **PR-ZDO-1.T6** `zappi_drain_capture_does_not_feed_back_into_setpoint`
  — run once (capture happens). Capture
  `world.grid_setpoint.target.value`. Mutate
  `world.zappi_drain_state.latest = None` (or push synthetic garbage
  snapshot). Run with identical inputs. Assert
  `world.grid_setpoint.target.value` is identical.
- **PR-ZDO-1.T7** `zappi_drain_capture_buffer_resets_on_fresh_boot` —
  push 50 samples; call `World::fresh_boot(now)`; assert
  `samples.len() == 0` and `latest.is_none()`.

---

### 4.2 PR-ZDO-2 — HA broadcast sensors (Option A)

**Scope.** Add a third broadcast class alongside Sensor / BookkeepingNumeric
/ BookkeepingBool. Three new entries:
- `controller.zappi-drain.compensated-w` (numeric, W, freshness-aware)
- `controller.zappi-drain.tighten-active` (bool)
- `controller.zappi-drain.hard-clamp-active` (bool)

Wire through `SensorBroadcastCore`, dedup cache, MQTT serialise,
HA discovery. Frontend display-name + descriptions follow.

**Files touched.**

- `crates/core/src/types.rs` — new `pub enum ControllerObservableId
  { ZappiDrainCompensatedW, ZappiDrainTightenActive, ZappiDrainHardClampActive }`
  with `name() -> &'static str` returning the dotted topic-tail.
  Two new `PublishPayload` variants: `ControllerNumeric { id, value, freshness }`
  (uses `encode_sensor_body`) and `ControllerBool { id, value }`.
  - Boolean broadcasts always-meaningful — `false` is honest pre-first-tick
    output. Numeric goes `unavailable` when `latest.is_none()`.
- `crates/core/src/world.rs` — extend `PublishedCache` with
  `controller_numeric: HashMap<ControllerObservableId, String>` and
  `controller_bool: HashMap<ControllerObservableId, bool>` (mirror
  existing dedup patterns).
- `crates/core/src/core_dag/cores.rs` — extend `SensorBroadcastCore::run`
  with a fourth block "Controller observables" reading
  `world.zappi_drain_state.latest`. Three publishes, dedup-on-wire-body.
  When `latest.is_none()`: numeric → `unavailable`, booleans → `false`.
- `crates/shell/src/mqtt/serialize.rs` — `encode_publish_payload` two
  new arms for `ControllerNumeric`/`ControllerBool` → topic
  `controller/<name>/state`. Round-trip tests near
  `serialize.rs:1309`.
- `crates/shell/src/mqtt/discovery.rs` — `publish_controller_observables`
  fn (mirror of `publish_bookkeeping`). Three configs: 1× `sensor`
  (compensated-w with unit `W`, `device_class: "power"`,
  `state_class: "measurement"`); 2× `binary_sensor`. Wire into top-level
  `publish_discovery` (`discovery.rs:124-135`).
- `web/src/displayNames.ts` — three entries (snake_case → dotted).
- `web/src/descriptions.ts` — three entries explaining each, calling out
  "broadcast-only — also visible on the Detail tab chart."

**Acceptance criteria.**

- `cargo test --workspace` green.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cd web && ./node_modules/.bin/tsc --noEmit -p .` clean.
- `cargo build --target armv7-unknown-linux-gnueabihf --release` green.
- HA discovery: three new entities (1 sensor + 2 binary_sensor) under
  the `victron-controller` device.
- `mosquitto_sub -t 'victron-controller/controller/#'` shows three
  retained-then-live topics flowing on every tick (subject to dedup).

**Risks / failure modes.**

- **Topic collision.** Reserve `controller/` namespace. Document in
  discovery-publish doc-block: future controller-derived observables
  (setpoint decision tag, schedule activation flags) ride this prefix.
- **Dedup cache growth.** 3 variants; bounded.
- **Boolean stale-as-unavailable.** Decision: booleans don't flip to
  `unavailable` even when snapshot is None — they publish `false`.
  Numeric uses `unavailable` because `0 W` would be a real reading.

**Test plan (≥ 4 new tests).**

- **PR-ZDO-2.T1** `controller_observables_publish_on_first_tick` — fresh
  World, run `run_setpoint` once with Tighten scenario, run
  `SensorBroadcastCore`. Assert three `Effect::Publish` variants emitted.
- **PR-ZDO-2.T2** `controller_observables_dedup_on_unchanged_state` —
  run twice with identical inputs. Second run emits 0 publishes.
- **PR-ZDO-2.T3** `controller_observables_compensated_w_unavailable_when_no_capture`
  — fresh World, no `run_setpoint`. Run broadcast. Assert
  `compensated-w` body is `"unavailable"`. Booleans publish `false`.
- **PR-ZDO-2.T4** `controller_observables_round_trip_through_serialize`
  — for each `ControllerObservableId`, encode + assert topic + body.
- **PR-ZDO-2.T5** `freshness_aware_publish_for_compensated_w` — set
  snapshot drain=1500, broadcast → publishes `"1500"`. Clear snapshot,
  re-broadcast. Body flips to `"unavailable"`.

---

### 4.3 PR-ZDO-3 — Wire format + dashboard data plumbing (Option C, backend half)

**Scope.** Extend baboon model: `ZappiDrainBranch` enum, `ZappiDrainSample`
data block, `ZappiDrainSnapshotWire` data block, `ZappiDrainState` data
block, `WorldSnapshot.zappi_drain_state` field. Plumb through
`world_to_snapshot`. No frontend rendering yet.

**Files touched.**

- `models/dashboard.baboon` — additive within v0.3.0:
  ```
  enum ZappiDrainBranch {
    Tighten
    Relax
    Bypass
    Disabled
  }

  data ZappiDrainSample {
    captured_at_epoch_ms: i64
    compensated_drain_w: f64
    branch: ZappiDrainBranch
    hard_clamp_engaged: bit
  }

  data ZappiDrainSnapshotWire {
    compensated_drain_w: f64
    branch: ZappiDrainBranch
    hard_clamp_engaged: bit
    hard_clamp_excess_w: f64
    threshold_w: i32
    hard_clamp_w: i32
    captured_at_epoch_ms: i64
  }

  data ZappiDrainState {
    latest: opt[ZappiDrainSnapshotWire]
    samples: lst[ZappiDrainSample]
  }
  ```
  Extend `WorldSnapshot` with `zappi_drain_state: ZappiDrainState`.
  Run `scripts/regen-baboon.sh`. Fix compile errors in `convert.rs`.
- `crates/shell/src/dashboard/convert.rs` — import regenerated types.
  New helper `fn zappi_drain_state_to_model(s: &ZappiDrainState) ->
  ModelZappiDrainState`. Branch enum maps via small `match`. Add to
  `world_to_snapshot` (`convert.rs:324`).

**Acceptance criteria.**

- `scripts/regen-baboon.sh` clean.
- `cargo test --workspace` green; `world_to_snapshot_*` tests extended.
- `cargo build --target armv7-unknown-linux-gnueabihf --release` green.
- Browser dev-tools `console.log(snap.zappi_drain_state)` shows populated
  wire object after the controller has run a tick.

**Risks / failure modes.**

- **Wire size.** 120 samples × ~40 bytes ≈ 5 KB per snapshot. Comparable
  to existing soc_chart history. WebSocket comfortably accommodates.
- **Baboon `lst[T]` ordering** — generated TS preserves insertion
  order (oldest first). Document in helper.
- **`opt[ZappiDrainSnapshotWire]` rendering** — Frontend handles
  `null` latest in PR-ZDO-4.

**Test plan (≥ 2 new tests in convert.rs).**

- **PR-ZDO-3.T1** `dashboard_snapshot_surfaces_zappi_drain_state` — push
  5 samples, call `world_to_snapshot`, assert
  `samples.len() == 5` and `latest is Some` with matching values.
- **PR-ZDO-3.T2** `dashboard_snapshot_handles_empty_zappi_drain_state`
  — fresh World, no captures. Snapshot's `latest is None`,
  `samples.len() == 0`. No panic.

---

### 4.4 PR-ZDO-4 — Frontend rendering (Option C, frontend half)

**Scope.** New Detail-tab section above `#sensors`:
- 3 big-number widgets: current `compensated_drain_w` (W, 0 decimals);
  branch tag (text + colour-coded); `hard_clamp_engaged` indicator.
- Branch colours: Tighten=red `#d33`, Relax=green `#3a3`, Bypass=grey
  `#888`, Disabled=neutral `#555`.
- 30-min sparkline of `compensated_drain_w` (120-sample buffer).
- Reference lines: `threshold_w` (orange dashed), `hard_clamp_w` (red dashed).
- Sample colouring: each segment uses the colour of the *later* sample's
  branch (consistent with soc-chart segment colouring).

Reuse hand-rolled SVG idiom from `web/src/chart.ts` — no chart library.

**Files touched.**

- `crates/shell/static/index.html` — new `<section id="zappi-drain-section">`
  above `#sensors` (line 134).
- `crates/shell/static/style.css` — `.big-number-row`, `.big-number`,
  branch-colour classes, hard-clamp-engaged classes. Reuse existing
  dashed-line styles or add `.zd-threshold-line` / `.zd-hard-clamp-line`.
- `web/src/render.ts` — new exports:
  - `renderZappiDrainSummary(snap)` — populates 3 big-number slots from
    `snap.zappi_drain_state.latest`. When `latest is None`, all render
    `—` with neutral styling.
  - `renderZappiDrainChart(snap)` — hand-rolled SVG mirroring
    `chart.ts:renderSocChart`'s structure: viewBox, x-axis (-30 min
    to 0), y-axis (W), gridlines, threshold + hard-clamp dashed lines,
    polyline colour-coded per segment by branch.
- `web/src/index.ts` — in `applySnapshot`, after `renderSocChart`
  (line 167), add change-detected calls.
- `web/src/render.test.ts` — three new tests mirror existing pattern.

**Acceptance criteria.**

- `cd web && ./node_modules/.bin/tsc --noEmit -p .` clean.
- Populated buffer: Detail tab renders section with 3 big numbers + chart
  + reference lines.
- Empty buffer: `—` placeholders, no polyline; reference lines drawn.
- Live oscillation: with controller flipping Tighten ↔ Relax, polyline
  colour visibly alternates red ↔ green.

**Risks / failure modes.**

- **Re-render cost.** Chart re-renders once per second; 120 samples +
  simple polyline → SVG cost negligible.
- **Y-axis auto-scaling.** Default `[0, max(samples,
  hard_clamp_w * 1.5)]` — keeps reference lines visible when actual
  drain is small.
- **Non-monotonic timestamps.** Renderer sorts by `captured_at_epoch_ms`
  before plotting.
- **Colour-blind accessibility.** Tighten/Relax red/green is the worst
  pair. Branch tag's text label complements colour; semantic
  information not lost. Per-sample shape distinction deferred.

**Test plan (≥ 3 new tests in render.test.ts).**

- **PR-ZDO-4.T1** `renderZappiDrainSummary_displays_latest_snapshot` —
  fixture with Tighten + drain=1500 + clamp engaged. Assert DOM text
  "1500 W" / "Tighten" / "Engaged" + correct CSS classes.
- **PR-ZDO-4.T2** `renderZappiDrainSummary_handles_empty_state` —
  `latest = null`. All slots render `—`; no branch-colour class.
- **PR-ZDO-4.T3** `renderZappiDrainChart_draws_polyline_and_reference_lines`
  — fixture with 5 samples. SVG contains 1 `<polyline>` with 5 vertices,
  2 dashed `<line>` (threshold, hard-clamp). Branch colour on each
  segment matches sample's branch.

---

## 5. Verification matrix

| Command | Expected |
|---|---|
| `cargo test --workspace` | `test result: ok. <N> passed`. Milestone target ≥16 new tests (6+5+2+3). |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean. |
| `cd web && ./node_modules/.bin/tsc --noEmit -p .` | clean. |
| `cargo build --target armv7-unknown-linux-gnueabihf --release` | green. |
| `scripts/regen-baboon.sh` (PR-ZDO-3 only) | clean regen. |

Per-PR live verification:
- PR-ZDO-1: unit-test `samples.len()` reaches 120 and stays.
- PR-ZDO-2: `mosquitto_sub -t 'victron-controller/controller/#'` shows
  three live topics flipping with branch transitions.
- PR-ZDO-3: browser dev-tools shows populated `snap.zappi_drain_state`.
- PR-ZDO-4: Detail tab section renders + chart updates every 15 s.

---

## 6. Rollout / safety notes

Observability-only — no control path mutated.

- **Chart misbehaves**: operator ignores. To fully hide: revert the static
  HTML section; controller untouched.
- **HA discovery clutters**: empty-payload retain on the discovery topic
  removes single entities; full disable via existing `[mqtt.discovery]
  enabled` knob.
- **Memory footprint**: 120-sample buffer × ~32 B/sample ≈ 4 KB.
- **Observer mode**: `writes_enabled=false` does not short-circuit
  capture (PR-ZDO-1.T4). Operator sees loop's *intent* on the chart.
