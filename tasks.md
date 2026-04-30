# victron-controller — Task Ledger

Authoritative ledger of planned and completed work. Spec: `SPEC.md` in repo
root. Audit findings (seeded 2026-04-24) live in `./defects.md` as `A-NN`
entries.

Status: `[ ]` planned · `[~]` in progress · `[x]` done · `[!]` blocked

---

## Milestones (high-level)

- [x] **M-AUDIT** — Drain the CRITICAL-tier of the 68 audit findings
  (A-01…A-68). All 8 CRITICAL findings closed 2026-04-24; remaining
  MAJOR/minor/nit backlog rolled into M-AUDIT-2.
- [~] **M-AUDIT-2** — Remaining backlog from M-AUDIT plus new regressions
  surfaced by field deployment of `df3ae4d`. Priority items:
  (1) **PR-DAG** — lift shared classifiers into proper TASS derivation
  cores with topological orchestrator + cycle-validating registry
  (the A-05 hazard is architecturally the wrong shape — two cores
  agreeing on a derivation is a smell; the derivation should be its
  own core);
  (2) **PR-SCHED0** — schedule_0 observed disabled post-`df3ae4d`
  even though `evaluate_schedules` unconditionally sets DAYS_ENABLED
  on it; determine root cause and lock invariant in a test.
- [~] **M-UX-1** — Dashboard UX, HA discovery expansion, and a
  staleness-floor correctness invariant. Plan in
  `./docs/drafts/20260425-0130-m-ux-1-plan.md`. Five PRs; correctness
  item lands first.
- [x] **M-AS** — Unify actuated-readback ingestion with the sensor
  pipeline; collapse `Event::Readback`/`apply_readback`/`Route::*Readback`
  into `Route::Sensor` + `Event::ScheduleReadback`. Plan in
  `./docs/drafts/20260425-1947-pr-actuated-as-sensors.md`. Three PRs:
  PR-AS-A (additive infra, `21db585`), PR-AS-B (subscriber routing
  switch, `d8f5249`), PR-AS-C (delete the old types, `78abebe`).
- [x] **M-ZAPPI-DRAIN-OBS** — Observability for the M-ZAPPI-DRAIN
  compensated-drain loop: three new HA broadcast sensors
  (`controller.zappi-drain.compensated-w` / `.tighten-active` /
  `.hard-clamp-active`) plus an in-dashboard Detail-tab chart with
  current-snapshot widgets and a 30-minute branch-coloured sparkline.
  Read-only; no control-loop coupling. Plan in
  `./docs/drafts/20260430-2000-m-zappi-drain-obs-plan.md`.
- [x] **M-ZAPPI-DRAIN** — Replace the PV-only Zappi-active export
  clamp with a closed-loop controller using compensated battery drain
  (`max(0, -battery_dc_power - heat_pump - cooker)`) as the feedback
  signal. Adds 4 sensors (HP/cooker via zigbee2mqtt MQTT, two MPPT
  op-modes via D-Bus, observability-only), 5 knobs (threshold, relax
  step, kp, target, hard-clamp). Plan in
  `./docs/drafts/20260429-1700-m-zappi-drain-plan.md`.

---

## Milestone M-AUDIT — PR breakdown

Detail in `./docs/drafts/20260424-0000-m-audit-plan.md` (to be written by the
planning subagent). One line per PR here; sub-task checklists + acceptance
criteria live in the plan doc. User's priority list (12 items) maps into the
following PRs:

- [x] **PR-01** — NaN / Inf / Bool filter in `extract_scalar` (resolves
  A-01, A-02).
- [x] **PR-02** — Grid-voltage ÷ 0 guard with upper+lower EN 50160 band
  (resolves A-03).
- [x] **PR-03** — Zappi `time_in_state` monotonic-Instant fix (resolves
  A-04, A-24). Shipped in commit `aab6c28`.
- [x] **PR-URGENT-14** — Dedup retained-knob bootstrap apply by topic.
  Resolves A-71. Field confirmed 5 retained topics × 57 redeliveries =
  287 applies; fix uses `HashSet<String>` to keep first-seen per topic
  within the bootstrap window. Completion log reports `applied`,
  `unique_topics`, `duplicates_suppressed`. Diagnostic warn! removed.
- [x] **PR-URGENT-13** — Silent stale-sensor observability fix (resolves
  A-69 + A-70; PR-URGENT-13-D01/D02 resolved; D03-D09 deferred).
  warn-level rate-limited re-seed failures + error escalation at 5
  consecutive fails; mpsc 256→4096 + 75% watermark warning; independent
  heartbeat interval with raw/routed signal counters. **Unblocks field
  diagnostics.**
- [x] **PR-04** — Canonical `classify_zappi_active` shared by
  `DerivedView` and `current_limit` (single source of truth); real
  forecast-derived `charge_battery_extended_today` bookkeeping with
  midnight reset; dropped `!disable_night_grid_discharge` term from
  cbe derivation. Resolves A-05, A-15; partially A-18 (500 W fallback
  now canonical across controllers).
- [x] **PR-05** — Observer → live transition invariant: controllers
  early-return without mutating target state when writes are
  suppressed; `KillSwitch(true)` edge-triggers reset of every
  actuated target so the next tick forces a fresh WriteDbus.
  Resolves A-06, A-07, A-59. **Last CRITICAL-tier audit item closed.**
- [x] **PR-06** — MQTT retained-knob range + NaN/Inf validation + A-49
  DischargeTime HH:MM:SS + `apply_knob` catch-all warn (resolves A-08,
  A-61, A-49). Parallel table drift (PR-06-D01) deferred.
- [x] **PR-07** — `GetNameOwner` re-resolution on `NameOwnerChanged`
  (resolves A-11). Shipped in commit `88d5412`.
- [x] **PR-08** — `SchedulePartial` accumulator clearing (resolves A-12).
  Shipped in commit `0cf4a18`.
- [x] **PR-09a** — Minimal setpoint clamp: `grid_import_limit_w` knob
  (default 10 W), symmetric `.clamp(-export_cap, +import_cap)`, pre/post-
  clamp Decision factors. Resolves the explicit user ask for a
  configurable [-5000, +10] W window.
- [x] **PR-09b** — `grid_export_limit_w` hardening (A-09, A-10, A-34,
  A-35). Shipped in commit `6c8c9c8`. Deadband i64 widening (A-31)
  shipped separately in `PR-setpoint-deadband-i64` (commit `9eb899f`).
- [x] **PR-10** — `force_disable_export` deleted from `current_limit`
  (A-19). Shipped in commit `b9e39b6`.
- [x] **PR-11** — Weather-SoC γ-hold honoured + once-per-day guard
  (A-20, A-21). Shipped in commit `3d9c987`. A-36 (eddi observer-mode
  dwell honesty) shipped in `PR-eddi-dwell` (commit `b6dd179`).
- [x] **PR-12** — myenergi HTTP body-level error parsing (A-22, A-23).
  Shipped in commit `a25bc15`.

Remaining audit items (A-13 Zappi auto-stop wiring; A-14 kWh/% unit fix;
A-16 forecast freshness filter; A-17/A-18 Hoymiles solar export + 500 W
`zappi_active` fallback; A-25–A-28 myenergi & forecast hardening; A-36
observer-mode `eddi_last_transition_at` honesty; A-38 MQTT connect log;
A-39 dashboard three-gate badge; A-41 fusion NaN filter; A-42 log_layer
comment; A-43 Open-Meteo efficiency knob; A-50 forecast TZ config;
A-53–A-56, A-58, A-60, A-62–A-68 hygiene + honesty) are rolled into
M-AUDIT-2 below; the planning subagent for each PR decides which ride
along.

---

## Milestone M-AUDIT-2 — PR breakdown

Detail per PR in `./docs/drafts/YYYYMMDD-HHMM-m-audit-2-<name>.md`
(planning subagent writes one per PR at kickoff).

- [x] **PR-CADENCE** — Replace the 500 ms broadcast `GetItems` poll
  with per-path cadence + per-sensor freshness, per the research
  matrix at `docs/drafts/20260424-1959-victron-dbus-cadence-matrix.md`.
  Worst-case reseed load drops from 18 GetItems/s to 0.15 — matching
  what Victron reference clients do. Changes:
  - `DBUS_POLL_PERIOD` const → per-service reseed scheduler
    (`BTreeMap<Service, (interval, next_due)>`) round-robin.
  - `ControllerParams.freshness_local_dbus: Duration` → per-sensor
    `SensorFreshnessTable` keyed by `SensorId`.
  - Per-readback freshness windows (longer than sensors, since
    readbacks only change on writes).
  - Keeps PR-URGENT-19/20/22's reconnect + timeout scaffolding.
  Ships alone first; classification logging + progressive
  degradation (matrix §Rate-limit detection) are follow-ups.

- [x] **PR-DAG** — TASS core DAG orchestrator. Splits into PR-DAG-A
  (infra — zero behavior change), PR-DAG-B (migrate zappi_active →
  `world.derived.zappi_active` + delete `DerivedView`), PR-DAG-C
  (semantic `depends_on` edges + per-edge field surface).
  Plan: `docs/drafts/20260424-1700-m-audit-2-pr-dag-plan.md`.
  - [x] **PR-DAG-A** — Core trait, CoreRegistry, Kahn's topo sort,
    5+2 tests (build / determinism / cycle / missing / duplicate +
    boundary-consistency regression guard + tie-break). Six `run_*`
    wrapped as zero-sized-struct impls with linear-chain `depends_on`
    preserving today's order. `DerivedView` computed once per tick in
    `run_all` and passed by reference to each core. 2 review rounds
    (round 1 blocked on ship-critical D01; round 2 clean + 3 info
    notes).
  - [x] **PR-DAG-B** — `zappi_active` migrated to first-class
    `ZappiActiveCore` (topo-sort root, `depends_on=[]`) writing to
    `world.derived.zappi_active`. `DerivedView`, `compute_derived_view`,
    `bookkeeping.zappi_active`, `CurrentLimitBookkeeping.zappi_active`,
    all `*InputGlobals.zappi_active` fields, and the removed `Core::run
    &DerivedView` parameter all deleted. Dashboard wire-compat preserved
    (`ModelBookkeeping.zappi_active` sourced from `world.derived`).
    Semantic choice locked + documented: no cross-tick latching on
    stale sensors (departs from PR-04's latched-via-bookkeeping);
    SPEC §5.8 updated. 2 review rounds (D01 dismissed as misread plan;
    D02 real — landed 2 regression tests + doc comment).
  - [x] **PR-DAG-C** — Semantic `depends_on` edges per §4 audit. Every
    `depends_on` returns `&'static [DepEdge]` carrying the producing
    core PLUS the live `world.<area>.<field>` identifiers that
    motivate the edge; dashboard renders each edge as
    `"<core> via <field1>, <field2>"`. Linear-chain placeholder edges
    deleted: ZappiMode/EddiMode now `&[]`; WeatherSoc rewired to
    `[Setpoint via charge_to_full_required]`; Schedules and
    CurrentLimit gain real fields-attributed edges; SensorBroadcast
    depends on every actuator (was implicit via the chain). Material
    behaviour change: `CurrentLimit.depends_on += [Schedules]` flips
    the runtime order so CurrentLimit reads same-tick
    `battery_selected_soc_target` (was one-tick stale). Topological
    order: ZappiActive → Setpoint → ZappiMode → EddiMode → WeatherSoc →
    Schedules → CurrentLimit → SensorBroadcast. 3 new tests
    (`current_limit_runs_after_schedules_post_pr_dag_c`,
    `weather_soc_runs_after_setpoint_post_pr_dag_c`,
    `dashboard_depends_on_strings_carry_field_names`).
- [x] **PR-URGENT-20** — D-Bus session dies ~20s after startup; two-
  part fix: (1) reduce aggressive 500ms poll → 5s + freshness 2s →
  15s to stop hammering the Venus broker; (2) **graceful reconnect
  with exponential backoff** (user-mandated: if eviction ever does
  happen despite gentler polling, we recover without restarting the
  whole service). `Subscriber::connect` → `Subscriber::new` (pure
  config); `run()` loops `connect_and_serve()` with 1s→30s backoff
  (resets to 1s after 60s+ healthy session). Triggers: stream-end,
  dual-silence (no signals + no poll success in 30s after
  session_age≥30s). Persistent state stays on `Self`; per-session
  state (connection, owner_to_service, fail_counts) lives as
  function locals. Heartbeat enhanced with session_age + last-signal
  + last-poll-success metrics for operator visibility.

- [x] **PR-URGENT-19** — REAL root cause of the field wedge (confirmed
  by per-thread `wchan` diagnostic added to fetch-logs.sh by user's
  suggestion): `Subscriber::seed_service` awaits
  `proxy.call("GetItems", &()).await` with no timeout. One hung
  Venus D-Bus reply → poll arm parked forever → signal + heartbeat
  arms starved → sensors decay → controllers bail. This wedge
  class was called out as deferred D08 during PR-URGENT-13 review
  and never landed; now biting daily. Fix: 2 s per-call timeout
  on GetItems; error flows through the existing rate-limited
  warn + escalation path from PR-URGENT-13. PR-URGENT-15/16/17/18
  were all real downstream hardening but not THIS bug — each
  remains warranted.

- [x] **PR-URGENT-18** — ROOT CAUSE of the field wedge:
  `tracing_subscriber::fmt::layer()` default writer is synchronous
  `io::stdout()`. On daemontools the pipe buffer is ~64 KB;
  whenever multilog briefly slows, `write_all` blocks the emitting
  thread. With `worker_threads = 2`, two concurrent tracing events
  can stall BOTH workers → entire async runtime wedges →
  PR-URGENT-15/17 timeouts never fire because threads never reach
  their await points. PR-URGENT-15/16/17 each fixed a real bug but
  addressed symptoms downstream of this root cause. Fix: route
  `fmt_layer` through `tracing_appender::non_blocking` — writes
  queue onto a dedicated blocking thread; tokio workers never
  touch the pipe.

- [x] **PR-URGENT-17** — Log publisher timeout hotfix. Adversarial
  review of PR-URGENT-16 caught the sibling bug: `spawn_log_publisher`
  had raw `client.publish(...).await` with no timeout. Broker stall →
  log publisher blocks → log mpsc (cap 256) fills → subsequent
  `try_send`s drop — including PR-URGENT-15's "mqtt publish stuck
  >1s" warn from the runtime. Diagnostic self-silencing. Fix:
  1 s `tokio::time::timeout` with `eprintln!` on fire (NOT tracing —
  avoid re-entry into the wedge pipeline).

- [x] **PR-URGENT-16** — Second wedge hotfix: WS client held world
  mutex across the initial-snapshot `send_json` (axum WS TCP write).
  A stalled browser tab (paused, throttled, backpressured) → WS send
  stalls → MutexGuard never drops → runtime's `self.world.lock().await`
  blocks forever → tick loop wedges. Controllers stop ticking → sensors
  go Stale (2s freshness) → schedules bail → dashboard shows disabled.
  Pre-existing latent bug in `crates/shell/src/dashboard/ws.rs:54-61`;
  became visible because the user had the dashboard open while
  redeploying PR-URGENT-15. Fix: scope the MutexGuard to snapshot
  construction only; release before the network send. PR-URGENT-15's
  MQTT-queue fix is still a net improvement (avoids a separate wedge
  class) but was not the root cause this time.

- [x] **PR-URGENT-15** — Deploy-time wedge hotfix: rumqttc request-queue
  bump 64→4096 + 1s timeout on runtime dispatch's Publish await.
  Found post-deploy of `3f0821c`: all D-Bus sensors Stale, both
  schedules showing disabled, no heartbeats in log. Root cause:
  PR-SCHED0 lifted `Publish(ActuatedPhase)` above the writes_enabled
  gate → startup publish burst + HA discovery + retained bootstrap +
  MqttLogLayer stream saturated rumqttc's 64-slot request channel →
  `publish().await` blocked the runtime dispatch loop → event channel
  backed up → subscriber's `tx.send().await` blocked → no poll ticks,
  no heartbeats, sensors decay.

- [x] **PR-SCHED0** — Observer-mode target-mutation inversion. Root
  cause (b+a hybrid): observer mode left target=Unset while Node-RED
  legacy `days=-7` was the visible `actual`; dashboard rendered the
  actual verbatim. Fix: reversed half of PR-05 — in observer mode
  `propose_target` still runs (target reflects intent), but
  `WriteDbus`/`CallMyenergi`/`mark_commanded`/`actual.deprecate` stay
  gated. Also lifted `Publish(ActuatedPhase)` above the gate so the
  dashboard sees phase transitions honestly. A-06 remains fixed via
  PR-05's KillSwitch edge-reset. 4 review rounds; 14 defects filed
  (1 resolved-deferred, 13 resolved in-PR).
- [x] **PR-03** — Zappi `time_in_state` monotonic-Instant fix (A-04, A-24). Shipped in commit `aab6c28`.
- [x] **PR-07** — Subscribes to `org.freedesktop.DBus.NameOwnerChanged`
  on the same zbus connection. On each signal, if the well-known name
  is one we route, update `owner_to_service` map (remove old unique
  name, insert new) and flag the service's heap entry with
  `next_due = now` so the scheduler triggers an immediate reseed on
  next iteration. Empty `new_owner` (service disappearing) just drops
  the old mapping without reseeding. 4 unit tests: rename, disappear,
  ignored-non-watched, first-appearance-empty-old-owner. Rule scoped
  with `sender("org.freedesktop.DBus")` so only broker-emitted
  signals match. Resolves A-11.
- [x] **PR-08** — `SchedulePartial` accumulator clearing (A-12). Shipped in commit `0cf4a18`.
- [x] **PR-09b** — `grid_export_limit_w` hardening follow-up to PR-09a
  (A-09, A-10, A-34, A-35). Shipped in commit `6c8c9c8`. Deadband i64
  widening landed separately as `PR-setpoint-deadband-i64` (A-31, commit
  `9eb899f`).
- [x] **PR-10** — `force_disable_export`: delete the unused field (A-19).
  Shipped in commit `b9e39b6`.
- [x] **PR-11** — weather-SoC γ-hold + once-per-day (A-20, A-21).
  Shipped in commit `3d9c987`. A-36 (eddi_last_transition_at honesty)
  shipped separately in `PR-eddi-dwell` (commit `b6dd179`).
- [x] **PR-12** — myenergi HTTP body-level error parsing (A-22, A-23).
  Shipped in commit `a25bc15`.
- [x] **PR-MISC** — minor/nit hygiene rollup. Drained across
  `PR-HYGIENE-1..11` plus targeted PRs (`PR-forecast-freshness`,
  `PR-solcast-schema`, `PR-mqtt-uuid`, `PR-myenergi-backoff`,
  `PR-forecast-backoff`, `PR-open-meteo-efficiency`,
  `PR-dashboard-trysend`, `PR-gamma-hold-per-knob`,
  `PR-weather-soc-range`, `PR-solar-export-hoymiles`,
  `PR-sched-decisions`, `PR-UX-1`, etc.). See `git log` and
  `Drain remaining defect ledger` commit `a494602`.
- [x] **PR-writer-reconnect** — D-Bus writer reconnect + bounded
  `SetValue` + lazy infallible constructor (A-56). Mirrors
  PR-URGENT-20 subscriber pattern. Plan:
  `docs/drafts/20260424-2245-pr-writer-reconnect.md`.
- [x] **PR-zappi-schedule-stop** — Field-observed regression: extended
  charge started at 05:00 but never stopped because the legacy `00 08
  * * *` Off-cron was not ported. Adds a post-extended stop rule in
  `evaluate_zappi_mode` (08:00–08:04 window forces Off when current
  mode != Off) plus surfaces the three daily Zappi mode edges (02:00 /
  05:00 / 08:00) in the dashboard schedules section. Plan:
  `docs/drafts/20260427-1133-pr-zappi-schedule-stop.md`.

---

## Milestone M-ZAPPI-DRAIN — PR breakdown

Detail in `./docs/drafts/20260429-1700-m-zappi-drain-plan.md`. Five PRs;
sequenced so PR-ZD-1 + PR-ZD-2 are pure plumbing (zero behaviour change),
PR-ZD-3 + PR-ZD-4 ship the new control law, PR-ZD-5 is frontend-only.

- [x] **PR-ZD-1 — Sensors.** Wire `HeatPumpPower` + `CookerPower`
  (zigbee2mqtt MQTT, JSON `.power` field) and `Mppt0OperationMode` +
  `Mppt1OperationMode` (Victron D-Bus `/MppOperationMode`) through the
  full sensor pipeline (SensorId + world + ingestion + dashboard). No
  control-loop coupling. ≥ 10 new tests covering parse / fresh-stale
  transitions / dashboard surfacing.
- [x] **PR-ZD-2 — Knobs.** Add five knobs through the 11-step CLAUDE.md
  registration: `zappi_battery_drain_threshold_w` (default 1000),
  `…_relax_step_w` (default 100), `…_kp` (default 1.0), `…_target_w`
  (default 0, reserved for future PI extension; routes via
  `KnobValue::Float`), `…_hard_clamp_w` (default 200). All
  `category = "config"`. ≥ 4 new tests.
- [x] **PR-ZD-3 — Soft loop.** Replace lines 617–637 in
  `evaluate_setpoint()` with the compensated-drain control law. Drop the
  `(2..8)` Soltaro carve-out (folded into the unified loop). 23:55
  protection window unchanged. Add `battery_dc_power` to setpoint's
  required-fresh set. ≥ 8 new tests.
- [x] **PR-ZD-4 — Hard clamp.** Post-`evaluate_setpoint()` Fast-mode
  safety net in `run_setpoint`: when `zappi_mode.target == Fast` AND
  `!allow_battery_to_car` AND `compensated_drain > hard_clamp_w`, raise
  the proposed setpoint by the excess. ≥ 4 new tests covering Fast vs
  Eco vs Off vs `allow_battery_to_car=true`.
- [x] **PR-ZD-5 — Dashboard MPPT-mode display.** Frontend-only: render
  the two `Mppt*OperationMode` sensors as human-readable strings ("Off",
  "Voltage-or-current-limited", "MPPT-tracking"). 1 web test.

### Cross-cutting (M-ZAPPI-DRAIN)

- Compensated drain definition (locked):
  `compensated_drain = max(0, -battery_dc_power - heat_pump - cooker)`.
  Stale HP / cooker contribute `0` (conservative; tighter clamp).
- MPPT op-mode coupling (locked): observability only, no control-loop
  read. May feed future SoC-chart or forecast-view annotations.
- Hard-clamp scope (locked): only fires when the Zappi *target* (not
  *actual*) is `Fast`, AND `!allow_battery_to_car`, AND
  `world.derived.zappi_active`. Eco / Eco+ / Off bypass entirely.
- Stale-meter semantics (locked): for HP / cooker, stale → 0 W. For
  `battery_dc_power`, stale → `build_setpoint_input` returns `None`
  → safety fallback (idle 10 W).
- Knob category (locked): all 5 new knobs are `"config"` (install-time
  tuning, not daily-use operator).
- MPPT op-mode index orientation (locked): `Mppt0OperationMode` ↔
  `mppt_0` = `ttyUSB1` (DI 289); `Mppt1OperationMode` ↔ `mppt_1` =
  `ttyS2` (DI 274) — matches existing `MpptPower0`/`MpptPower1`
  numbering.

---

## Milestone M-ZAPPI-DRAIN-OBS — PR breakdown

Detail in `./docs/drafts/20260430-2000-m-zappi-drain-obs-plan.md`.
Four PRs, sequenced. Total: ~16 new unit tests across the milestone.

- [x] **PR-ZDO-1 — Capture pipeline (Core)**. New `ZappiDrainState` /
  `ZappiDrainSnapshot` / `ZappiDrainSample` / `ZappiDrainBranch` types
  in `core::world` + `core::types`. Capture fold into `run_setpoint`
  immediately after the hard-clamp block, before grid-cap. 120-sample
  FIFO ring; reset on `fresh_boot`. Pure backend; no broadcast or
  wire-format yet. ≥ 6 unit tests including: lockstep with controller
  output, ring-buffer eviction, branch-classification correctness
  across all 4 branches, observer-mode honesty,
  no-feedback-into-control invariant, fresh_boot reset.
- [x] **PR-ZDO-2 — HA broadcast sensors (Option A)**. New
  `ControllerObservableId` enum + `PublishPayload::ControllerNumeric`
  / `ControllerBool`. Wire through `SensorBroadcastCore` (4th block
  alongside Sensors / BkBool / BkNumeric). New
  `controller/<name>/state` topic root in `serialize.rs`. Three new
  HA discovery configs (1 `sensor`, 2 `binary_sensor`). Frontend
  display-name + description registrations. ≥ 5 tests.
- [x] **PR-ZDO-3 — Wire format + dashboard data plumbing (Option C,
  backend half)**. Extend `models/dashboard.baboon` with
  `ZappiDrainBranch` enum, `ZappiDrainSample` /
  `ZappiDrainSnapshotWire` / `ZappiDrainState` data blocks, and
  `WorldSnapshot.zappi_drain_state` field. Additive within v0.3.0.
  `scripts/regen-baboon.sh` + fix `convert.rs`. ≥ 2 tests.
- [x] **PR-ZDO-4 — Frontend rendering (Option C, frontend half)**.
  New `<section id="zappi-drain-section">` above `#sensors` in
  `crates/shell/static/index.html`. Three big-number widgets +
  hand-rolled SVG sparkline (mirroring `web/src/chart.ts` idiom)
  with branch-coloured segments and dashed reference lines for
  `threshold_w` (orange) and `hard_clamp_w` (red). New
  `renderZappiDrainSummary` + `renderZappiDrainChart` exports in
  `render.ts` wired into `applySnapshot`. ≥ 3 tests in
  `render.test.ts`.

### Cross-cutting (M-ZAPPI-DRAIN-OBS)

- **Read-only invariant (locked).** New observables consume what
  `evaluate_setpoint` and `run_setpoint`'s hard-clamp block already
  computed; no re-derivation, no feedback into any controller. Test
  PR-ZDO-1.T6 locks "no controller reads from
  `world.zappi_drain_state`."
- **Lockstep capture (locked).** Capture inside `run_setpoint`
  immediately after the hard-clamp block determines
  `(hard_clamped_target, hard_clamp_engaged, hard_clamp_excess)`.
  The same tick's controller output is recorded; cross-tick drift
  is impossible.
- **Branch classifier mirror (locked).** New
  `classify_zappi_drain_branch` pure helper; mirrors the if/else if
  ladder in `evaluate_setpoint`. `// LOCKSTEP:` comments on both
  sites; PR-ZDO-1.T3 covers all 4 branches.
- **Honesty under observer mode (locked).** `writes_enabled=false`
  does NOT short-circuit capture. PR-ZDO-1.T4.
- **Ring-buffer policy (locked).** N=120, FIFO, reset on
  `fresh_boot`, no persistence.
- **Branch enum (locked).** `Tighten` / `Relax` / `Bypass` /
  `Disabled`.
- **Sensor naming (locked).** `controller.zappi-drain.compensated-w`
  / `.tighten-active` / `.hard-clamp-active` under
  `controller/<name>/state` topic root.
- **Wire-format additive (locked).** New `ZappiDrainState` block +
  new `WorldSnapshot.zappi_drain_state` field within v0.3.0; no
  migration stubs.
- **Tighten-active during Bypass/Disabled (locked).** `false`
  (boolean, not tri-state).
- **Per-sample timestamp source (locked).** `clock.wall_clock_epoch_ms()`;
  renderer sorts at draw time.
- **Polyline interpolation (locked).** Linear (matches SoC chart
  convention).

---

## Cross-cutting architectural notes (locked)

- [x] **ET112 grid current sensor is not trusted — derive `grid_current` from
  `grid_power / grid_voltage` instead.** The ET112 reports phantom amps
  (non-zero current with near-zero real power). The controller intentionally
  uses the system-aggregate power reading divided by a sanity-gated voltage
  (see `effective_grid_v` in `crates/core/src/controllers/current_limit.rs`).
  This is why PR-02 hardens the division path (A-03) rather than switching
  to the direct current sensor. Don't "simplify" by swapping in the direct
  `grid_current` sensor; it will starve the controller with ghost amps.

- [x] **Observer-mode cold-start default is `writes_enabled = false`** —
  SPEC §7 is to be updated to match code (safer default). See A-37.
- [x] **Three-layer actuation safety chain must be preserved** —
  (1) core `knobs.writes_enabled`, (2) config `[dbus] writes_enabled`,
  (3) config `[myenergi] writes_enabled`. No PR relaxes this.
- [x] **Every controller branch that changes outputs must populate a
  Decision** — the "honesty invariant" the user has been building. Fixes
  that short-circuit output paths must still emit a Decision explaining
  why.
- [x] **No refactors beyond what a fix requires.** Surgical patches.
- [x] **`charge_battery_extended` derivation:** derivation in
  `run_schedules` is the source of truth. Weather-SoC writes a separate
  `bookkeeping.charge_battery_extended_today` that resets at midnight.
  Schedules ORs that in. Lands in PR-04.
- [x] **`grid_import_limit_w` default:** `10 W` (matches idle-bleed
  promotion). `grid_export_limit_w` unchanged (`5000 W`). Ingest clamp
  `SAFE_MAX_GRID_LIMIT_W = 10_000` applied to both. Lands in PR-09.
- [x] **`force_disable_export` in `CurrentLimitInputGlobals`:** delete
  the field (not yet used; dead code). Lands in PR-10.

- [ ] **TASS cores form a validated DAG.** Any derived value read by
  more than one core MUST be its own core (derivation core). The
  orchestrator walks cores in topological order; dependency graph is
  built at registry construction and validated for cycles + missing
  deps. Lands in PR-DAG. Applies to `zappi_active` first; review other
  existing read/write bookkeeping fields for similar shape.

---

## Completed

- **PR-ZDO-4 — Frontend rendering (Option C, frontend half)**
  (M-ZAPPI-DRAIN-OBS, 2026-04-30) — New `<section
  id="zappi-drain-section">` above `#sensors` in the Detail tab. Three
  big-number widgets (drain W / branch label / hard-clamp Engaged
  status) plus a hand-rolled SVG sparkline showing 30 minutes of
  `compensated_drain_w` history with branch-coloured per-segment
  polyline and dashed reference lines for `threshold_w` (orange) and
  `hard_clamp_w` (red). Branch colour scheme: Tighten=red `#d33`,
  Relax=green `#3a3`, Bypass=grey `#888`, Disabled=neutral `#555`.
  Y-axis auto-scaling excludes Disabled samples from the max
  calculation (per locked PR-ZDO-1-D05 / PR-ZDO-2-D02 contract:
  Disabled-branch `compensated_drain_w = 0.0` is a placeholder, not
  a real reading); the `compensated-w` big-number renders `—`
  (not `0 W`) when `latest.branch == Disabled`. Disabled segments
  in the polyline are drawn at y=0 with 50% opacity grey to indicate
  the controller didn't run during those samples.
  Pulled out a pure `summaryFor(latest)` decision helper to enable
  unit-testing of the big-number rendering logic without a DOM —
  matches the project's "tsc + esbuild only, no test runner" stance
  (consistent with `fmtMpptOperationMode` from PR-ZD-5).
  Adversarial review found 10 defects, all minor/nit/cosmetic. Three
  actionable (D01 test rigor → extracted `summaryFor`; D02
  tautological assertion → deleted; D07 dead conditional → cleaned
  up); seven closed deferred or note-only (D03 cast workaround,
  D04 single-sample edge, D05 label overlap, D06 inline style,
  D08 Disabled-at-y=0 documented behaviour, D09 theoretical edge,
  D10 ordering note).
  Verification: `cd web && ./node_modules/.bin/tsc --noEmit -p .`
  clean; `cargo test --workspace` → 572 passed (no backend changes;
  count unchanged from PR-ZDO-3); `cargo clippy --workspace
  --all-targets -- -D warnings` clean; `cargo build --target
  armv7-unknown-linux-gnueabihf --release` green.
  Notes / surprises:
  - The pure `summaryFor` helper makes the rendering decisions
    testable without jsdom. Locks the Disabled→`—` honest contract
    in a unit test rather than relying on a code-comment + manual
    verification. This pattern can be reused for future
    DOM-mutating renderers.
  - Branch colours chosen for semantic clarity (red=tighten=action,
    green=relax=ok, grey=bypass=neutral, neutral=disabled=offline).
    Tighten/Relax red-vs-green is the worst colour-blind pair, but
    the branch tag's text label complements the colour so semantic
    info is preserved.
  - Refactor `summaryFor` extraction also serves as the documented
    surface area for future PRs that want to display the snapshot
    elsewhere (e.g. an ops page).
  Constraints future work must respect:
  - The Disabled→`—` invariant in `summaryFor` MUST be preserved.
    A "fix" that displays `0 W` for Disabled would re-introduce
    the dishonest-zero defect that PR-ZDO-2-D02 fixed.
  - The chart's reference lines (threshold + hard-clamp) come from
    `latest.threshold_w` / `latest.hard_clamp_w` (snapshotted at
    capture time). If the operator retunes mid-window, the chart's
    reference lines reflect the snapshot's value, not the current
    knob — by design (locked: snapshotted-for-chart-consistency
    rationale in PR-ZDO-1).
  - Future renderers that consume `ZappiDrainState` MUST sort
    `samples` by `captured_at_epoch_ms` at render time (PR-ZDO-1
    risk: GX clock can jump backwards).

- **PR-ZDO-3 — Wire format + dashboard data plumbing (Option C, backend
  half)** (M-ZAPPI-DRAIN-OBS, 2026-04-30) — Extends the baboon model
  with four new types so the WorldSnapshot carries
  compensated-drain observability state to the dashboard. Additive
  within v0.3.0 (no version bump per CLAUDE.md "Deployment topology"):
  - `enum ZappiDrainBranch { Tighten, Relax, Bypass, Disabled }`.
  - `data ZappiDrainSample` (4 fields — compact ring-buffer entry).
  - `data ZappiDrainSnapshotWire` (7 fields — current snapshot for
    big-number widgets in PR-ZDO-4).
  - `data ZappiDrainState { latest: opt[ZappiDrainSnapshotWire],
    samples: lst[ZappiDrainSample] }`.
  - `WorldSnapshot.zappi_drain_state` field (appended at end, 16th
    field — no insertion that would shift wire ordering).
  Regen produced 4 new files each in `crates/dashboard-model/` and
  `web/src/model/`. `crates/shell/src/dashboard/convert.rs` adds
  `zappi_drain_branch_to_model` (4-arm exhaustive match) and
  `zappi_drain_state_to_model` (maps `latest` + `samples`); name
  swap `captured_at_ms` (core) → `captured_at_epoch_ms` (wire) is
  consistent. `world_to_snapshot` populates the new field.
  Frontend `displayNames`/`descriptions` entries from PR-ZDO-2 are
  now wire-reachable.
  Adversarial review: zero defects on round 1. T1 verifies
  oldest-first sample ordering + Tighten/Bypass enum round-trip;
  T2 verifies empty-world edge. No control-loop coupling, no HA
  broadcast change, no frontend rendering change.
  Verification: `bash scripts/regen-baboon.sh` clean (82 retained
  v0.3.0 definitions); `cargo test --workspace` → 572 passed (355
  core + 10 dashboard-model + 207 shell, +2 vs PR-ZDO-2 baseline
  570); clippy + tsc clean; armv7 cross-build green.
  Notes / surprises:
  - Wire size ceiling: 120 samples × ~32 bytes + 7-field snapshot
    ≈ 4 KB, comparable to existing `soc_chart` history.
  - The auto-emitted `from_0_2_0_world_snapshot.rs` migration stub
    is commented-out / empty per the regen template (project
    convention: single-client deployment, never called at runtime).
  - PR-ZDO-2's preregistered displayNames + descriptions entries
    (D04 deferred) become reachable now that the wire field exists.
    No additional frontend work in PR-ZDO-3.
  Constraints future work must respect:
  - The `WorldSnapshot.zappi_drain_state` field MUST stay at the
    end of the WorldSnapshot data block. Inserting new fields
    before it would shift the wire ordering and break the
    additive-within-v0.3.0 invariant.
  - The `captured_at_ms` (core) → `captured_at_epoch_ms` (wire)
    name swap is intentional — it follows the convention that
    wire fields use the explicit `_epoch_ms` suffix while internal
    fields use the shorter `_ms`. Maintain this when adding new
    timestamp fields.

- **PR-ZDO-2 — HA broadcast sensors (Option A)** (M-ZAPPI-DRAIN-OBS,
  2026-04-30) — Three new derived sensors broadcast via
  `SensorBroadcastCore` so HA Recorder can chart the
  M-ZAPPI-DRAIN compensated-drain loop's behaviour over time:
  - `controller.zappi-drain.compensated-w` (numeric, W,
    freshness-aware)
  - `controller.zappi-drain.tighten-active` (binary_sensor, true when
    soft loop is tightening)
  - `controller.zappi-drain.hard-clamp-active` (binary_sensor, true
    when Fast-mode hard clamp engaged)
  Topic root `controller/<name>/state` (new namespace, distinct from
  `sensor/` and `bookkeeping/`). New `ControllerObservableId` enum +
  `PublishPayload::ControllerNumeric` / `ControllerBool` variants +
  `PublishedCache.controller_{numeric,bool}` dedup HashMaps. Fourth
  block in `SensorBroadcastCore::run` reads
  `world.zappi_drain_state.latest` (lockstep with PR-ZDO-1 capture);
  no re-derivation. HA discovery: 1× `sensor` (compensated-w with
  `device_class: power`, `state_class: measurement`, `unit: W`) +
  2× `binary_sensor`.
  Two adversarial review rounds. Round 1 caught **2 major defects**:
  (D01) HA discovery referenced an unpublished `availability_topic`
  → all entities would have shown as Unavailable in HA;
  (D02) `branch == Disabled` placeholder `compensated_drain_w = 0.0`
  was leaking to HA Recorder as a real 0 W reading during every
  `apply_setpoint_safety` fallback. The user's exact framing —
  "a derived sensor out of sync with the controller is worse than
  no observability" — would have been violated.
  Fixes:
  - D01: removed `availability_topic` from all three discovery
    configs; freshness signalled inline via `unavailable` token in
    state-body (matches `publish_sensors` / `publish_bookkeeping`
    convention).
  - D02: added a guard arm in `SensorBroadcastCore`'s match on
    `world.zappi_drain_state.latest`: `Some(s) if s.branch ==
    Disabled` → `(0.0, Stale)` → wire body encodes as
    `"unavailable"`. Both no-snapshot and Disabled-branch paths now
    yield the same wire output.
  - D05: new regression test
    `controller_observables_disabled_branch_yields_unavailable_and_false_bools`
    locks D02's fix.
  Verification: `cargo test --workspace` → 570 passed (355 core +
  10 dashboard-model + 205 shell, +10 vs PR-ZDO-1 baseline 560);
  clippy + tsc clean; armv7 cross-build green.
  Defects (5 filed, all closed): D01 + D02 major (HA discovery
  unavailable / Disabled-leak), D03 minor (T4 numbering, note-only),
  D04 nit (displayNames preregistered for PR-ZDO-3, deferred), D05
  nit (regression test for D02).
  Notes / surprises:
  - The two majors demonstrate why the user's "lockstep" framing is
    operationally about more than just the value-axis: the HA-side
    surface (recorder, dashboards) is itself a downstream renderer
    of the broadcast, and must respect the same "skip Disabled
    samples" contract as the in-dashboard chart will (PR-ZDO-4).
    The fix unifies both surfaces under the same wire encoding.
  - `availability_topic` rule: the project's existing convention
    (since `publish_sensors`) is to encode freshness inline via the
    `unavailable` token in the state body — never via a separate
    availability topic. PR-ZDO-2's first attempt accidentally
    diverged; the fix restores convention parity.
  - Operator action post-deploy: HA caches retained discovery
    payloads; restarting the controller re-publishes the corrected
    discovery (no `availability_topic`), and HA picks up the new
    config. No manual cleanup needed.
  - Frontend `displayNames` / `descriptions` entries are
    preregistered now even though no wire-format field references
    them yet. PR-ZDO-3 lands the wire field; preregistration removes
    a future-PR step.
  Constraints future work must respect:
  - The `Disabled` branch is the canonical "controller couldn't
    run" signal. Any future broadcast surface (numeric or
    string-valued) MUST treat `branch == Disabled` as Stale, not
    as a real reading.
  - `controller/<name>/state` topic root is reserved for
    controller-derived observables. Future siblings (setpoint
    decision tag, schedule activation flags) ride this prefix.
  - HA discovery for new derived sensors MUST NOT include
    `availability_topic` unless the controller actually publishes
    to it. Inline `unavailable` in state-body is the convention.

- **PR-ZDO-1 — Capture pipeline (Core)** (M-ZAPPI-DRAIN-OBS, 2026-04-30)
  — Backend-only observability foundation. New types in `core::types`
  (`ZappiDrainBranch` enum: Tighten / Relax / Bypass / Disabled with
  `name()` + `Display`) and `core::world` (`ZappiDrainSnapshot`,
  `ZappiDrainSample`, `ZappiDrainState` with `RING_CAPACITY = 120` +
  `SAMPLE_INTERVAL_MS = 15_000` + `push()`). New `pub
  zappi_drain_state: ZappiDrainState` field on `World`; reset on
  `fresh_boot`. Capture in `run_setpoint` immediately after the
  hard-clamp block; also in `apply_setpoint_safety` (records
  `branch = Disabled`). Lockstep helper `classify_zappi_drain_branch`
  mirrors `evaluate_setpoint`'s if/else ladder; `// LOCKSTEP:`
  comments on both sides. New `Clock::wall_clock_epoch_ms()` trait
  method (impl on `RealClock`, `FixedClock`, test `AdvancingClock`).
  Two adversarial review rounds; round 1 caught a **major** defect
  (D01): `run_setpoint` runs on every event in production, not just
  Tick — so the 120-sample 30-min buffer would have collapsed to
  seconds. Fix: time-gate the `samples.push_back` half of `push` on
  `SAMPLE_INTERVAL_MS = 15_000`. `latest` updates unconditionally
  every call so HA broadcasts (PR-ZDO-2) and wire snapshots
  (PR-ZDO-3) stay lockstep. Round 2 found 2 follow-ups (D08 weak
  test assertion, D09 missing doc on backwards-clock-jump gate
  behaviour); both resolved.
  Verification: `cargo test --workspace` → 560 passed (349 core +
  10 dashboard-model + 201 shell, +9 vs M-ZAPPI-DRAIN baseline 551);
  clippy + tsc clean; armv7 cross-build green.
  Defects (9 filed, all closed): D01 major (run_setpoint per-event
  firing → time-gated push), D02-D06 minor (doc/test gaps), D07-D08
  nit, D09 minor (backwards-clock-jump doc).
  Notes / surprises:
  - The major D01 defect demonstrates why round-1 review was
    essential: the plan-faithful capture-on-every-tick interpretation
    matched the plan's wording but not the production runtime, where
    `run_setpoint` is a per-event callback. Time-gating inside `push`
    keeps `latest` at controller cadence (broadcast-friendly) while
    bounding `samples` to the chart's 30-min window.
  - `apply_setpoint_safety` records `branch = Disabled,
    compensated_drain_w = 0.0`. The `0.0` is a placeholder; renderers
    MUST grey-out / skip `Disabled` samples. Locked by D05's
    field doc-comment.
  - `wall_clock_epoch_ms` is a new `Clock` trait method;
    deterministic-clock test fixture had to be updated to implement it.
  Constraints future work must respect:
  - `// LOCKSTEP:` comment cross-references between
    `classify_zappi_drain_branch` (process.rs) and `evaluate_setpoint`'s
    if/else ladder (setpoint.rs) MUST be maintained. PR-ZDO-1.T3 is
    the regression guard.
  - No controller branch reads from `world.zappi_drain_state`.
    PR-ZDO-1.T6 locks this.
  - `SAMPLE_INTERVAL_MS = 15_000` matches the default tick cadence.
    If the tick rate ever changes, this constant must move with it.

- **PR-ZD-5 — Dashboard MPPT-mode display** (M-ZAPPI-DRAIN, 2026-04-29)
  — Frontend-only. Renders the two `Mppt*OperationMode` sensors as
  human-readable strings instead of raw `0`/`1`/`2` numbers:
  `0 → "Off"`, `1 → "Voltage-or-current-limited"`,
  `2 → "MPPT-tracking"`. Out-of-range / float-drift / NaN / Infinity
  fall back to `String(value)` so future firmware drift degrades
  visibly. PR-ZD-1's descriptions for these sensors are already
  accurate (corrected by PR-ZD-1-D05); no change needed.
  New helpers in `web/src/render.ts`: `MPPT_OP_MODES` lookup table,
  `fmtMpptOperationMode(value: number): string`,
  `fmtSensorValue(name, value): string | null` dispatcher. The
  `renderSensors` valText assignment routes through `fmtSensorValue`
  before falling back to `fmtNum`. Side-fix: `act.value === null`
  → `v == null` (covers `null` AND `undefined`; behavioural no-op
  because `fmtNum` already handles both, but tighter contract).
  No test framework in `web/`; added a standalone smoke-check
  `web/src/render.test.ts` with 8 assertions runnable via esbuild
  bundle + node, type-checked by `tsc --noEmit`.
  Adversarial review: zero defects. Two informational notes (N01
  about the side-fix being behaviourally a no-op; N02 noting six
  other call sites with the same `=== null` pattern that PR-ZD-5
  did not touch — out of scope).
  Verification: `cargo test --workspace` → 551 passed (no backend
  changes, count unchanged); `cargo clippy --workspace --all-targets
  -- -D warnings` clean; `cd web && ./node_modules/.bin/tsc --noEmit
  -p .` clean. Smoke-check assertions pass when run.

- **PR-ZD-4 — Hard clamp** (M-ZAPPI-DRAIN, 2026-04-29) — Fast-mode-only
  hard clamp as a separate post-`evaluate_setpoint()` step in
  `run_setpoint`. Fires only when ALL of: `world.zappi_mode.target.value
  == Some(ZappiMode::Fast)` (commanded target, predictive arming —
  not the readback), `!allow_battery_to_car`, `world.derived.zappi_active`,
  AND `compensated_drain > zappi_battery_drain_hard_clamp_w` (default
  200 W). Raises the proposed setpoint by `(drain - hard_clamp_w)`
  BEFORE the existing `grid_export_limit_w / grid_import_limit_w`
  clamp; the grid-cap acts as the final ceiling. Eco / Eco+ / Off
  bypass entirely (those modes self-modulate via Zappi's CT clamp +
  the soft loop is sufficient).
  Centralised the compensated-drain formula: new
  `compute_compensated_drain(battery, hp, cooker)` pure helper in
  `crates/core/src/controllers/setpoint.rs`; new wrapper
  `compensated_drain_w(&World)` in `crates/core/src/process.rs`. The
  PR-ZD-3 soft-loop call site refactored to call the helper —
  behaviour identical, purely deduplication. All PR-ZD-3 tests pass
  with unchanged expected values.
  Decision factors only emitted when clamp engaged (mirrors PR-09a-D02
  pattern): `hard_clamp_engaged: "true"`, `hard_clamp_excess_W`,
  `hard_clamp_threshold_W`, `hard_clamp_pre_W`, `hard_clamp_post_W`.
  Adversarial review: 4 defects (1 minor coverage gap, 3 nits). All
  closed. Round-2 review skipped (single low-risk test addition).
  Verification: `cargo test --workspace` → 551 passed (340 core + 10
  dashboard-model + 201 shell, +8 vs PR-ZD-3 baseline 543); `cargo
  clippy --workspace --all-targets -- -D warnings` clean; `cd web &&
  ./node_modules/.bin/tsc --noEmit -p .` clean; `cargo build --target
  armv7-unknown-linux-gnueabihf --release` green.
  Defects (4 filed, all closed):
  - D01 (minor) — no test for `zappi_active=false` bypass → added
    `hard_clamp_disengaged_when_zappi_active_false`. Surface
    clarification: with `zappi_active=false`, `evaluate_setpoint`
    itself returns idle (10 W) — soft loop and hard clamp both
    bypassed at that level. Test asserts both setpoint=10 AND
    `hard_clamp_engaged` factor absent.
  - D02 (nit) — placement-rationale doc claim of "circular dep" was
    inaccurate (the claim was in the executor's report only, not in
    code) → resolved note-only.
  - D03 (nit) — redundant `.clone()` on `out.decision` → deferred,
    preserves existing PR-09a-D02 idiom.
  - D04 (nit) — `compensated_drain_w` recomputed when gate cannot
    fire → deferred, cost negligible.
  Notes / surprises:
  - Helper placement: pure formula sits in `setpoint.rs` (with the
    controller that defines its semantics); `&World` wrapper in
    `process.rs` (where the runtime aggregate is consumed). No
    actual circular dep prevented either direction; placement is
    by-domain.
  - When `zappi_active=false`, `evaluate_setpoint` bypasses the
    Zappi soft-loop branch entirely (the existing `if g.zappi_active
    && !g.allow_battery_to_car` gate). The hard clamp's same gate
    therefore acts as a defence-in-depth: even if a future refactor
    accidentally let the soft-loop branch run with `zappi_active=false`,
    the hard clamp would still gate correctly.
  - Test 32 verifies the additive interaction: drain=2000,
    threshold=1000, kp=1.0, hard_clamp=200 → soft adds +1000, hard
    adds +1800 (drain - hard_clamp), grid-cap clips. Combined raise
    is the user-intended belt-and-suspenders.
  Constraints future work must respect:
  - Hard clamp ONLY in Fast mode by design. Eco/Eco+ rely on Zappi's
    CT self-modulation. Do not extend the clamp to other modes
    without explicit operator input.
  - `world.zappi_mode.target.value` (commanded target), NOT
    `world.typed_sensors.zappi_state.value.zappi_mode` (readback).
    Predictive arming is the locked decision.
  - The shared `compute_compensated_drain` helper is the single
    source of truth. Future changes to the formula must update the
    helper, not duplicate inline.

- **PR-ZD-3 — Soft loop** (M-ZAPPI-DRAIN, 2026-04-29) — Replaced the
  PV-only Zappi-active export clamp at
  `crates/core/src/controllers/setpoint.rs:617-637` (both `(2..8)`
  Soltaro carve-out and the daytime `-solar_export` else) with a
  unified compensated-drain control law. The loop now uses
  `compensated_drain = max(0, -battery_dc_power - heat_pump - cooker)`
  as feedback. When drain exceeds `threshold_w`, tightens setpoint by
  `kp × (drain - threshold)`. When drain is below threshold, walks
  setpoint **bidirectionally** toward `-solar_export` at
  `relax_step_w` per tick. The early-morning Soltaro carve-out is
  folded into the unified loop (Soltaro AC export naturally registers
  in the battery power balance). The 23:55-00:00 protection window
  stays untouched.
  Wiring: `SetpointInput` extended with `battery_dc_power`,
  `heat_pump_power`, `cooker_power`, `setpoint_target_prev`.
  `SetpointInputGlobals` extended with the four soft-loop knobs
  (`zappi_drain_threshold_w`, `relax_step_w`, `kp`, `target_w` —
  `target_w` reserved inert for future PI extension; `hard_clamp_w`
  not read here, that's PR-ZD-4). `build_setpoint_input` requires
  `battery_dc_power` Fresh; HP/cooker stale → `0.0`
  (`unwrap_or(0.0)`, locked semantic — clamps tighter on dead bridge).
  `compute_battery_balance::PreserveForZappi` updated to
  `net_battery_w = 0.0` for the Zappi branch (projection
  approximation; cannot replay recurrence dynamics).
  Adversarial review found a **major control-law bug** on round 1:
  the original plan formula `(prev + relax_step).max(-solar_export)`
  is direction-asymmetric — only converges from below. After a single
  tighten cycle that drove setpoint to idle (10 W), the relax branch
  would walk the setpoint AWAY from `-solar_export` indefinitely,
  permanently disabling solar export. **The bug was in the plan**;
  the executor implemented it faithfully. Round-1 review caught it.
  Replaced with bidirectional step-toward construction:
  `if prev < target { (prev + step).min(target) } else { (prev -
  step).max(target) }`. Three existing tests (15, 18, 21) had
  expected values updated to match the corrected gradual walk.
  Verification: `cargo test --workspace` → 543 passed (332 core +
  10 dashboard-model + 201 shell, +14 vs PR-ZD-2 baseline 529);
  `cargo clippy --workspace --all-targets -- -D warnings` clean;
  `cd web && ./node_modules/.bin/tsc --noEmit -p .` clean.
  Defects (13 filed, all closed):
  - D01 (major) — relax-branch direction asymmetry → bidirectional
    step-toward construction.
  - D02 (major) — no multi-tick integration test → added two tests
    (`zappi_active_loop_multi_tick_trajectory`,
    `zappi_active_relax_walks_toward_minus_solar_export`).
  - D03 (minor) — kp=1.0 in every test → added `tighten_scales_with_kp`
    with kp=0.3.
  - D04 (minor) — magic constant `10` instead of `idle_setpoint_w`
    → threaded as parameter through `build_setpoint_input`.
  - D05 (minor) — stale-substitution path untested → resolved
    subsumed by D02.
  - D06 (minor) — windup-clamp untested → resolved subsumed by D02.
  - D07 (minor) — bookkeeping test only relax → added
    `bookkeeping_unchanged_in_tighten_branch`.
  - D08 (minor) — factor names tested not values → added
    `zappi_active_decision_factor_values_correct`.
  - D09 (minor) — early-morning tighten untested → added
    `early_morning_zappi_tightens_when_battery_draining`.
  - D10 (minor) — deadband-stall test → deferred (cross-cutting).
  - D11 (nit) — test 25 breadcrumb → deferred cosmetic.
  - D12 (nit) — target_w inert guard → note-only.
  - D13 (nit) — clamp value not asserted → extended test 16.
  Notes / surprises:
  - The plan's relax formula was wrong. The reviewer caught it on
    round 1 specifically because of the bidirectional-direction
    edge case (post-tighten state with prev ≥ -solar_export). Future
    control-law plans should specify the formula AND its convergence
    behaviour from every initial-condition region.
  - `prepare_setpoint`'s `/50` rounding interacts with the relax
    walk: raw step from prev=10, step=100 produces -90, which
    rounds to -100 (and prepare_setpoint's positive-promotion fires
    only on ≥0, so -90 stays as -100 via rounding).
  - `EXPECTED_FIRST_RUN_EFFECTS` count unchanged (the new sensor
    publishes from PR-ZD-1 already accounted for).
  - `cores.rs::SetpointCore::last_inputs` (display-only) reads
    `HardwareParams::defaults().idle_setpoint_w` because `Topology`
    is not in scope at the `Core` trait method. Acceptable: this
    path is debug/inspection, not actuation; the live actuation
    path in `run_setpoint` reads the actual topology.
  Constraints future work must respect:
  - The bidirectional step-toward formula is the canonical relax
    behaviour. Do not "simplify" back to one-direction max() — that's
    the original bug.
  - `setpoint_target_prev` = `world.grid_setpoint.target.value`
    (the *commanded* target, not the *actual* readback). Reading
    actual would couple the loop to MQTT roundtrip latency.
  - HP/cooker stale → 0 W in the loop. Do not change to "use last
    value" — failing toward conservative is the locked semantic.
  - The 23:55-00:00 Soltaro protection window MUST stay untouched.
    Field-tested protection against a grid quirk; not part of this
    redesign.

- **PR-ZD-2 — Knobs** (M-ZAPPI-DRAIN, 2026-04-29) — Five new knobs
  registered through all 11 CLAUDE.md layers. All `category =
  "config"`, `group = "Zappi compensated drain"`. Inert until PR-ZD-3
  reads them.
  Defaults: `zappi_battery_drain_threshold_w = 1000`,
  `zappi_battery_drain_relax_step_w = 100`,
  `zappi_battery_drain_kp = 1.0`, `zappi_battery_drain_target_w = 0`,
  `zappi_battery_drain_hard_clamp_w = 200`. Dotted MQTT names:
  `zappi.battery-drain.{threshold-w,relax-step-w,kp,target-w,hard-clamp-w}`.
  Wire-format choice: `target_w` is signed (i32) but `KnobValue` has
  no `Int32` variant, so it routes via `KnobValue::Float` (additive,
  no wire-format variant change). `apply_knob` casts back via
  `v.round() as i32` (rounds half-away-from-zero, correct for negative
  references). The other three `_w` knobs use `KnobValue::Uint32`;
  `kp` uses `Float`.
  Adversarial review: zero defects on round 1 — full 5×11 layer
  matrix verified populated, including the easiest-to-forget
  `web/src/displayNames.ts` mapping.
  Verification: `cargo test --workspace` → 529 passed (318 core + 10
  dashboard-model + 201 shell, +8 vs PR-ZD-1 baseline 521); `cargo
  clippy --workspace --all-targets -- -D warnings` clean; `cd web &&
  ./node_modules/.bin/tsc --noEmit -p .` clean; `cargo build --target
  armv7-unknown-linux-gnueabihf --release` green (1m 12s).
  Notes / surprises:
  - `parse_knob_value` merged the `Kp` and `TargetW` Float arms via
    `|` to satisfy `clippy::match_same_arms`. `apply_knob` keeps
    them as separate arms (different field-routing logic) — clippy
    only complained about the parse layer where the shape is
    identical.
  - Discovery test added a new `#[cfg(test)]` module to
    `crates/shell/src/mqtt/discovery.rs` (the file had none
    previously). The new `discovery_includes_zappi_drain_knobs` test
    iterates the 5 IDs and asserts each gets a non-empty schema.
  - `knob_range` exhaustive-match property of the existing pattern
    means a missing arm would fail to compile — partial registration
    can't slip through this layer.
  - `safe_defaults_match_spec_7` extended in-place AND a dedicated
    `safe_defaults_match_spec_zappi_drain` added as a sibling; the
    duplicate-coverage is intentional (the canonical spec test is
    where future maintainers look first).
  Constraints future work must respect:
  - The five knobs are inert in this PR. PR-ZD-3 will read them in
    `evaluate_setpoint`'s Zappi branch; PR-ZD-4 will read
    `hard_clamp_w` in the post-`evaluate_setpoint` Fast-mode clamp.
    Do not introduce reads from any other controller without
    relitigating the design.
  - `target_w` is exposed but documented as inert — the math uses
    `threshold_w` as reference. Reserved for future PI extension.
    Do not "improve" the loop to use `target_w` without confirming
    with the operator first.

- **PR-ZD-1 — Sensors** (M-ZAPPI-DRAIN, 2026-04-29) — Plumbing-only PR
  wiring four new sensors through the full pipeline: `HeatPumpPower`
  and `CookerPower` (zigbee2mqtt MQTT, JSON `.power` field; topics
  `zigbee2mqtt/nodon-mtr-heat-pump` / `zigbee2mqtt/nodon-mtr-stove`
  configured via a new `[zigbee2mqtt]` config section), plus
  `Mppt0OperationMode` and `Mppt1OperationMode` (Victron D-Bus path
  `/MppOperationMode` on `mppt_0` / `mppt_1` services). Index
  orientation locked to existing power-sensor numbering: op-mode 0 ↔
  `mppt_0` ↔ `ttyUSB1` (DI 289); op-mode 1 ↔ `mppt_1` ↔ `ttyS2`
  (DI 274). All four sensors flow into `world.sensors`, decay via
  `apply_tick`, surface on the dashboard sensor table, and have
  human-readable descriptions in `web/src/descriptions.ts`. **Zero
  control-loop coupling** — the four sensors are read by the soft
  loop in PR-ZD-3 and the hard clamp in PR-ZD-4. Two adversarial
  review rounds; round 1 surfaced 9 defects (D01-D09) and round 2
  verified clean.
  Verification: `cargo test --workspace` → 521 passed (312 core + 10
  dashboard-model + 199 shell, +18 vs pre-PR baseline 503); `cargo
  clippy --workspace --all-targets -- -D warnings` clean; `cd web &&
  ./node_modules/.bin/tsc --noEmit -p .` clean.
  Defects (all closed):
  - D01 (major) — MPPT op-mode `[0, 5]` range guard missing →
    `mppt_operation_mode_in_range(v: f64) -> bool` helper added in
    `crates/core/src/process.rs`; out-of-range readings emit
    `Effect::Log{Warn}` and skip `on_reading`, leaving the slot
    Unknown so freshness expires. New test
    `mppt_operation_mode_out_of_enum_range_is_dropped`.
  - D02 (major) — `dashboard_snapshot_surfaces_new_sensors` test
    missing → added `mod snapshot_new_sensors_tests` with two tests
    covering the four-sensor wire-format mapping + the `sensors_meta`
    omit-when-untopiced negative case.
  - D03 (minor) — `_rejects_non_finite` test misnamed → renamed to
    `_rejects_null_power`; new `_rejects_overflow_power` exercises
    `1e400 → INFINITY` against the `is_finite()` guard.
  - D04 (minor) — orchestrator-side ledger checkbox.
  - D05 (minor) — wrong MPPT op-mode descriptions (Volt/Var,
    PowerCtrl, Remote, Ext) → corrected to documented Victron enum
    (0=Off, 1=Voltage-or-current-limited, 2=MPPT-tracking).
  - D06 (minor) — no dispatch-level negative-rejection test →
    extracted `handle_zigbee2mqtt_power_payload` pure helper; both
    HP/cooker dispatch arms call it; 3 new tests
    (`_drops_negative`, `_drops_overflow`, `_emits_event_on_valid`).
  - D07 (minor) — test value 3.0 → 2.0; aligned with D01's clamp.
  - D08 (nit) — closed deferred (cosmetic; `-1.0` and `-50` exercise
    identical guard arm).
  - D09 (nit) — closed note-only (project convention: baboon
    migration stubs are auto-emitted with `todo!()` and never called).
  Notes / surprises:
  - The core crate has no `tracing` dependency, so D01's warn path
    uses `Effect::Log { level: Warn }` (the existing in-process
    logging effect) rather than `warn!()`.
  - Per-service min cadence on solarcharger services unaffected by
    adding `/MppOperationMode` (15 s reseed; `MpptPower*` already
    drives the per-service min at 5 s). New regression test
    `mpp_operation_mode_does_not_shorten_mppt_service_cadence`.
  - Availability topic (zigbee2mqtt `online`/`offline`) treated as
    informational only — no synthetic stale events; relies on
    freshness window. Decision documented in code per round-1
    review.
  - `EXPECTED_FIRST_RUN_EFFECTS` bumped 28 → 32 in
    `crates/core/src/core_dag/tests.rs` to account for the four new
    sensors' boot-time logging.
  - `discover-victron.sh` extended with a focused MPPT-mode probe
    section (the artefact that drove the D-Bus-path discovery).
  Constraints future work must respect:
  - The four sensor slots are now load-bearing for PR-ZD-3 (soft
    loop reads `heat_pump_power` / `cooker_power` / `battery_dc_power`)
    and PR-ZD-4 (hard clamp reads same set). Stale → 0 contribution
    is the locked semantic; do not "improve" by reading last value.
  - MPPT op-mode is observability-only by design. Do not couple it
    into any controller until a follow-up explicitly relitigates
    that decision (the operator wanted observability first to gather
    data before any closed-loop coupling).
  - The `[zigbee2mqtt]` config section is now part of `Config`;
    operators with existing `config.toml` files do not need to add
    it (all four fields are `Option<String>` defaulting to `None`).

- **PR-zappi-schedule-stop** (2026-04-27) — Field regression: last night
  the user had `charge_car_boost = false` and `charge_car_extended =
  true` (Auto path). The Zappi went Fast at 05:00 and stayed Fast past
  08:00 forever, charging the car into the day-rate band. Root cause:
  the legacy Node-RED flow had a separate `00 08 * * *` cron firing a
  `chargeMode: Off` change-node (`legacy/debug/20260421-120500-injects-crons.txt:8`,
  flow node id `f93090cc98e44e37`); the Rust port reproduced the
  Boost / NightExtended cron-windows in `evaluate_zappi_mode` but
  not the standalone Off-edge. Two surgical fixes in one PR:
  (1) **`crates/core/src/controllers/zappi_mode.rs`** — new "post-
  extended stop" rule between the NightExtended block and the night
  auto-stop block: when `now.hour() == 8 && now.minute() <
  POST_EXTENDED_STOP_WINDOW_MINUTES (= 5)` and `current_mode !=
  Off`, return `Set(Off)`; if already Off, return Leave. The Decision
  summary formats the upper-bound minute from the constant, so
  changing the window width keeps the summary honest. 5-minute width
  gives 20 ticks of headroom over the 15s poll cadence; outside the
  window the daytime Leave is unchanged so manual user mode-changes
  during the day still survive.
  (2) **`crates/shell/src/dashboard/convert_schedule.rs`** — new
  `zappi_actions(world, now_local)` emits three daily edges (02:00,
  05:00, 08:00) with `source = "zappi.mode"` and `period_ms =
  Some(DAY_MS)`, mirroring the eddi-edge pattern. Labels reflect knob
  state: `world.knobs.charge_car_boost` for 02:00,
  `victron_controller_core::process::effective_charge_car_extended`
  for 05:00 (which in `Auto` mode reads `bookkeeping.auto_extended_today`),
  always `Off` for 08:00. Wired into `compute_scheduled_actions`
  between the weather-soc edge and the sort. The `WireAction.source`
  is an open string (`crates/dashboard-model/.../scheduled_action.rs`)
  and the frontend (`web/src/render.ts:580`) renders by index, so no
  TS-side changes were needed.
  Verification: `cargo test --workspace --no-fail-fast` →
  283 (core) + 10 (dashboard-model) + 168 (shell) passing, +5 new
  zappi_mode tests + 3 new convert_schedule tests; `cargo clippy
  --workspace --all-targets -- -D warnings` clean.
  Two adversarial review rounds; round 1 surfaced four nits
  (D01–D04, all minor/nit) which were resolved in round 2:
  D01 — formatted summary derives end-minute from the constant so the
  user-facing string can never lie about the window width;
  D02 — added an `Eco`-arm test (`base_input` defaulted to Eco; the
  predicate `current_mode != Off` covers Eco/EcoPlus too);
  D03 — substring assertion changed from `"08:00"` to `"Post-extended"`
  to pin rule identity rather than a digit that recurs in other
  Decision summaries;
  D04 — added a dashboard test pinning the production `Auto`-mode →
  `bookkeeping.auto_extended_today` linkage that the original two
  knob-state tests bypassed via `Disabled` / `Forced` short-circuits.
  Notes / surprises:
  - `Knobs::safe_defaults()` defaults `charge_car_boost = true` and
    `charge_car_extended_mode = Auto` — opposite of what the plan doc
    initially assumed. Tests pin both flags explicitly to keep the
    assertions deterministic.
  - `effective_charge_car_extended` is now a cross-crate import from
    the dashboard read-path. Verified pure (`crates/core/src/process.rs:975-982`):
    `Forced → true`, `Disabled → false`, `Auto → bookkeeping.auto_extended_today`
    — no side effects, safe to call every snapshot tick. The latch
    itself is written once per local date by `maybe_evaluate_auto_extended`
    on the controller path.
  - Idempotency over the 5-minute window relies on the existing TASS
    `propose_target` short-circuit: first tick at 08:00:00 sets
    target=Off (changed=true) and `Effect::CallMyenergi(SetZappiMode(Off))`
    is emitted; subsequent ticks at 08:00:15..08:04:45 with the same
    target/owner return changed=false at `crates/core/src/process.rs:1741`
    and the controller early-returns. No write amplification.
  - DST: the 02:00 zappi/eddi edge gets DST handling for free via the
    existing `next_local_hm` helper (eddi spring-forward test already
    covers it). 08:00 always exists in Europe/London — no extra
    coverage needed.

- **PR-auto-extended-charge** (2026-04-25) — Replace the boolean
  `evcharger.extended.enable` knob with a tri-state
  `evcharger.extended.mode` (`Auto | Forced | Disabled`), default
  `Auto`. New `core::knobs::ExtendedChargeMode` enum, new
  `KnobId::ChargeCarExtendedMode` + `KnobValue::ExtendedChargeMode`
  variants, new `Bookkeeping::auto_extended_today` /
  `auto_extended_today_date` latch fields. New
  `process::effective_charge_car_extended` helper threaded through
  every controller input builder (current_limit, schedules,
  zappi_mode); `process::maybe_evaluate_auto_extended` runs at the
  top of every `apply_tick`, idempotent per local date, fires on the
  first tick at-or-past 04:30 local. Conditions for enable in `Auto`:
  `ev_soc < 40` OR `ev_charge_target > 80`; Stale/Unknown `ev_soc`
  defensively disables. New `SensorId::EvChargeTarget` mirrors
  `EvSoc` (12 h staleness, 60 min cadence, ext-mqtt regime); rename
  `[ev_soc] discovery_topic` config block to `[ev] soc_topic +
  charge_target_topic`. MQTT subscriber generalised to two
  independent two-stage discovery + state subscriptions. Baboon
  bumped within 0.2.0: `Knobs.charge_car_extended` → `_mode:
  ExtendedChargeMode`, `Sensors.ev_charge_target: ActualF64`,
  `Bookkeeping.auto_extended_today + _date_iso`, new
  `Command::SetExtendedChargeMode`. HA discovery: bool switch →
  three-option select. Web `KNOB_SPEC` + display-names + descriptions
  updated. No back-compat for the bool→enum knob; both halves of the
  baboon model deploy together. Verification: `cargo test --all`
  green (271 core, 144 shell), `cargo clippy --all-targets -- -D
  warnings` clean, host + armv7 nix builds clean.

- **PR-writer-reconnect** (2026-04-24) — D-Bus writer reconnect + bounded
  SetValue + lazy infallible constructor (`crates/shell/src/dbus/writer.rs`).
  Resolves **A-56**. Plan:
  `docs/drafts/20260424-2245-pr-writer-reconnect.md`.
  Shape: `Writer::new` pure/infallible; lazy `Connection::system()` with
  exponential backoff (500 ms → 30 s, cap reached in 7 consecutive
  failures); healthy-reset threshold 60 s (backoff resets after the
  first successful write following ≥60 s of healthy operation).
  `tokio::sync::Mutex<WriterInner>` held only for state-mutation spans;
  released for both `Connection::system()` and `SetValue` awaits (per
  round-1 D01). `set_value` extracted as free function taking
  `&Connection`. Separate `last_warn_at` / `last_error_at` dedup fields
  for connect-throttle vs write-failure log streams (per D03).
  `main.rs:137` callsite simplified from `Writer::connect(...).await?`
  to `Writer::new(...)`. Writer intentionally does NOT emit
  `ActuatedPhase{Unset}` — phase management stays core/runtime
  concern; sustained outages rely on subscriber reconnect + freshness
  decay to drive TASS forward once the bus returns (follow-up ticket
  suggested: core demotes phases on `last_readback_at` staleness).
  Review rounds: 2. Round 1 surfaced 5 defects — D01/D02 major (lock
  held across await; premature backoff reset), D03/D04 minor (error
  dedup; fn-pointer infallibility check), D05 nit. All major/minor
  resolved; D05 resolved note-only after round-2 reviewer confirmed
  the `last_warn_at`/`last_error_at` split is clearer, not worse.
  Round 2: clean. Verification: `cargo test --all` → all green
  including 4 writer unit tests (`dry_run_skips_dispatch`,
  `resolve_covers_every_target`, `next_backoff_doubles_capped`,
  `mark_failed_throttles_consecutive_errors`, plus the compile-time
  `const _NEW_IS_INFALLIBLE: fn(DbusServices, bool) -> Writer =
  Writer::new` check); `cargo clippy --all-targets -- -D warnings`
  clean; ARMv7 cross-compile clean.
  Notes / constraints for future work:
  - Keep `Writer::new` infallible. Any future bus-probe must go
    through the lazy-connect path, never eager-fail `new`.
  - `zbus::Connection` is internally `Arc`; cloning the handle is
    cheap and a stale clone fails `SetValue` naturally. Do not
    add a second layer of liveness probing.
  - SetValue-failure error dedup window (`THROTTLED_WARN_DEDUP`,
    5 s) is shared with connect-throttle warns; tune together.
  - Subscriber's similar N-consecutive-failures-escalate-to-error!
    path is intentionally NOT mirrored here (plan §8 defers); add
    only if live-Venus logs show the dedup isn't enough.

- **PR-01** (2026-04-24) — NaN / ±Inf / subnormal / Bool filter in
  `extract_scalar` (crates/shell/src/dbus/subscriber.rs). Resolves A-01,
  A-02. Guard: `Value::F64(f) if f.is_finite() && (*f == 0.0 || f.is_normal())`.
  `Value::Bool` arm deleted. Tests added: NaN / ±Inf / subnormal /
  Bool(true) / Bool(false) / finite negative all rejected where
  appropriate. Verification: `cargo test --all` → 199+46+10+45 ok,
  `cargo clippy --all-targets -- -D warnings` clean, ARMv7 cross-compile
  clean. Review rounds: 1 (6 findings — D01/D04/D05 fixed; D02/D03/D06
  deferred). Notes: `#[allow(clippy::match_same_arms)]` removed; the
  wildcard `_ => None` now handles the non-finite fall-through cleanly.
  Constraint for future work: any new `Value::F64(_)` arm reintroduced
  must preserve the guard. Property test of "random NaN → no actuation"
  deferred to M-AUDIT-2.

- **PR-02** (2026-04-24) — Grid-voltage sanity gate with EN 50160 band
  (crates/core/src/controllers/current_limit.rs). Resolves A-03. Bounds:
  `MIN_SENSIBLE_GRID_V = 207.0`, `MAX_SENSIBLE_GRID_V = 260.0`,
  `NOMINAL_GRID_V = 230.0`. Inclusive-range check; fallback emits a
  Decision factor `grid_v_fallback` when fired. Tests added at exact
  207, 260, plus 179 V (fallback), 270 V over-voltage, 240 V (no
  fallback; asserts 10.0 A). Numeric assertion added to the grid-loss
  test. Review rounds: 1 (7 findings — D01-D06 fixed including major
  upper-bound + floor raise; D07/D08/D09 deferred). Verification: green.
  Constraint for future work: **ET112 grid current sensor is not
  trusted** (phantom amps); derive `grid_current` from `grid_power /
  v_eff` only. Locked architectural note in tasks.md.

- **PR-09a** (2026-04-24) — Symmetric setpoint clamp + `grid_import_limit_w`
  knob (default 10 W). Resolves user ask for configurable [-5000, +10] W
  window. Partial for A-09/A-10/A-34; full hardening in PR-09b.
  Touched: `crates/core/src/knobs.rs`, `types.rs`, `process.rs`,
  `shell/src/mqtt/{serialize,discovery}.rs`, `shell/src/dashboard/convert.rs`,
  `models/dashboard.baboon` (+regenerated), `web/src/knobs.ts`,
  `SPEC.md` §7. 3 Decision factors (pre_clamp_setpoint_W,
  clamp_bounds_W, post_clamp_setpoint_W) emitted always. Review rounds:
  1 (9 findings — D01/D02/D04/D05 deferred as honesty nits, D03 redundant
  test deferred, D06/D07 scope-sprawl misattributed to pre-review-loop
  state, D08/D09 deferred to PR-09b). Verification: green (196+10+45
  tests, clippy, ARMv7, web bundle 26.8kb).

- **PR-05** (2026-04-24) — Observer→live transition invariant.
  Resolves A-06, A-07, A-59. **Closes the last CRITICAL-tier audit
  item.** New method `Actuated<V>::reset_to_unset(&mut self, Instant)`
  in `crates/core/src/tass/actuated.rs` — resets target to Unset
  without touching actual. Every `maybe_propose_*` in process.rs
  (setpoint, current-limit propose block, schedule, zappi_mode,
  eddi_mode) now checks `!world.knobs.writes_enabled` before any
  target mutation; in observer mode emits only
  `Effect::Log { source: "observer", … }` and returns. Decision
  population happens BEFORE the early-return so the dashboard's
  Decision view is honest in observer mode too.
  `Command::KillSwitch(enabled)` captures `prev = world.knobs.writes_enabled`;
  on `!prev && enabled` edge, `reset_to_unset(at)` is called on
  all six actuated entities and six `ActuatedPhase{Unset}` are
  published so the dashboard reflects the transition. `true→true`,
  `false→false`, `true→false` are no-ops. Tests:
  `observer_mode_does_not_mutate_target_phase`,
  `kill_switch_false_to_true_resets_pending_targets_and_forces_rewrite_next_tick`,
  `kill_switch_true_to_true_is_noop`. Existing test
  `observer_mode_logs_decisions_and_publishes_phase` renamed to
  `observer_mode_logs_only_no_target_mutation` and its
  `ActuatedPhase` assertion inverted (it was testing the old broken
  behaviour).  Verification: 202 core + 10 property + 50 shell
  green, clippy -D warnings clean, ARMv7 release ok.
  Constraint for future work: the deadband check in
  `maybe_propose_setpoint` / `run_current_limit` still guards against
  micro-retargets once a target is set — it's compatible with the
  reset pattern because `target.value = None` after reset bypasses
  the deadband on the first re-propose.

- **PR-04** (2026-04-24) — Canonical `classify_zappi_active` + real
  forecast-derived CBE with midnight reset. Resolves A-05, A-15;
  partial A-18. Field-observed bug (user saw cbe=true-by-default on
  fresh boot) eliminated. New module
  `crates/core/src/controllers/zappi_active.rs` holds the single
  canonical classifier consumed by both `compute_derived_view`
  (via `DerivedView`) and `run_current_limit` (via
  `CurrentLimitInputGlobals.zappi_active`, pre-computed in
  `process.rs` and passed in). Threshold canonicalised to
  `evcharger_ac_power > 500 W` per SPEC §5.8. Preserves existing
  current_limit classifier semantics including `ZappiPlugState`
  handling, `Fault`/`Complete` inactivity, and
  `WAIT_TIMEOUT_MIN=5 min` after WaitingForEv. `Bookkeeping` gains
  `charge_battery_extended_today: bool` and
  `charge_battery_extended_today_date: Option<NaiveDate>`;
  `run_weather_soc` writes them at 01:55 from its real forecast
  decision; `apply_tick` clears the flag on day rollover;
  `run_schedules` consumes it as one of two OR-inputs to `cbe`
  (the other is the existing weekly `charge_to_full_required`
  rollover). `!disable_night_grid_discharge` term dropped —
  that was the placeholder that made cbe true by default. Two
  adversarial review rounds; D01 (cross-controller classifier
  disagreement) was the major finding, resolved by sharing the
  function. New tests: `setpoint_first_tick_sees_derived_zappi_active`,
  `setpoint_follows_live_state_over_stale_bookkeeping_zappi_active`,
  `charge_to_full_required_resets_after_midnight_if_weekly_not_active`,
  `cbe_is_false_on_fresh_boot_default`. Verification: 199 core + 50
  shell + 10 property tests green, clippy, ARMv7 release, web bundle.
  Constraint for future work: do not add new zappi_active
  classifications inline in any controller — use
  `classify_zappi_active`. Adding a new `ZappiMode` variant MUST
  preserve the function's exhaustive handling (the reviewer noted a
  defensive-fallthrough `power_active` return currently unreachable
  given 4-variant enum; left in place for future-proofing).

- **PR-06** (2026-04-24) — Retained-knob range + NaN/Inf validation at
  the MQTT boundary; `apply_knob` silent drop promoted to
  `Effect::Log`. Resolves A-08, A-49, A-61. `knob_range()` table in
  `crates/shell/src/mqtt/serialize.rs` (currently duplicating
  `knob_schemas()` in `mqtt/discovery.rs` — PR-06-D01 deferred).
  Helpers `parse_ranged_float` / `parse_ranged_u32` split parse and
  finite-check so NaN / ±Inf emit their own `"knob non-finite;
  dropped"` warn!, separate from the range violation
  `"knob value out of range; dropped"` warn!. A-49 ride-along:
  DischargeTime accepts HH:MM and HH:MM:SS. `apply_knob` catch-all
  now emits `Effect::Log { level: Warn, source: "process::command",
  … }` — preserves the core-crate dependency-free invariant (core has
  no tracing dep; Effect::Log is the established pattern). `apply_knob`
  signature changed to `&mut Vec<Effect>`; two call sites updated.
  Review round 1 flagged D02 (silent NaN drop) + D03 (log wording
  said "retained" on a shared path) as actionable; both fixed in the
  same pass as PR-04's D01/D02/D03/D04/D05. D04 (boundary-accept
  tests), D05 (test count miscount), D06 (process/scope)
  deferred. Verification green alongside PR-04.
  Constraint for future work: range bounds in `knob_range()` must
  stay in sync with `mqtt/discovery.rs::knob_schemas()`; a TODO is
  tracked as PR-06-D01 to make `discovery.rs` consume `knob_range()`
  as the single source.

- **PR-URGENT-14** (2026-04-24) — Retained-knob bootstrap dedup by topic.
  Resolves A-71. Field data showed 5 broker-retained topics redelivered
  ~57× each, inflating `applied` from 11→287. Fix: `HashSet<String>`
  tracks first-seen topic in the bootstrap window; duplicates increment
  a counter and are skipped before decode. Completion log now honest:
  `applied=11, unique_topics=11, duplicates_suppressed=0` expected on
  a clean run; anomalies visible at a glance. Root cause of redelivery
  remains unattributed (rumqttc/Mosquitto session interaction); the
  dedup is robust to whichever it turns out to be. Also removed the
  temporary A-71 diagnostic warn! and its explanatory comment.
  Verification: 199 core + 46 shell + 10 dashboard-model tests green;
  clippy clean; ARMv7 cross-compile ok. Adversarial review round 1
  returned clean with no defects. Constraint for future work: do NOT
  add other HashSets keyed on `String` derived from `p.topic` without
  first considering whether the underlying rumqttc type is `String` or
  `Bytes` — it's currently `String` (rumqttc 0.24.0).

- **PR-URGENT-20** (2026-04-24) — D-Bus session goes silent at t=~20s
  despite PR-URGENT-19. Field bundle
  (`victron-bundle-20260424-192155.txt`) showed ALL 9 Victron
  services time out on GetItems simultaneously at t=~24s after
  startup, AND signals stopped flowing at t=~20s. Not a single
  service hang — the whole zbus session goes dark. Hypothesis:
  500ms poll × 9 services × 18 msgs each = 40+ msg/sec on a single
  D-Bus connection triggers a broker-side eviction or rate-limit
  on the Venus's dbus-daemon. User's feedback made the path
  forward clear: "if that's connection eviction — we MUST make
  sure that if it happens with slower polling our app gracefully
  reconnects."
  Two-part fix landed together:
  **(a) Gentler polling + better observability.**
    - `DBUS_POLL_PERIOD` 500ms → 5s (10× less broker pressure).
    - `ControllerParams::freshness_local_dbus` 2s → 15s (must
      coordinate with poll period — 5s poll with 2s freshness
      would mean sensors perpetually Stale).
    - `HEARTBEAT_INTERVAL` 60s → 20s (faster diagnosis signal;
      revert when field-stable).
    - Heartbeat logs enhanced: `since_start_s`,
      `since_last_signal_s`, `since_last_poll_success_s`. Added
      tracking fields `started_at`, `last_signal_at`,
      `last_successful_poll_at` on the Subscriber struct.
    - Stream errors now logged at warn; stream-end now logged at
      error and triggers reconnect.
  **(b) Graceful reconnect with exponential backoff.**
    - `Subscriber::connect` renamed to `Subscriber::new` (pure
      config, no I/O — clones `DbusServices` + builds routing
      table).
    - `Subscriber::run(tx)` becomes an outer loop calling private
      `connect_and_serve(&mut self, &tx, attempt)`. Backoff 1s →
      30s cap, resets to 1s after a session lasting ≥ 60s
      (`HEALTHY_SESSION_THRESHOLD`).
    - `connect_and_serve` opens fresh `Connection::system()`,
      resolves `GetNameOwner` for each service, subscribes to
      `ItemsChanged`, runs the `tokio::select!` loop. Returns
      `Err` on: (1) `stream.next() → None` (broker dropped us,
      strongest signal), (2) dual-silence (no signals AND no
      successful polls in 30 s after `session_age ≥ 30 s`),
      (3) connection-open / match-rule-subscribe / proxy-build
      failures propagated via `?`.
    - Per-session state (connection, owner_to_service, fail_counts,
      last_warn, message stream) lives as function locals inside
      `connect_and_serve`; persistent state (routes, service set,
      schedule accumulators, cross-session counters, clocks) stays
      on `Self` so heartbeats and readbacks are continuous across
      reconnects.
    - Each reconnect logs `attempt`, `backoff_ms`, `session_age_s`
      so operators can see reconnect storms.
    - Previously: subscriber task ending killed the whole service
      (supervisor restart). Now: recovers in-process, World state
      preserved.
  Touched files: `crates/shell/src/dbus/subscriber.rs` (major
  refactor, ~100 lines churn), `crates/core/src/topology.rs`
  (freshness default), `crates/core/src/process.rs` (one test
  assertion updated to new freshness window), `crates/shell/src/
  main.rs` (`::connect(...).await?` → `::new(...)`).
  Verification: 263 tests green; clippy -D warnings clean; ARMv7
  release ok; web bundle 26.8 kB. Review round 1: 8 concerns (all
  "ship it" — no defects). Preserved: PR-URGENT-19's 2s GetItems
  timeout. Known trade-off: `HEARTBEAT_INTERVAL=20s` is tighter
  than ideal for production; revert to 60s in a follow-up once
  field-stable. Dashboard schedule_0 rendering (user mentioned
  "still disabled") is a separate, tracked UX issue — the target
  column in observer mode does show `{days: 7}` per PR-SCHED0,
  but the actual column shows `{days: -7}` from legacy Node-RED
  leftover state. Not a core bug.

- **PR-URGENT-19** (2026-04-24) — **Real root cause of the field
  wedge.** User added per-thread `/proc/<pid>/task/*/wchan` to
  `fetch-logs.sh` at my request — that diagnostic was decisive.
  Observed thread states on the wedged service:
  ```
  tid=main         wchan=futex_wait_queue   # tokio::select! in main, normal
  tid=tokio-worker wchan=do_epoll_wait      # IDLE worker, no tasks ready
  tid=tokio-worker wchan=futex_wait_queue   # one task parked on a lock
  tid=tracing-appe wchan=futex_wait_queue   # idle, waiting for log, normal
  ```
  One idle + one blocked worker rules out a stdout-pipe wedge
  (both would be in `pipe_write`). So PR-URGENT-18 (tracing
  non_blocking) was real hardening but not the actual bug.
  Root cause: `crates/shell/src/dbus/subscriber.rs::seed_service`
  awaits `proxy.call("GetItems", &()).await` on zbus with NO
  timeout. The subscriber's `tokio::select!` has three arms
  (signal stream, periodic poll reseed, heartbeat). The poll arm
  body iterates all 9 Victron services sequentially. If ONE
  service hangs on its reply (Venus daemon briefly unresponsive,
  D-Bus broker queue, service startup race), `seed_service` parks
  inside the await. The select loop can't re-enter: signals stop
  being consumed, heartbeat stops firing. Sensors decay at the
  2-second freshness window. Controllers bail. Observer-mode logs
  go quiet (stable same-value propose_target returns false). The
  matching 20-s-of-activity-then-silence field symptom is exact.
  This wedge class was called out during PR-URGENT-13's review as
  deferred D08 ("D-Bus wedge on `seed_service()` can still park
  the select loop; PR-URGENT-13b should wrap that call in a
  timeout") and never landed.
  Fix: added `const GET_ITEMS_TIMEOUT: Duration = Duration::
  from_secs(2);` + `tokio::time::timeout(GET_ITEMS_TIMEOUT,
  proxy.call("GetItems", &())).await`. Healthy Venus responds in
  <50 ms; 2 s is 40× headroom. Timeout failure flows through the
  existing error path from PR-URGENT-13 (rate-limited warn at
  30 s, error! escalation at `RESEED_ESCALATE_AFTER=5`
  consecutive fails) so operators see a clear signal before the
  next tick. `Proxy::new` NOT wrapped — verified against zbus
  4.4.0 source, `CacheProperties::Lazily` default skips any
  D-Bus round-trip; it's purely local struct construction and
  can't hang. Tests 275 green, clippy clean, ARMv7 release ok.
  Constraint for future work: EVERY zbus `proxy.call(...).await`
  in this codebase needs a bounded wait. If we add new services
  or new method calls, they get the same timeout pattern.
  Longer-term option: split `seed_service` into parallel
  `FuturesUnordered` over the 9 services so one slow service
  doesn't even delay the others — deferred; per-call timeout is
  sufficient to unwedge the loop.

- **PR-URGENT-18** (2026-04-24) — **Root cause of the persistent
  field wedge:** `tracing_subscriber::fmt::layer()` default writer
  is synchronous `io::stdout()`. Under daemontools (`exec 2>&1`)
  the stdout/stderr pipe has a ~64 KB kernel buffer; when multilog
  briefly slows (tmpfs write, signal, load spike), `write_all`
  blocks whatever thread emitted the tracing event. With only 2
  tokio worker threads (`#[tokio::main(worker_threads = 2)]`), two
  concurrent tracing events can stall BOTH workers → entire async
  runtime freezes. PR-URGENT-15 (MQTT publish timeout) and
  PR-URGENT-17 (log publisher timeout) never fire because the
  worker threads never reach those `.await` points — they're stuck
  inside synchronous `write_all`. `eprintln!` fallback also blocks
  on the same pipe. Each of those three PRs fixed a real bug
  (MQTT queue saturation; WS lock across send; log publisher
  wedge) but they were all SYMPTOMS downstream of the stdout-pipe
  wedge. **Fix:** route `fmt_layer` through `tracing_appender::
  non_blocking(std::io::stdout())`. That wraps stdout with a
  bounded mpsc and drains it on a dedicated BLOCKING thread —
  tokio workers never touch the pipe. The returned `WorkerGuard`
  is bound to `_tracing_guard` at the top of `main` so the drain
  thread survives for the program's lifetime. Touched files:
  `crates/shell/Cargo.toml` (+`tracing-appender = "0.2"`),
  `crates/shell/src/main.rs` (init_tracing returns guard; call
  site binds it). Verification: 50 shell + 212 core + 11 property
  tests green; clippy clean; ARMv7 release ok; web bundle 26.8kB.
  Constraint for future work: NEVER use `tracing_subscriber::fmt`
  with the default writer on a small-worker-count tokio runtime
  under daemontools or any other pipe-based supervisor. Always
  wrap via `tracing_appender::non_blocking`. (`eprintln!` fallbacks
  in `spawn_log_publisher` left as-is — rare diagnostic path; the
  remaining blocking-stderr risk is acceptable vs. the re-entry
  hazard of routing through the same tracing pipeline.)

- **PR-URGENT-17** (2026-04-24) — MQTT log publisher timeout hotfix.
  Caught during adversarial review of PR-URGENT-16. `spawn_log_publisher`
  in `crates/shell/src/mqtt/log_layer.rs` had raw
  `client.publish(...).await` with no timeout. Broker backpressure on
  rumqttc's request channel (even at 4096 slots) → publisher blocks
  → the log-forwarding mpsc (cap 256) fills → subsequent `try_send`s
  drop tracing records silently — including PR-URGENT-15's
  `warn!("mqtt publish stuck >1s; dropping")` diagnostic from
  `Runtime::dispatch`. Self-silencing wedge: the only diagnostic that
  would tell us the runtime was wedged was itself swallowed by the
  wedge. Explains why the field bundle showed zero warn lines despite
  the tick loop being frozen. Fix: `tokio::time::timeout(Duration::
  from_secs(1), client.publish(...))` with `eprintln!` on fire
  (emphatically NOT `tracing::warn!` — that would re-enter MqttLogLayer
  and feed the very wedge we're reporting on). Original publish-error
  `eprintln!` preserved. No rate-limiting; the bounded mpsc bounds
  eprintln rate to one per second of stall. Verified: 275 tests green,
  clippy clean, ARMv7 release ok. Constraint for future work: any
  async code inside `spawn_log_publisher` must use `eprintln!` for
  diagnostics — tracing macros inside this task are a re-entry hazard.

- **PR-12** (2026-04-24) — myenergi HTTP body-error parsing. Before
  this PR: `set_zappi_mode` / `set_eddi_mode` returned `Ok(())` on
  any 2xx, including rejections (myenergi returns 200 with
  `{"zsh": 3}` on rejected zappi commands; similar `esh` for eddi).
  Dashboard confirmed; device didn't change. Credential-empty path
  also silent-succeeded.
  Fix: after `get_json`, pass the body through new helpers
  `interpret_zappi_mode_response` / `interpret_eddi_mode_response`.
  Rules: `zsh`/`esh` integer `0` → `Ok(())`; non-zero → `Err` with
  code in message; missing/non-numeric → `Err("missing/non-numeric
  zsh|esh")`. Rejections log the full body at `warn!` for diagnosis.
  No-credentials / no-serial returns `Err(anyhow!("myenergi not
  configured (…)"))` instead of `Ok(())`.
  `Writer::execute` logs revamped:
  - Dry-run: `info!(?action, "myenergi action (dry-run;
    writes_enabled=false, not sent)")` — honest.
  - Live success: `info!(?action, "myenergi action confirmed (zsh=0
    or esh=0)")`.
  - Live failure: `warn!(?action, error = %e, "myenergi action
    failed")` — covers not-configured, HTTP errors, and body-level
    rejections.
  7 new unit tests (zsh=0, zsh=non-zero, missing, non-numeric; same
  four for esh). Writer log tests via inspection only (can't mock
  HTTP cleanly without a fixture server).
  Explicit non-goal: publishing `Effect::Publish(ActuatedPhase{Unset})`
  on failure (A-22's secondary suggestion) — Writers live shell-side
  and don't speak Effect. That reset signal is a wider refactor for
  a later PR.
  Resolves A-22 + A-23. A-24 (ts-parse sentinel), A-25 (u8
  truncation) still open.
  Verification: 63 myenergi tests + 214 core + 11 property + 50
  shell all green; clippy clean; ARMv7 release ok; web bundle ok.

- **PR-08** (2026-04-24) — `SchedulePartial` take-and-clear semantic.
  Resolves A-12. Previously the accumulator persisted across emits:
  initial GetItems filled all 5 fields → emit Schedule0/1 readback →
  any subsequent single-field ItemsChanged re-emitted a spec with 1
  fresh value + 4 possibly-stale values, which TASS could Confirm
  against a target that didn't match the bus. Fix: introduce
  `take_spec(&mut self)` on the emit path; returns `Some(spec)` iff
  all 5 are present AND clears the accumulator. Next emit requires
  all 5 to be re-observed atomically — via the 300 s settings
  GetItems reseed or a full ItemsChanged envelope carrying all 5.
  Staleness bound: ≤ 300 s (`SEED_INTERVAL_SETTINGS`), well under
  the 900 s Schedule staleness matrix. Existing `as_spec(&self)`
  kept under `#[cfg(test)]` as a read-only peek for other unit
  tests. 2 new unit tests: `schedule_partial_clears_after_emit`
  (fill → take_spec Some → accumulator empty → single field → None →
  complete remaining → Some) and
  `schedule_partial_single_field_update_does_not_re_emit_stale`
  (exact A-12 repro). 56 subscriber tests green; clippy clean;
  ARMv7 release ok; web bundle ok.

- **PR-07** (2026-04-24) — `NameOwnerChanged` watch. Subscribes to
  `org.freedesktop.DBus.NameOwnerChanged` on the same zbus connection,
  with `sender("org.freedesktop.DBus")` filter. On each signal for a
  well-known name in our `service_set`, updates `owner_to_service`
  (removes stale unique bus name, inserts new) and flags the
  service's heap entry with `next_due = now` for immediate reseed.
  Empty `new_owner` (service disappearing) drops the mapping without
  reseeding. Free `handle_name_owner_changed` helper (testable) + 4
  unit tests: rename (`:1.42 → :1.91`), disappear (empty new owner),
  ignored-non-watched (`org.freedesktop.systemd1`), first-appearance
  (empty old owner). New fourth arm in the subscriber's `tokio::
  select!` alongside ItemsChanged / sleep_until_next_due /
  heartbeat. Stream end triggers reconnect via the existing outer
  loop. Addresses A-11 which was a deferred M-AUDIT-2 item: Venus
  services can restart (firmware update, USB replug, user restart
  via GUI) and without this watch all signals from a restarted
  service were silently dropped until the next full subscriber
  reconnect. Review round 1: 2 actionable (tighten sender filter,
  add 4th test) — both fixed in same round. Verification: 278 tests
  green, clippy clean, ARMv7 release ok, web bundle ok. Constraint
  for future work: any new D-Bus service added to `DbusServices`
  automatically benefits — the handler walks `service_set`.

- **PR-CADENCE** (2026-04-24) — Per-path D-Bus cadence + per-sensor
  freshness. Based on research (`docs/drafts/20260424-1959-victron-
  dbus-cadence-matrix.md`) showing NO Victron reference client
  periodically re-polls GetItems — they seed once + rely on
  ItemsChanged. Our 500 ms × 9-service broadcast (~18 calls/s) is
  unprecedented and almost certainly the cause of the ~15 s field
  eviction. Changes:
  - `DBUS_POLL_PERIOD` const → per-service `BinaryHeap<Reverse<
    ServiceSchedule>>` min-heap scheduler. Each service has its own
    `(interval, next_due)`. `select!` poll arm pops earliest-due,
    seeds one service, reschedules. Worst-case load: ~0.14
    GetItems/s across 9 services (vs. 18/s before, 120× gentler).
  - `SEED_INTERVAL_DEFAULT = 60 s`, `SEED_INTERVAL_SETTINGS = 300 s`.
  - `ControllerParams::freshness_local_dbus` deleted. Replaced with
    `SensorId::freshness_threshold(self) -> Duration` const fn
    keyed per variant (5 s fast paths, 10 s grid voltage, 15 s SoC,
    30 s MPPT yield, 900 s SoH + EssState, 3600 s InstalledCapacity,
    40 min OutdoorTemperature).
  - `ActuatedId::freshness_threshold(self) -> Duration` added for
    readback windows (600 s CurrentLimit, 900 s GridSetpoint +
    Schedule0/1). ZappiMode / EddiMode route through
    `params.freshness_myenergi` (single source of truth for myenergi).
  - `POLL_ITERATION_BUDGET` 5 s → 3 s (strictly > `GET_ITEMS_TIMEOUT`
    = 2 s so the outer timeout bounds everything inside `seed_service`
    including `Proxy::new`, not just GetItems).
  - `apply_tick` now decays actuated readbacks (grid_setpoint,
    current_limit, zappi_mode, eddi_mode, schedule_0, schedule_1)
    with per-id thresholds.
  - Dashboard metadata synthesizes per-sensor cadence + staleness.
  Preserved: reconnect loop (PR-URGENT-20), GetItems timeout
  (PR-URGENT-19), poll-iteration budget (PR-URGENT-22), dual-silence
  detection, `HEARTBEAT_INTERVAL = 20 s`. Deferred follow-ups:
  (i) classification logging on each reconnect (rate_limit /
  broker_restart / network / client_defect / unknown); (ii)
  progressive degradation per matrix §"Rate-limit detection &
  response" — only implement if classification logs show recurring
  `rate_limit`; (iii) parallelize the initial seed on reconnect
  (currently sequential with no outer budget — reviewer-flagged D2
  minor). Verification: 275 tests green, clippy clean, ARMv7 release
  ok. Review rounds: 2 (round-1: 5 findings; D1 landmine fixed;
  D3/D4 quick wins; D2 deferred; D5 acceptable). Constraint for
  future work: if a Venus D-Bus service is added or a new path
  routed, update BOTH `SensorId::freshness_threshold` (or
  `ActuatedId::freshness_threshold`) AND the matrix Summary table.

- **PR-URGENT-16** (2026-04-24) — WS initial-snapshot lock scoping
  hotfix. User redeployed PR-URGENT-15 (commit `530f5b6`); field
  regression persisted. Second log bundle
  (`victron-bundle-20260424-175032.txt`) showed NO `mqtt publish
  stuck >1s` warnings — proving MQTT backpressure wasn't the root
  cause this time. Log fell silent after ~15s uptime, service still
  running. Diagnosed by grepping `world.lock().await` call sites:
  `crates/shell/src/dashboard/ws.rs:54-61` held the `MutexGuard`
  across the awaited `send_json()` for the initial-connection
  Snapshot message. Paused / throttled / dead browser tab stalls
  the TCP send → guard never drops → next `Runtime::run` tick
  blocks on `self.world.lock().await` at `runtime.rs:86` → tick
  loop freezes → sensor-stale decay at 2s → controllers bail →
  dashboard shows empty. One-file surgical fix: scope the guard
  to snapshot construction only. Verified: 275 tests green, clippy
  clean, ARMv7 release ok. Constraint for future work: NEVER hold
  `world.lock()` across any `.await` that touches network I/O or
  another async boundary with unknown latency.
  (PR-URGENT-15's 4096-slot queue + 1s publish timeout still
  warranted — it closes a separate wedge class that would have
  surfaced under heavier publish load.)

- **PR-URGENT-15** (2026-04-24) — MQTT publish backpressure hotfix.
  Field-observed wedge: user deployed `3f0821c`, dashboard showed
  all D-Bus sensors Stale + both schedules disabled after 27 s of
  uptime; no heartbeat logs. Root cause: rumqttc's `AsyncClient`
  internal request queue was bounded at 64 slots. Drained only by
  `EventLoop::poll()` on the main task. PR-SCHED0 lifted
  `Effect::Publish(ActuatedPhase)` above the writes_enabled gate,
  so startup emitted ~6 ActuatedPhase + 35 HA discovery + 5 retained-
  knob bootstrap + ongoing MqttLogLayer traffic all sharing that
  64-slot queue. Queue filled → `client.publish(...).await` in
  runtime::dispatch blocked → event channel backed up → subscriber's
  `tx.send(event).await` in the signal arm blocked → poll/heartbeat
  arms of the `tokio::select!` starved → no sensor refresh → sensors
  decayed → controllers bailed.
  Fix: (1) `AsyncClient::new(opts, 4096)` at `mqtt/mod.rs:115-116`
  (per-slot memory cost ~tens of KB on ARMv7, negligible). (2) 1 s
  `tokio::time::timeout` guard around the `Effect::Publish` await in
  `runtime.rs:112-126`; on timeout emits
  `warn!(?payload, "mqtt publish stuck >1s; dropping")` and
  continues — the runtime dispatch loop can never deadlock on a
  publish again. (3) `log_layer.rs:132` already used `try_send`; no
  change. PR-05 (`df3ae4d`) didn't hit this because observer mode
  then skipped `propose_target` entirely; zero `Publish(ActuatedPhase)`
  in observer mode. Verification: 50 shell + 212 core + 11 property
  tests green; clippy clean; ARMv7 release ok; web bundle ok.
  Constraint for future work: NEVER `.await` an MQTT publish from
  the runtime dispatch loop without a timeout. The 1 s budget is
  generous for a healthy broker on the LAN; consider shortening
  after observation. Rate-limited warn on the log publisher's
  try_send drop-counter is still a nice-to-have — deferred.

- **PR-DAG-B** (2026-04-24) — `zappi_active` as a first-class TASS
  derivation core. Completes the user's architectural request:
  "if two TASS cores need to agree on a classifier, the derivation
  should be its own core; cores form a DAG executed in topological
  order". `ZappiActiveCore` (zero-sized struct, `depends_on=[]`)
  writes `world.derived.zappi_active` from a single canonical
  `classify_zappi_active(world, clock)` call per tick. `DerivedView`
  / `compute_derived_view` / `bookkeeping.zappi_active` /
  `CurrentLimitBookkeeping.zappi_active` / every `*InputGlobals.zappi_active`
  field / the `Core::run &DerivedView` parameter all deleted.
  `depends_on` wiring post-PR: ZappiActive `[]`, Setpoint
  `[ZappiActive]`, CurrentLimit `[ZappiActive, Setpoint]`, Schedules
  `[ZappiActive, CurrentLimit]`, ZappiMode `[Schedules]`, EddiMode
  `[ZappiMode]`, WeatherSoc `[EddiMode]`. Topological order
  `[ZappiActive, Setpoint, CurrentLimit, Schedules, ZappiMode,
  EddiMode, WeatherSoc]`. **Semantic choice locked with tests +
  docs:** when both typed Zappi state and `evcharger_ac_power`
  are unusable (Stale/Unknown), `world.derived.zappi_active=false`
  — no cross-tick latching. This is a deliberate departure from
  PR-04's bookkeeping-latched behavior (which masked sensor loss
  because `run_current_limit` early-returned on the freshness gate
  and left the stored global untouched). New behavior surfaces
  sensor loss honestly and is safer — don't hog EV current for a
  car we can't see. Locked by
  `zappi_active_drops_to_false_when_both_sensor_paths_unusable`
  and `zappi_active_uses_power_fallback_when_typed_state_is_stale`
  in `core_dag::tests`. SPEC §5.8 updated. Dashboard wire-compat
  preserved (`ModelBookkeeping.zappi_active` sourced from
  `world.derived`). Tear-down invariants: `rg "DerivedView|
  compute_derived_view|bookkeeping\.zappi_active|bk\.zappi_active"`
  in `crates/core` returns only doc-comment history references;
  `rg "zappi_active" crates/shell` returns one match
  (dashboard/convert.rs) properly sourced from `world.derived`.
  Touched: `crates/core/src/world.rs` (new `DerivedState`; removed
  `Bookkeeping.zappi_active`), `core_dag/{mod.rs,cores.rs,tests.rs}`
  (new `ZappiActiveCore` + semantic edges + 2 new tests),
  `process.rs` (deleted DerivedView/compute_derived_view; updated
  all zappi_active reads to `world.derived`; rewrote 2 A-05 tests),
  `controllers/current_limit.rs` (removed field from
  `CurrentLimitBookkeeping`; rewrote 2 tests),
  `controllers/zappi_active.rs` (doc update),
  `shell/src/dashboard/convert.rs` (wire-compat), `SPEC.md` (§5.8).
  Verification: 212 core + 11 property + 50 shell + 2 new = 275
  tests green; clippy clean; ARMv7 release ok; web bundle 26.8kB.
  Review rounds: 2 (round 1: D01 reviewer-misread plan dismissed,
  D02 real semantic change fixed in round 2; D03 nit — call-counting
  clock assertion — deferred).
  Constraint for future work: do NOT add controller-local calls
  to `classify_zappi_active`. Read `world.derived.zappi_active`.
  PR-DAG-C will add remaining semantic `depends_on` edges for the
  other cross-core bookkeeping reads (`charge_to_full_required`,
  `battery_selected_soc_target`, `charge_battery_extended_today`).

- **PR-DAG-A** (2026-04-24) — TASS core DAG infrastructure. Zero-
  behavior-change refactor wrapping the six existing `run_*`
  controllers as zero-sized-struct `Core` impls with a `CoreRegistry`
  that validates topological order at build time (cycle / missing
  dep / duplicate rejection) via Kahn's algorithm with deterministic
  tie-break (`BTreeMap<CoreId, _>` keyed on discriminant). `depends_on`
  wiring is a linear chain in -A (preserves today's execution order);
  PR-DAG-C will replace with semantic edges derived from the §4 audit.
  Core trait takes `(world, derived, clock, topology, effects)` —
  `&DerivedView` is computed once per tick in `run_all` and passed by
  reference to every core, replacing PR-04's ad-hoc plumbing of
  `DerivedView` through individual function signatures. Only
  `SetpointCore` / `CurrentLimitCore` consume it today; other four
  accept `_derived`. **Regression guard landed:** `AdvancingClock`
  D02 test with a `Cell<NaiveDateTime>`-based clock verified by
  temporary rollback to fail with `"setpoint (factor zappi_active=true)
  and current_limit (bookkeeping.zappi_active=false) disagreed across
  the WAIT_TIMEOUT_MIN boundary"`, then restored. This is the A-05
  hazard PR-04 originally fixed; the D02 test now traps any future
  refactor that re-introduces double-derivation-per-tick.
  Registry `OnceLock` initialized on first `process()` call (lazy;
  infallible for the static production list — `.expect(...)` on invalid
  graph). 7 new tests total (5 registry meta + D02 boundary-consistency
  + D03 tie-break). Touched: `crates/core/src/core_dag/{mod.rs,
  cores.rs,tests.rs}` (new), `crates/core/src/lib.rs` (module export),
  `crates/core/src/process.rs` (pub(crate) on run_* + DerivedView +
  compute_derived_view; `run_controllers` → `registry().run_all(...)`).
  Review rounds: 2 (round 1 blocked on ship-critical D01 — double
  `compute_derived_view` reintroduced A-05 with uncached
  `RealClock::naive()`; round 2 clean with 3 informational notes
  R2-I01..I03). Verification: 212 core + 11 property + 50 shell
  tests green; clippy clean; ARMv7 release ok; web bundle 26.8kB.
  Constraint for future work: any new `Core` impl MUST take
  `&DerivedView` even if unused — signals participation in the
  single-source-of-truth discipline. PR-DAG-B replaces `DerivedView`
  with `world.derived.zappi_active` populated by a dedicated
  `ZappiActiveCore`.

- **PR-SCHED0** (2026-04-24) — Observer-mode target-mutation inversion.
  User-reported regression: on field deploy of `df3ae4d`, schedule_0
  appeared "disabled" on the dashboard. Investigation showed the
  dashboard was rendering `actual.days=-7` (legacy Node-RED residue)
  because observer mode never proposed a target, so there was no
  "target: DAYS_ENABLED" to display. Fix reverses half of PR-05:
  `propose_target` now runs unconditionally; only the physical-write
  effects (`WriteDbus`, `CallMyenergi`, `mark_commanded`, and — via
  D02 — `actual.deprecate`) remain gated behind `writes_enabled`.
  `Effect::Publish(ActuatedPhase)` lifted above the gate so the
  dashboard sees Unset→Pending and Pending→Commanded transitions in
  real time. Touched `crates/core/src/tass/actuated.rs` (split
  `actual.deprecate` out of `propose_target` into `mark_commanded` —
  cleaner TASS contract: target-phase transitions no longer side-
  effect on Actual's freshness machine), all five `maybe_propose_*`
  sites in `crates/core/src/process.rs`, plus extensive test coverage
  across 4 review rounds. New/revised tests: six-actuator observer
  Pending assertion (positive across all cores), zappi_mode BOOST-
  window fixture, observer→KillSwitch(true)→writes real `eff_on`
  assertion with distinct-field HashSet check, property test split
  into negative invariants (random events) + positive prelude (unit
  test). **A-06 regression analysis:** the original A-06 bug was
  "observer-mode target stuck Pending forever". PR-05's fix had two
  parts — observer-mode-skip and KillSwitch-edge-reset. PR-SCHED0
  reverses half of part-1 (target is set), keeps all of part-2 (edge
  reset). Same-value propose_target short-circuits so stuck-Pending
  can still form, but the edge-reset clears it on the flip to live.
  Verified by `schedule_0_observer_then_kill_switch_true_emits_write_dbus_next_tick`
  and by the existing `kill_switch_false_to_true_…` test. Verification:
  205 core + 11 property + 50 shell tests green; clippy clean;
  ARMv7 release ok; web bundle 26.8kB. Review rounds: 4 (round 1:
  5 defects D02-D06; round 2: 4 defects R2-D01..D04; round 3:
  3 defects R3-D01..D03; round 4 clean). One defect deferred to
  M-AUDIT-2 MQTT hygiene: R2-D04 (double-publish dedup via
  `last_published_phase` on `Actuated<V>`).
  Constraint for future work: do NOT put observer-mode-disables on
  any new propose path — the pattern is "propose_target always;
  Publish(ActuatedPhase) always; all other effects gated by
  `writes_enabled`".

- **PR-URGENT-13** (2026-04-24) — Silent stale-sensor observability fix.
  Resolves A-69 (debug!→warn! periodic re-seed failures with 30s
  rate-limit, error! escalation at 5 consecutive fails) and A-70 (mpsc
  256→4096 to absorb 431-event bootstrap flood). Added independent
  60s heartbeat with split raw/routed signal counters, and a 75%
  watermark warn on the event channel. Touched: `shell/src/dbus/
  subscriber.rs` (struct + run loop), `shell/src/main.rs` (channel +
  watermark task). Review rounds: 2 (first round: D01 major fixed —
  heartbeat independent; D02 minor fixed — counter split. Second round:
  D08 minor — heartbeat not starvation-proof from blocking poll-arm
  body, deferred with documented mitigation via `tokio::time::timeout`
  wrap; D09 nit — `routed_signals` counts per-dispatched-path not
  per-signal, deferred as rename). Verification: green (199 passed).
  Constraint for future work: a D-Bus wedge on `seed_service()` can still
  park the select loop; PR-URGENT-13b should wrap that call in a timeout.

---

## Milestone M-UX-1 — PR breakdown

Detail in `./docs/drafts/20260425-0130-m-ux-1-plan.md`. Anchored by a
correctness item (PR-staleness-floor) plus four UX/feature PRs. Wire
format goes 0.1.0 → 0.2.0 (additive only) under PR-session-kwh-sensor;
PR-tass-dag-view rides the same bump or its own minor follow-on.

- [x] **PR-staleness-floor** — Enforce `staleness ≥ 2 × reseed_cadence`
  for slow/reseed-driven sensors via startup assertion + per-variant
  test. Sensor audit found one offender: `BatterySoc` (cadence 60 s,
  staleness 15 s → bumped to 120 s). User-flagged as correctness-tier;
  landed first. 0 review defects after round 1.

- [x] **PR-session-kwh-sensor** — Add `Sensors.session_kwh: ActualF64`
  (sourced from myenergi `che` field via `ZappiState`). Add
  `SensorId::SessionKwh` (ReseedDriven, external-polled, cadence 300 s
  / staleness 600 s; staleness invariant holds). Wire-format bump
  0.1.0 → 0.2.0 (additive); manual converter for `Sensors` between
  versions. Round 1 review caught D01 (major latent) — the
  WorldSnapshot back-compat stub was bypassing the manual converter
  and would have panicked with `missing field 'session_kwh'` on real
  0.1.0 input. Fixed inline + regression test landed.

- [x] **PR-ha-discovery-expand** — Extended HA MQTT discovery beyond
  knobs/phases. 20 `sensor` (19 D-Bus + outdoor_temp + session_kwh)
  and 6 `sensor`/`binary_sensor` for controller-relevant bookkeeping
  (D01: `prev_ess_state` dropped to avoid colliding with the existing
  persistence path). 26 new discovery configs + 26 state topics; ~10
  KB extra retained. New `PublishPayload::{Sensor, BookkeepingNumeric,
  BookkeepingBool}` + `SensorBroadcastCore` (depends on `ZappiActive`
  + `WeatherSoc`; runs last in topo order). Stale →
  `"unavailable"` (HA convention) via the shared
  `encode_sensor_body` helper in core. Dedup on encoded body string
  (D03/D04: avoids noisy republishes from sub-mW rounding and
  Fresh↔Stale flicker). Round 1 review: 6 defects (1 major, 1 minor
  fixed; 2 minor subsumed; 1 nit deferred; 1 trivia). All blockers
  closed.

- [x] **PR-dashboard-ux** — Frontend-only. Items 2 + 3 + 5 from the
  user list: hover descriptions (70 entries); compact identifier-copy
  icon (drops `Identifier` column); boolean badges (filled vs hollow
  disc, neutral colour per round-1 D02 — green/red would imply value
  judgement that's wrong for kill-switch flags like
  `force_disable_export=false`). Wire format unchanged.

- [x] **PR-tass-dag-view** — New dashboard section between Decisions
  and Bookkeeping showing `production_cores()` with `depends_on` edges,
  per-core outcome, and (for `ZappiActiveCore`) last payload. Wire
  format extended within 0.2.0: new `CoreState` / `CoresState` baboon
  types + `WorldSnapshot.cores_state`. `CoreRegistry::run_all`
  clears+repopulates `world.cores_state` after each tick; topo_order
  locked from validated registry order. Bool-typed payloads route
  through the existing `maybeBoolBadge` helper. Back-compat 0.1.0 →
  0.2.0 stub initialises `cores_state` to empty; regression test
  added. Bundle 36.2 → 37.1 KB. 0 review defects (inline review).

### Cross-cutting (M-UX-1)

- Honesty invariant: PR-tass-dag-view's outcome tracking does not
  suppress per-controller Decision writes.
- Three-layer safety chain: HA discovery additions are read-only
  (`sensor` + `binary_sensor` only); no new writable entity surfaces.
- Wire format: 0.1.0 → 0.2.0, additive only. Older clients ignore
  unknown fields per baboon forward-compat.
- Description registry stays frontend-only — different audiences
  from HA discovery payloads.
- MQTT volume: ~26 KB total retained after expansion; FlashMQ
  default tolerances comfortably accommodate.

---

## Milestone M-AS — PR breakdown

Detail in `./docs/drafts/20260425-1947-pr-actuated-as-sensors.md`.

- [x] **PR-AS-A** — Additive infra: new `SensorId` variants
  (`GridSetpointActual` 5s/15s, `InputCurrentLimitActual` 5s/15s,
  `Schedule0/1{Start,Duration,Soc,Days,AllowDischarge}Actual` 60s/180s),
  `SensorId::actuated_id() -> Option<ActuatedId>`,
  `Event::ScheduleReadback` variant. Sensor handler in
  `apply_sensor_reading` gains the post-update `confirm_if` block (per
  user 2026-04-25: live in the sensor handler, not a sibling hook).
  Old `Event::Readback`/`apply_readback`/`Route::*Readback` paths
  remain functional in parallel; this PR is purely additive.
- [x] **PR-AS-B** — Subscriber routing switch: routing table emits
  `Route::Sensor(...)` for grid_setpoint, current_limit, and the 10
  schedule leaf fields; emits `Event::ScheduleReadback` when the
  existing `SchedulePartial` accumulator completes. Delete
  `Route::GridSetpointReadback`, `Route::CurrentLimitReadback`,
  `Route::ScheduleField`, `ScheduleSpecField`, and the
  `ACTUATED_RESEED_*` constants. Per-service `min` cadence on
  `settings` collapses from 300 s to 5 s (driven by GridSetpointActual).
- [x] **PR-AS-C** — Cleanup: delete `apply_readback`, `Event::Readback`,
  `ActuatedReadback`, `ActuatedId::freshness_threshold`, the four
  explicit per-actuated `apply_tick` decay calls. Migrate the three
  remaining tests in `process.rs` and the proptest. ZappiMode test
  moves to the production `Event::TypedSensor` path.

---

## Milestone M-PINNED — PR breakdown

- [x] **PR-pinned-registers** — Persistent enforcement of selected
  Victron D-Bus settings that reset on firmware updates. New
  `[[dbus_pinned_registers]]` config section (path / type / value
  triplets). Shell-side `dbus::pinned` module reads each register
  hourly via `com.victronenergy.BusItem.GetValue` and emits
  `Event::PinnedRegisterReading`. Core-side `apply_pinned_register_reading`
  compares to the configured target with float-tolerant /
  bool↔int(0,1) coercion semantics, increments per-register
  `drift_count`, stamps `last_drift_at` / `last_check`, and emits
  `Effect::WriteDbusPinned` on drift. The new effect goes through
  the same `Writer::dispatch_set_value` chokepoint as the actuator
  `WriteDbus`, so the `[dbus] writes_enabled = false` observer mode
  blocks it (three-layer safety chain preserved). Wire format
  bumped 0.2.0 → 0.3.0 — additive `PinnedRegister` data type plus
  `pinned_registers: lst[PinnedRegister]` on `WorldSnapshot`. New
  Detail-tab section "Pinned D-Bus registers" with status pill
  (red/orange for drifted, green for confirmed, grey for unknown);
  hidden when no entries are configured. SPEC §7.2 documents the
  feature; `config.example.toml` carries a commented-out reference
  set. 6 config-validation tests + 5 pinned-reader tests + 6
  apply-event tests cover the surface. All four verification
  commands green: `cargo test --all` (453 passed), `cargo clippy
  --all-targets -- -D warnings`, `cargo build --target
  armv7-unknown-linux-gnueabihf --release`, `bash
  scripts/build-web.sh`.
