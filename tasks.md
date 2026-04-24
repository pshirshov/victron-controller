# victron-controller — Task Ledger

Authoritative ledger of planned and completed work. Spec: `SPEC.md` in repo
root. Audit findings (seeded 2026-04-24) live in `./defects.md` as `A-NN`
entries.

Status: `[ ]` planned · `[~]` in progress · `[x]` done · `[!]` blocked

---

## Milestones (high-level)

- [~] **M-AUDIT** — Drain the 68 audit findings (A-01…A-68). Priority is
  **physical-safety first** (NaN poisoning, stale-bookkeeping ordering,
  observer→live transition, retained-MQTT range validation), followed by
  honesty-of-state for observer-mode shadow runs. **All CRITICAL-tier
  findings closed as of 2026-04-24 Wave 4.** Remaining: MAJOR/minor/nit
  batchable into M-AUDIT-2.

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
A-15 `charge_to_full_required` latch reset; A-16 forecast freshness filter;
A-17/A-18 Hoymiles solar export + 500 W `zappi_active` fallback; A-25–A-28
myenergi & forecast hardening; A-36 observer-mode `eddi_last_transition_at`
honesty; A-38 MQTT connect log; A-39 dashboard three-gate badge; A-41
fusion NaN filter; A-42 log_layer comment; A-43 Open-Meteo efficiency knob;
A-49 DischargeTime HH:MM:SS; A-50 forecast TZ config; A-53–A-56, A-58,
A-60, A-62–A-68 hygiene + honesty) are **deferred to a follow-up milestone
M-AUDIT-2** unless trivially adjacent to a PR above; the planning subagent
decides which ride along.

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
