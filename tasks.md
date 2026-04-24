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
- [ ] **PR-03** — Zappi `time_in_state` monotonic-Instant fix (resolves
  A-04, A-24).
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
- [ ] **PR-07** — `GetNameOwner` re-resolution on `NameOwnerChanged`
  (resolves A-11).
- [ ] **PR-08** — `SchedulePartial` accumulator clearing (resolves A-12,
  related A-57).
- [x] **PR-09a** — Minimal setpoint clamp: `grid_import_limit_w` knob
  (default 10 W), symmetric `.clamp(-export_cap, +import_cap)`, pre/post-
  clamp Decision factors. Resolves the explicit user ask for a
  configurable [-5000, +10] W window.
- [ ] **PR-09b** — `grid_export_limit_w` hardening follow-up to PR-09a:
  reject `grid_export_limit_w > SAFE_MAX` at ingest, fix the
  export-cap=0 idle-promotion edge case, deadband i64 overflow
  (A-31), dashboard `u32 → i32` truncation (A-34/A-35). Requires
  PR-06's `KnobRange` table; Wave 5. Covers remainder of A-09, A-10.
- [ ] **PR-10** — `force_disable_export` in current_limit: delete the field
  (A-19); revisit clamping semantics in a follow-up PR if the user
  decides it's needed.
- [ ] **PR-11** — Weather-SoC routed through `accept_knob_command`; γ-hold
  honoured; once-per-day guard (resolves A-20, A-21).
- [ ] **PR-12** — myenergi HTTP body-level error parsing (resolves A-22,
  related A-23, A-24).

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

- [~] **PR-DAG** — TASS core DAG orchestrator. Splits into PR-DAG-A
  (infra — zero behavior change), PR-DAG-B (migrate zappi_active →
  `world.derived.zappi_active` + delete `DerivedView`), PR-DAG-C
  (remaining `depends_on` edges for cross-core bookkeeping reads).
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
  - [ ] **PR-DAG-C** — Semantic `depends_on` edges per §4 audit (recommended; deferrable).
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
- [ ] **PR-03** — Zappi `time_in_state` monotonic-Instant fix (A-04, A-24).
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
- [ ] **PR-08** — `SchedulePartial` accumulator clearing (A-12, A-57).
- [ ] **PR-09b** — `grid_export_limit_w` hardening follow-up to PR-09a
  (remainder of A-09, A-10, A-31, A-34/A-35).
- [ ] **PR-10** — `force_disable_export`: delete the unused field (A-19).
- [ ] **PR-11** — weather-SoC routed through `accept_knob_command` +
  γ-hold + once-per-day (A-20, A-21, A-36).
- [ ] **PR-12** — myenergi HTTP body-level error parsing (A-22, A-23).
- [ ] **PR-MISC** — minor/nit hygiene rollup (A-38, A-42, A-43, A-50,
  A-53-A-68 as appropriate).

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
  promotion). `grid_export_limit_w` unchanged (`4900 W`). Ingest clamp
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
