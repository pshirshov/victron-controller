# M-ACTUATED-RETRY — universal actuator retry

## Origin

Field observation 2026-05-01 (post-PR-EDDI-SENSORS-1 root-cause analysis):
every actuator controller in the codebase has the same shape:

```
let changed = world.<X>.propose_target(desired, owner, now);
effects.push(...publish phase...);
if !out.action.should_actuate() || !changed { return; }
...
effects.push(Effect::CallMyenergi/WriteDbus(...));
world.<X>.mark_commanded(now);
```

The `!changed` short-circuit doubles as dedup *and* as suppression of
retries. When the device doesn't comply with a previously-issued
command (firmware refusal, HTTP 5xx, no-op acknowledgement), the
controller sits at `Commanded` with a mismatching `actual` forever and
never re-fires. The eddi-`Pending`-forever bug was the trigger; the
structural hole exists for `grid_setpoint`, `input_current_limit`,
`zappi_mode`, `eddi_mode`, `schedule_0`, `schedule_1`.

User's framing: "We should always reissue commands for all the
actuated values if the actuation does not happen."

## Acceptance criteria

1. New method `Actuated<V>::needs_actuation(now: Instant,
   retry_threshold: Duration) -> bool` on `crates/core/src/tass/actuated.rs`
   returns true when `phase ∈ {Pending, Commanded}` AND
   `now - target.since ≥ retry_threshold`. The value-comparison
   ("does actual still differ from target?") is **deliberately not**
   in this predicate — `confirm_if` is what drives `Commanded → Confirmed`
   with the per-controller tolerance, so by the time `needs_actuation`
   is called from the controller, `phase=Confirmed` already means
   "actual matches target". `needs_actuation` short-circuiting on
   phase=Confirmed is therefore equivalent to "actual matches target".
   This avoids re-deriving tolerance equality for f64 in the predicate.
2. New knob `actuator_retry_s: u32` (default 60). Full 11-layer
   registration per project CLAUDE.md.
3. Each of the five controllers' `if !changed { return; }` gate
   replaced with `if !changed && !world.<X>.needs_actuation(now,
   Duration::from_secs(world.knobs.actuator_retry_s as u64)) {
   return; }`. `should_actuate()` gating preserved for the Leave/Set
   distinction.
4. Tests:
   - `Actuated::needs_actuation` matrix: phase × time-vs-threshold.
     Phase=Unset → false; Phase=Confirmed → false; Phase=Pending,
     time<threshold → false; Phase=Pending, time>threshold → true;
     Phase=Commanded, time<threshold → false; Phase=Commanded,
     time>threshold → true.
   - One per-controller integration test that drives the retry path:
     - Construct world with eddi target=Stopped, mark_commanded fired,
       actual=Normal (mismatch). Advance clock past `actuator_retry_s`.
       Run `run_eddi_mode`. Assert `Effect::CallMyenergi(SetEddiMode(
       Stopped))` is in effects, `mark_commanded` re-fired (target.since
       updated to new now).
     - Same shape for one of the f64-shaped actuators (grid_setpoint or
       input_current_limit) to lock the float-tolerance-doesn't-cause-
       eternal-retry invariant — confirm_if upgrades to Confirmed when
       actual ≈ target within the controller's deadband; needs_actuation
       sees Confirmed and returns false.
5. Verification:
   - cargo test --workspace
   - cargo clippy --workspace --all-targets -- -D warnings
   - cd web && ./node_modules/.bin/tsc --noEmit -p .

## Plan layers (knob registration checklist)

Per project CLAUDE.md, each layer must be touched:

1. **Baboon model** — `models/dashboard.baboon` `Knobs` block: add
   `actuator_retry_s: i32` (or u32 — match the existing knob shape;
   most numeric knobs in the wire model are i32 / f64). Run regen.
2. **Core knobs struct** — `crates/core/src/knobs.rs`: add field +
   default in `safe_defaults()`. Update the `safe_defaults_match_spec_7`
   test if non-trivial.
3. **Core enum** — `crates/core/src/types.rs`: add `KnobId::ActuatorRetryS`.
4. **Core apply_knob** — `crates/core/src/process.rs`: add the
   `(KnobId::ActuatorRetryS, KnobValue::U32(_))` arm.
5. **Shell MQTT serialize** — four sites in
   `crates/shell/src/mqtt/serialize.rs`: `knob_name`, `knob_id_from_name`,
   `knob_range`, `parse_knob_value`.
6. **Shell HA discovery** — `crates/shell/src/mqtt/discovery.rs`
   `knob_schemas`: numeric `number_knob` entry with step + unit
   ("s" for seconds).
7. **Shell config defaults** — `crates/shell/src/config.rs`
   `KnobsDefaultsConfig`: `Option<u32>` field + `set!(...)` line in
   `apply_to`. Document in `config.example.toml`.
8. **Shell dashboard convert** — `crates/shell/src/dashboard/convert.rs`
   two sites: `knobs_to_model` and `knob_id_from_name`.
9. **Web display-name table** — `web/src/displayNames.ts`:
   `actuator_retry_s: "actuator.retry.s"` (matching the dotted
   convention the codebase uses).
10. **Web KNOB_SPEC** — `web/src/knobs.ts`: numeric kind, sensible
    range (e.g. 10..=600), default 60, category `"config"` (this is an
    install-time tuning knob, not a daily-use operator knob), group
    matching the operational-tuning section.
11. **Web descriptions** — `web/src/descriptions.ts`: dotted name →
    "How long to wait after a write before re-issuing the same command
    when actual hasn't matched target. Applies to all actuated values
    (grid setpoint, current limit, zappi/eddi modes, schedules)."

After all 11 layers: cargo test, clippy, tsc, manual dashboard reload
to confirm the knob shows up in the right group with working controls.

## Risks and assumptions

- **Float tolerance**: `Actuated<f64>::confirm_if` (e.g. for
  grid_setpoint) uses the controller's deadband to decide
  Pending→Confirmed. As long as needs_actuation only fires when
  phase ∈ {Pending, Commanded}, a Confirmed actuator never retries
  even if `actual` momentarily drifts ε away from `target`. The
  invariant is: confirm_if owns "does actual match target?";
  needs_actuation owns "is the answer to that question stale?".
- **Schedule equality**: `Actuated<ScheduleSpec>::confirm_if` uses
  struct-equality. ScheduleSpec is Copy+PartialEq. needs_actuation
  doesn't touch the V type — purely phase + time — so this works for
  any V.
- **Race between propose and gate**: the controller flow is
  `propose_target → push phase publish → gate check`. `propose_target`
  on a no-change call doesn't update target.since. So when phase is
  Commanded and time-since-mark exceeds threshold, propose_target
  returns false (`changed=false`) but needs_actuation returns true,
  pushing through to mark_commanded which updates since to now. Next
  tick: changed=false, needs_actuation false (since just got reset).
  Self-stabilising — exactly one retry per threshold-window.
- **Cumulative retry at boot with retained MQTT**: on fresh boot,
  target=Unset, target.since=now (boot). First tick proposes,
  mark_commanded → since=now. needs_actuation returns false until
  threshold elapses. If the first write fails (network not up yet),
  the next retry comes after `actuator_retry_s` not immediately —
  acceptable.
