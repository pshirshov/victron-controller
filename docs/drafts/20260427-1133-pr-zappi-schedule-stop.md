# PR Plan: Zappi extended-charge stop + Zappi schedule edges

PR id: **PR-zappi-schedule-stop**
Owner: review-loop
Date: 2026-04-27

## 1. Problem statement

**Bug 1 — extended charge never stops.** `evaluate_zappi_mode`
(`crates/core/src/controllers/zappi_mode.rs`) sets the Zappi to Fast inside
the NightExtended 05:00–08:00 window when `charge_car_extended` is true.
After 08:00 it falls through to the daytime `Leave` branch, so the Zappi
stays in Fast mode and the car charges into the day rate. The legacy
Node-RED flow had a separate `00 08 * * *` cron that injected
`chargeMode: Off` (`legacy/debug/20260421-120500-injects-crons.txt:8`,
change-node `f93090cc98e44e37`); the Rust port never reproduced that
stop edge.

**Bug 2 — Zappi edges missing from dashboard "Schedules" panel.**
`crates/shell/src/dashboard/convert_schedule.rs` builds the wire payload
from four sources (eddi tariff, schedule.0/1, next_full_charge,
weather_soc). The Zappi has no entries even though three daily edges
(02:00, 05:00, 08:00) are predictable.

## 2. File-level changes

### 2a. `crates/core/src/controllers/zappi_mode.rs` — post-extended stop rule

Insert a new rule between the existing `NightExtended` block (lines 86–97)
and the night auto-stop block (lines 99–120). Rationale: must run after
NightExtended so 07:55 still goes to Fast, but before night auto-stop so
the post-extended rule wins cleanly during 08:00–08:04 (which is now in
the Day band, but defensively placed first).

Anchor — current code immediately after NightExtended block:
```rust
    // 3. Night-time auto-stop. The `<= 65 kWh` gate mirrors the legacy
    // NR behaviour: ...
    let is_night = band.kind == TariffBandKind::Night;
```

New rule (insert above the `let is_night = …` line):
- Compute `now.hour() == 8 && now.minute() < 5` (5-minute window covers
  any briefly-skipped 15s ticks).
- If true and `input.current_mode != ZappiMode::Off`, return
  `ZappiModeAction::Set(ZappiMode::Off)` with
  `Decision.summary = "Post-extended stop window (08:00–08:04) → Off"`
  and the existing `common.factors`.
- If true and already Off, return `Leave` (idempotent).
- Otherwise fall through to the night auto-stop and final daytime
  `Leave` (unchanged).

The existing daytime test `daytime_always_leaves_mode_alone` uses 12:00 —
unaffected.

### 2b. `crates/core/src/controllers/zappi_mode.rs` — new tests (≥3)

Add to the `tests` module:

- `post_extended_stop_window_sets_off_when_currently_fast` —
  `clock_at(8, 0)`, `current_mode = Fast` → `Set(Off)`. Reproduces last
  night's bug.
- `post_extended_stop_window_leaves_when_already_off` —
  `clock_at(8, 2)`, `current_mode = Off` → `Leave`. Idempotency.
- `post_extended_stop_window_ends_at_0805` — `clock_at(8, 5)`,
  `current_mode = Fast` → `Leave`. Window has closed; respect manual
  user setting.
- `post_extended_stop_summary_mentions_window` — assert
  `decision.summary.contains("08:00")` to lock the user-facing wording.

### 2c. `crates/shell/src/dashboard/convert_schedule.rs` — `zappi_actions`

Anchor — current `compute_scheduled_actions` body (lines 55–62):
```rust
    let mut entries: Vec<WireAction> = Vec::new();
    entries.extend(eddi_tariff_actions(now_local));
    entries.extend(schedule_actions(world, now_local));
    entries.extend(next_full_charge_action(world, tz, now_ms));
    entries.extend(weather_soc_action(now_local));
```

Add `entries.extend(zappi_actions(world, now_local));` between
`weather_soc_action` and the `sort_by_key`.

New function next to `eddi_tariff_actions` (the closest stylistic
neighbour):

```rust
fn zappi_actions(world: &World, now_local: DateTime<Tz>) -> Vec<WireAction>
```

Returns three `WireAction`s, all with `source = "zappi.mode"`,
`period_ms = Some(DAY_MS)`:
- 02:00 — `format!("Zappi 02:00 → {}", if world.knobs.charge_car_boost { "Fast" } else { "Off" })`.
- 05:00 — `format!("Zappi 05:00 → {}", if effective_charge_car_extended(world) { "Fast" } else { "Off" })`.
- 08:00 — `"Zappi 08:00 → Off".to_string()` (always).

Each computes `next_fire_epoch_ms` with `next_local_hm(now_local, h, 0)?`
and is filtered with `filter_map` so DST-skipped days are dropped just
like the eddi edges.

Add `use victron_controller_core::process::effective_charge_car_extended;`
next to the existing `victron_controller_core::*` imports.

### 2d. `crates/shell/src/dashboard/convert_schedule.rs` — new tests

- `zappi_actions_emits_three_daily_edges` — UTC, noon, default knobs
  (boost=false, extended_mode=Disabled). Assert 3 entries, all source
  `zappi.mode`, period `Some(DAY_MS)`, labels `"Zappi 02:00 → Off"`,
  `"Zappi 05:00 → Off"`, `"Zappi 08:00 → Off"`. Each next-fire ∈
  (now, now+24h].
- `zappi_actions_label_reflects_knob_state` — Set
  `world.knobs.charge_car_boost = true` and
  `world.knobs.charge_car_extended_mode = ExtendedChargeMode::Forced`.
  Assert labels become `"Zappi 02:00 → Fast"`,
  `"Zappi 05:00 → Fast"`, `"Zappi 08:00 → Off"`.

Update `compute_scheduled_actions_sorts_ascending`: add
`assert!(sources.contains("zappi.mode"));` to the existing source-set
assertions block.

## 3. Acceptance criteria

- [ ] `cargo test -p victron-controller-core` green
- [ ] `cargo test -p victron-controller-shell` green
- [ ] ≥3 new tests in `zappi_mode.rs` (`post_extended_stop_window_*`)
- [ ] ≥2 new tests in `convert_schedule.rs` for `zappi_actions`
- [ ] `compute_scheduled_actions_sorts_ascending` asserts `zappi.mode`
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] No behavioural change to existing rules (boost window,
  NightExtended, night auto-stop, daytime `Leave`) — existing tests
  pass untouched.

## 4. Risks / open questions

- **Window width.** Five minutes (08:00–08:04) is a judgement call.
  Alternatives: single-tick edge (risks missing the stop if 08:00:00 is
  skipped); wider e.g. 08:00–08:30 (more chance of stomping a manual
  user mode-change). 5 min matches 15 s polling with 20 ticks of
  headroom.
- **Always-show vs. omit Zappi 02:00/05:00 when flag is false.** Plan
  emits `"→ Off"` in both branches (parallel to eddi). Alternative is
  to omit. Recommendation: keep always-show; flagged for reviewer.
- **Idempotency with TASS.** The Zappi mode controller already commands
  every tick when target ≠ actual; the idempotent `Leave`-when-Off
  branch is defensive, not load-bearing.
- **DST handling.** Reuses `next_local_hm` which already has eddi DST
  coverage — no new DST tests needed.
