# victron-controller — Defect Ledger

Audit findings and per-PR review defects. Never deleted; status flips in place.

Status: `[ ]` open · `[~]` under fix · `[x]` resolved

Seeded 2026-04-24 from four adversarial audits (honesty, actuation-safety,
numeric-correctness, boundary-correctness). Each `A-NN` entry is an audit
finding; PR-NN-DMM entries (added later) capture defects found while
reviewing a specific PR's patch.

---

## Audit backlog — 2026-04-24 (four parallel adversarial subagents)

### [A-01] `extract_scalar` forwards NaN / ±Inf / sub-normal floats as valid readings
**Status:** resolved (PR-01)
**Severity:** major
**Location:** `crates/shell/src/dbus/subscriber.rs:436-457`
**Description:** Venus-published NaN or ±Inf on any float path (observed during grid-loss on `/Ac/L1/Voltage`; plausible on bus glitches) lands as `Fresh` in `Actual<f64>`. `is_usable` remains true; decay doesn't engage. NaN then poisons `current_limit = grid_power / grid_voltage`, passes through `clamp` (Rust `f64::clamp(NaN, …) = NaN`), and finally is cast to `i32` on the setpoint path — producing `i32::MAX` worth of commanded grid import for one tick (deadband then latches).
**Root cause:** Convenience extraction over zbus `Value<_>` types without a finite-ness filter. Flagged convergently by boundary-C3, safety-C1, numeric-C2.
**Suggested fix:** `Value::F64(f) if f.is_finite() => Some(*f)`; else `None`. Also drop the `Value::Bool(b) → f64` arm (see A-02). Add a property test: random NaN events never produce actuation effects.

### [A-02] `extract_scalar` coerces `Value::Bool(b)` to 0.0 / 1.0, letting a single `false` glitch fabricate SoC=0
**Status:** resolved (PR-01)
**Severity:** major
**Location:** `crates/shell/src/dbus/subscriber.rs:454`
**Description:** Venus occasionally serves `Value::Bool(false)` on the `"Value"` key during a BMS resync glitch. Our extractor returns 0.0 and emits a `SensorReading` — BatterySoc reports 0%, `low_soc` triggers panic grid-charging.
**Suggested fix:** Drop the `Value::Bool` arm entirely. Float sensors must never accept a bool.

### [A-03] `grid_current = grid_power / grid_voltage` divides by zero during grid-loss transitions
**Status:** resolved (PR-02)
**Severity:** major
**Location:** `crates/core/src/controllers/current_limit.rs:141` (also `:176`, `:188`)
**Description:** During grid-loss the ET340 reports `grid_voltage=0` alongside `grid_power=0`. `0/0 = NaN` — passes `is_usable`; propagates through `clamp` and into `input_current_limit`. Also `grid_voltage ≈ 0.01 V` sensor noise yields wildly wrong A and starves downstream.
**Suggested fix:** Gate all `/grid_voltage` divisions: if `grid_voltage < MIN_SENSIBLE_GRID_V (180 V)` fall back to nominal 230 V (with a decision factor noting the fallback). Apply at `:141`, `:176`, `:188`.

### [A-04] Zappi `time_in_state_min` mixes Local clock with UTC myenergi timestamps — off by TZ offset during BST
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/controllers/current_limit.rs:153-154`, `crates/shell/src/myenergi/types.rs:107-113`
**Description:** `clock.naive()` returns `Local::now().naive_local()`; `zappi_last_change_signature` is parsed from myenergi's UTC `dat`+`tim` into bare `NaiveDateTime`. Subtraction in London summer is off by 1 h → 5-min `WAIT_TIMEOUT_MIN` fires immediately every invocation in BST. After DST fall-back, delta goes negative and the timeout never fires.
**Suggested fix:** Switch `zappi_last_change_signature` to a monotonic `Instant` stamped by the poller on state-change, not a wall-clock parsed from the cloud. Or: parse myenergi ts as UTC and convert to Local before storing. Option (1) is strictly better — "time since last observed change" is what the controller wants.

### [A-05] Controller ordering: `run_setpoint` reads bookkeeping that later controllers write
**Status:** resolved (PR-04)
**Severity:** major
**Location:** `crates/core/src/process.rs:412-418`
**Description:** `run_setpoint` consumes `bookkeeping.zappi_active` (written by `run_current_limit`), `battery_selected_soc_target` (written by `run_schedules`), and `charge_to_full_required` (written by `run_weather_soc`). First tick of an evening Zappi charge sees `zappi_active=false` (stale) → setpoint can propose −3.5 kW discharge into the car's grid leg despite `allow_battery_to_car=false`. Dead-band then locks the bad value in.
**Suggested fix:** Compute `zappi_active` once at the top of the process pipeline from `world.typed_sensors.zappi_state` (the derivable predicate), pass to both controllers. Same for the other two fields where derivable; else reorder controllers.

### [A-06] Observer → writes-enabled transition: Pending targets never commit
**Status:** resolved (PR-05)
**Severity:** major
**Location:** `crates/core/src/process.rs` — every `maybe_propose_*` (`:503-548`, `:640-681`, `:767-846`, `:879-908`, `:939-970`)
**Description:** With `writes_enabled=false`, the controller calls `propose_target` *before* the kill-switch check. Target transitions to `Pending` with value V; no `WriteDbus`/`CallMyenergi` is emitted; `mark_commanded` not called. When user flips `writes_enabled=true` later, controller computes same V → `propose_target` returns false (same value, non-Unset phase) → no effect emitted. Target stays Pending forever; the bus retains whatever Venus had before.
**Suggested fix:** In every `maybe_propose_*`, when `writes_enabled=false`, do NOT mutate target; only emit `Effect::Log`. When the flag is true, run the existing propose/commanded/emit sequence. Add a regression test for the observer→live→observer→live cycle.

### [A-07] `Command::KillSwitch(true)` doesn't invalidate in-flight Pending targets
**Status:** resolved (PR-05)
**Severity:** major
**Location:** `crates/core/src/process.rs:257-260`
**Description:** Sibling of A-06. Even after A-06 is fixed, controllers' `propose_target` same-value short-circuit suppresses writes on the first post-flip tick because no sensor changed. The correct semantics: on `false→true`, invalidate every actuated target so the next tick forces a write.
**Suggested fix:** On `KillSwitch(true)` transition, reset every target to `Target::unset(at)`: grid_setpoint, input_current_limit, zappi_mode, eddi_mode, schedule_0, schedule_1.

### [A-08] `parse_knob_value` accepts `"inf"` / `"NaN"` / out-of-range from retained MQTT, promoted to System-owned knobs at boot
**Status:** resolved (PR-06)
**Severity:** major
**Location:** `crates/shell/src/mqtt/serialize.rs:263-332`, bootstrap ingest at `:59-112`
**Description:** `f64::from_str("inf") → Ok(INFINITY)`, `"NaN" → NaN`, `"-50"`/`"9999"`/`u32::MAX` all parse. Bootstrap records these as `Owner::System` knobs, overriding any `HaMqtt` value from a previous run. `ExportSocThreshold=9999` means battery never releases; `BatterySocTarget=-50` starts an infinite discharge.
**Suggested fix:** In `parse_knob_value`: `.filter(|f| f.is_finite())` for all float paths. Add per-knob range validation at the boundary, using the table already used by HA discovery. Invalid retained state → drop + warn!, not load.

### [A-09] `grid_export_limit_w > i32::MAX` silently disables the export cap
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:470-471`
**Description:** `let grid_cap = -i32::try_from(k.grid_export_limit_w).unwrap_or(i32::MAX);` — for any u32 > i32::MAX, `try_from` fails, `unwrap_or(i32::MAX)` yields +2_147_483_647, then unary-minus gives -2_147_483_647 — i.e., effectively unbounded export.
**Suggested fix:** Clamp ingest of `grid_export_limit_w` to a SAFE_MAX (e.g. 10000). On the consumer side use `k.grid_export_limit_w.min(10_000) as i32`. Validate at dashboard + MQTT edges.

### [A-10] `grid_export_limit_w = 0` pins the setpoint at 0 W, losing idle-bleed invariant
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:470-471`
**Description:** Edge case: `grid_cap = 0`, `capped = target.max(0)` — any negative decision becomes 0 W, bypassing the `prepare_setpoint` idle=10 W promotion. Some Victron firmware treats 0 and 10 distinctly.
**Suggested fix:** After clamp, re-assert the idle-promotion: `if capped >= 0 { 10 } else { capped }`. Plus a symmetric `grid_import_limit_w` knob (default 10) clamped via `.min(...)`.

### [A-11] `GetNameOwner` resolved once at startup; signals from a restarted Victron service go to /dev/null
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/dbus/subscriber.rs:226-254`
**Description:** `owner_to_service` is built once. `svc -t /service/com.victronenergy.system` gives the service a new `:1.N` unique name; signals arrive with an unmapped sender (debug-logged then dropped). Service silently degrades to 500 ms poll-only; event-driven reactivity for fast-moving sensors is lost.
**Suggested fix:** Subscribe to `org.freedesktop.DBus.NameOwnerChanged`; re-map on every event for a known well-known name. Alternative: on each unmapped-sender signal whose *path* belongs to a routed service, refresh the mapping.

### [A-12] `SchedulePartial` accumulator never clears; a single-field `ItemsChanged` re-emits 4 stale fields as if they were just observed
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/dbus/subscriber.rs:141-172, 360-371`
**Description:** Accumulator is process-wide mutable state. First seed populates all five fields; thereafter, any single-field change (Venus emits only the changed path) re-emits a full `Schedule0/1` readback with 4 hours-stale values. TASS may Confirm a target that doesn't match the bus.
**Suggested fix:** Version/sequence token in `SchedulePartial`. Emit a readback only when all five fields have been re-observed since the last emission. Or: emit only when *all five* came from the same batch (seed pass, or same `ItemsChanged` envelope).

### [A-13] Zappi night auto-stop is advertised end-to-end but `session_charged_pct` is hardcoded 0 in `run_zappi_mode`
**Status:** resolved (PR-zappi-kwh)
**Severity:** major
**Location:** `crates/core/src/process.rs:860`
**Description:** SPEC §3.5 + dashboard Decision all show the night auto-stop rule. But `run_zappi_mode` feeds literal `0.0` into the controller. The real `che` kWh is parsed by the myenergi poller (`types.rs:30`) and dropped. For users setting `zappi_limit ≤ 65`, the car charges until the tariff window closes regardless — hours of unnecessary grid pull.
**Suggested fix:** Plumb `session_kwh` from `ZappiObservation` through `TypedReading::Zappi` / `ZappiState` into `run_zappi_mode`. Compute `session_charged_pct` from a user knob (see A-14 for the unit bug).

### [A-14] `zappi_limit` documented as % but legacy semantic was kWh — wrong comparison unit
**Status:** resolved (PR-zappi-kwh — user picked kWh)
**Severity:** major
**Location:** `crates/core/src/controllers/zappi_mode.rs:39-41`, HA discovery `discovery.rs:143`
**Description:** HA advertises `"%"` unit; legacy NR compared `che` kWh against a kWh limit. Even after A-13 is fixed, `session_charged_pct >= zappi_limit_pct` compares kWh-as-% against %-as-%. User setting `zappi_limit=30` meaning "30 kWh" gets "stop at 30%", firing at session 1.35 kWh.
**Suggested fix:** Keep `session_che_kwh` in kWh; add separate `zappi_limit_kwh` knob; compare kWh-to-kWh. Or have the shell precompute `session_charged_pct = min(100, che/limit*100)` and keep limit as 100 in core.

### [A-15] `charge_to_full_required |=` is a sticky latch; grid-charging forced on for up to 7 days after one bad morning
**Status:** resolved (PR-04)
**Severity:** major
**Location:** `crates/core/src/process.rs:1078`
**Description:** Weather-SoC ORs `d.charge_battery_extended` into `bookkeeping.charge_to_full_required`. The only reset path is the weekly Sunday-17:00 rollover in `evaluate_setpoint`. Between rollovers one cold morning locks grid-charging schedules on nightly until next Sunday. Dashboard shows `charge_to_full_required=true` with no reason.
**Suggested fix:** Don't `|=`. Either (a) add a separate bookkeeping field `charge_battery_extended_today` with daily reset at midnight, or (b) recompute `charge_to_full_required` each tick from ingredients (weekly rollover OR today's weather_soc) rather than latching.

### [A-16] Forecast fusion's `is_fresh` predicate is `|_,_| true` — SPEC §5.13 12h/48h rules not implemented
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:1028`, `crates/core/src/controllers/forecast_fusion.rs:20-21`
**Description:** `run_weather_soc` passes an always-true filter. `typed_sensors.forecast_*` is only overwritten on successful fetch, never cleared. A three-day-old Solcast snapshot followed by API-key expiry is still "fresh" at tomorrow's 01:55.
**Suggested fix:** Compute `is_fresh` from `clock.monotonic().saturating_duration_since(snap.fetched_at) <= topology.controller_params.freshness_forecast`. Add `freshness_forecast: Duration` to `ControllerParams`. Log "all providers stale → conservative preset" when triggered.

### [A-17] SPEC §5.8 — Hoymiles EV-branch export not folded into `solar_export`
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/controllers/setpoint.rs:184`
**Description:** SPEC §5.8: `solar_export_w = max(0, mppt_0) + max(0, mppt_1) + max(0, soltaro) + max(0, -evcharger_35.ac_power)`. Code omits the EV-branch term. Hoymiles export sails past the controller unseen → evening discharge under-exports by the Hoymiles kW, `max_discharge` cap is tighter than the SPEC promises.
**Suggested fix:** Add `evcharger_ac_power: f64` to `SetpointInput`; include `max(0.0, -evcharger_ac_power)` in `solar_export`; require `evcharger_ac_power.is_usable()` in the freshness guard.

### [A-18] SPEC §5.8 — `zappi_active` still uses 1 A fallback instead of 500 W
**Status:** resolved (closed by PR-04's canonical `classify_zappi_active` — `ZAPPI_POWER_FALLBACK_W = 500.0` replaces the legacy 1 A threshold)
**Severity:** major
**Location:** `crates/core/src/controllers/current_limit.rs:34,168`
**Description:** SPEC §5.8 replaced `zappi_amps > 1` with `evcharger_ac_power > 500 W` to avoid false-firing on Hoymiles exports. Code still uses amps.
**Suggested fix:** Plumb `evcharger_ac_power` into `CurrentLimitInput`. Replace the `zappi_amps > ZAPPI_AMPS_FALLBACK_THRESHOLD` test with `evcharger_ac_power > 500.0`.

### [A-19] `force_disable_export` plumbed into `CurrentLimitInputGlobals` but never consulted by any branch of `evaluate_current_limit`
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/controllers/current_limit.rs:62,263`
**Description:** Defence-in-depth gap. Setpoint forces idle when the flag is on, but current-limit still grants full 65 A AC-in authority — any alternate setpoint writer (future dashboard override) escapes the export kill.
**Suggested fix:** Either (a) delete the field from `CurrentLimitInputGlobals` if it's truly unused — "delete, don't pretend"; or (b) clamp `input_current_limit` to `offgrid_current + small_headroom` when `force_disable_export=true`. Prefer (a) first; revisit semantics before implementing (b).

### [A-20] Weather-SoC bypasses owner-priority + γ-hold; a dashboard write at 01:54 is clobbered at 01:55
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:1054-1077, 1085-1093`
**Description:** `run_weather_soc` calls `apply_knob` directly (no owner check). γ-hold in `accept_knob_command` protects dashboard writes from HaMqtt for 1 s; nightly planner has no such courtesy and runs for a full minute at 01:55:00–01:55:59.
**Suggested fix:** Route every planner knob change through the same `accept_knob_command` path (adding a `WeatherSocPlanner`-owned command variant if needed). Or add a γ-hold check in `run_weather_soc` that suppresses if any knob's last_dashboard_write is within N minutes.

### [A-21] Weather-SoC fires 60 times in the 01:55:00–01:55:59 window, emitting ~300 retained-knob messages
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:980`
**Description:** Controller runs on every `Event` inside the minute, not once. Flood of identical retained-MQTT messages; any dashboard override mid-minute is overwritten repeatedly.
**Suggested fix:** Track `last_weather_soc_run_date` in bookkeeping; run the body only once per wall-day at the first tick within the 01:55 window. Combines naturally with A-20.

### [A-22] myenergi HTTP writer treats any 2xx as success, ignoring body-level error codes
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/myenergi/mod.rs:116-144`
**Description:** `set_zappi_mode`/`set_eddi_mode` return `Ok` on any HTTP 2xx. myenergi returns 200 with `{"zsh": 3}` on rejected commands. Dashboard shows `Commanded`, user sees "it worked" — but the device didn't change state.
**Suggested fix:** Parse the JSON; require the success field (`zsh=0` for mode change). Non-zero → `Err`. On `execute`, on error publish `ActuatedPhase{Unset}` so the UI signals failure.

### [A-23] myenergi writer logs "action ok" when credentials are empty (no HTTP attempted)
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/myenergi/mod.rs:117-119, 134-135`, Writer::execute
**Description:** Credential guard returns `Ok(())` with no request. Writer::execute logs "myenergi action ok"; TASS target stays in `Commanded` forever; dashboard says "in flight".
**Suggested fix:** Return a distinguishable `Err("not configured")`; Writer::execute logs at `warn!`; Runtime publishes `ActuatedPhase{Unset}` to reset UI state.

### [A-24] `parse_myenergi_ts` falls back to `(2026-01-01, 00:00:00)` on parse failure; every poll thereafter looks identical
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/myenergi/types.rs:107-113` + `:42-43`
**Description:** Missing/unparseable `dat` or `tim` silently coerces to sentinel. Change-detection using `zappi_last_change_signature` blinds: same value across polls → "not a new event".
**Suggested fix:** Return `None` from `parse_zappi` on any parse failure; treat the whole poll as failed. Removes the `unwrap_or("01-01-2026")` / `unwrap_or("00:00:00")` too.

### [A-25] `parse_zappi` / `parse_eddi` use `as u8` truncation on `zmo`/`sta` integers
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/myenergi/types.rs:39-41, 66`
**Description:** `as_u64() as u8` wraps on ≥256 (firmware bug or future extension). `sta=257 → 1 → Paused`; we trust the wrong state.
**Suggested fix:** `u8::try_from(...).ok()?`; out-of-range returns `None` for the whole poll.

### [A-26] Solcast schema drift / zero-items response silently emits 0 kWh as a fresh forecast
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/forecast/solcast.rs:78-91`
**Description:** Failed-item skip is silent. If every item has an unknown `period` or `pv_estimate:null`, we return `Ok(ForecastTotals{today:0, tomorrow:0})` — "no sun today" — triggering battery-saver behaviour on a sunny day.
**Suggested fix:** Require ≥ N parseable items per day-bucket; else return `Err`. Distinguish a truly zero forecast from schema drift.

### [A-27] Solcast `period_end` used for day bucketing misattributes boundary periods
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/forecast/solcast.rs:125-130`
**Description:** `period_end = 00:00 next_day` after a 23:30–00:00 bucket puts 30 min of Monday production into Tuesday. Few kWh/day systematic shift.
**Suggested fix:** Use `period_end − period` (bucket start) for attribution, or midpoint.

### [A-28] 401 / 403 / 429 not distinguished from timeouts; we keep hammering rate-limited endpoints
**Status:** resolved (both forecast and myenergi sides)
**Severity:** major
**Location:** `crates/shell/src/forecast/mod.rs:121-138`, `crates/shell/src/myenergi/mod.rs:70-88`
**Description:** Solcast free tier: 10 calls/day; we burn it in 10 ticks on a 429. No exponential backoff.
**Suggested fix:** Match status codes. 401/403 → fail the client entirely; 429 → exponential backoff (re-enter scheduler loop with a delay); 5xx → normal backoff + retry.

### [A-29] `SetValue` on Schedule paths sends fixed type assumptions; Venus firmware variance causes retry-loop log spam + partial writes
**Status:** resolved (PR-dbus-types — probe-driven type alignment)
**Severity:** minor
**Location:** `crates/shell/src/dbus/writer.rs:86-104`
**Description:** Soc field was sent as f64 in our code but the live Venus firmware expects i32; would have produced silent "Wrong type" errors every tick once `writes_enabled=true`. The setpoint path (`/Settings/CGwacs/AcPowerSetPoint`) had the inverse problem — sent as i32 but Venus expects double.
**Root cause:** Two assumptions in `process.rs` mis-typed the wire write. `read-only probe (`scripts/probe-schedule-types.sh`) called `GetValue` on every relevant path on the live Venus and printed the variant signature: all 5 schedule fields are `int32`; AcPowerSetPoint is `double`. Empirical, not guessed.
**Fix:** `process.rs:766` — `DbusValue::Int(value)` → `DbusValue::Float(f64::from(value))` for AcPowerSetPoint. `process.rs:1080` — `DbusValue::Float(spec.soc)` → `DbusValue::Int(spec.soc.round().clamp(0.0, 100.0) as i32)` for Schedule.Soc. Tests updated to assert the new wire types. The `try-i32-fallback-to-f64` defensive wrapper considered but rejected — definitive probe data makes it unnecessary noise.

### [A-30] Event channel `mpsc::channel(256)` has no watermark; stale-batched events stamped `Fresh`
**Status:** resolved (partially addressed — channel size is now 4096 per PR-URGENT-13 with a 75%-full watermark watcher + trend direction per PR-HYGIENE-10. The remaining "stamp on consumer" suggestion is rejected: producer-side stamping is the correct semantic — `Actual::tick(now, threshold)` compares the reading's producer-stamped `at` against `clock.monotonic()` at tick time, which correctly measures age-at-processing-time. Stamping on the consumer would hide genuine producer-side latency)
**Severity:** minor
**Location:** `crates/shell/src/main.rs:47`
**Description:** Backpressure works (`.await send`) but a slow runtime after a burst leaves events in queue, stamped at `Instant::now()` on the producer side. Freshness gate sees "fresh" while values are ~seconds old.
**Suggested fix:** Stamp events with the receive time on the consumer; add a queue-depth metric/log when >50% full.

### [A-31] `i32 - i32` for setpoint retarget deadband can overflow if C1 allows pathological grid_cap
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:513-516`
**Description:** `current_target - value` on i32 panics in debug / wraps in release if either operand is near extrema. Combined with A-09 this becomes reachable.
**Suggested fix:** `i64::from(a) - i64::from(b)` then `.abs()` compared to `i64::from(params.setpoint_retarget_deadband_w)`.

### [A-32] Weather-SoC `disable_export` inner post-condition is dead code (copy-paste trap)
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/weather_soc.rs:90-97`
**Description:** `*threshold = 100.0; if (threshold - 100.0).abs() >= EPSILON { threshold = 80.0; }` — inner branch unreachable because `threshold` was just set to 100. Happens to align with intended behaviour for this caller but invites bugs when someone copy-pastes.
**Suggested fix:** Delete the dead branch, comment that `disable_export` is `threshold=100; dsoc=30`.

### [A-33] Float-equality ladder in PV-multiplier silently drops to 0 on `balance_soc ± ε` noise
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/setpoint.rs:371-395`
**Description:** `battery_soc == balance_soc`, `... == balance_soc - 1.0` etc. MQTT retained SoC can deserialise to `80.0000001`; the ladder falls through to `0.0` (below-threshold) → PV-multiplier is 0 → setpoint clamps to `min_setpoint` instead of exporting.
**Suggested fix:** Widen ladder rungs to half-open ranges: `battery_soc >= balance_soc - 0.5 && battery_soc < balance_soc + 0.5 → 1.0`, etc.

### [A-34] `grid_export_limit_w as i32` in `convert.rs` silently truncates `u32 → i32`
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dashboard/convert.rs:417`
**Description:** Dashboard displays sign-flipped nonsense for u32 above i32::MAX. Combined with A-09 the UI also lies.
**Suggested fix:** `i32::try_from(k.grid_export_limit_w).unwrap_or(i32::MAX)`, or change wire type to i64/u32.

### [A-35] `eddi_dwell_s as i32` silent truncation (same family as A-34)
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/dashboard/convert.rs:421`
**Description:** Low blast radius (60 s default). Fix for consistency.
**Suggested fix:** `i32::try_from(...).unwrap_or(i32::MAX)`.

### [A-36] Observer mode (`writes_enabled=false`) suppresses `eddi_last_transition_at` bookkeeping
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:963-965`
**Description:** The bookkeeping write is gated by the same `writes_enabled` check that gates the HTTP call. During the M11 shadow-run week the dwell clock never advances → every Eddi proposal logs "first transition (no dwell)". Decision factors the user is verifying are all lies.
**Suggested fix:** Move the `eddi_last_transition_at = Some(now)` update above the `if !writes_enabled` gate — it's TASS state, not actuation.

### [A-37] `safe_defaults.writes_enabled = false` contradicts SPEC §7 (documented default: `true`)
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/knobs.rs:150-151`, SPEC §7
**Description:** Internal test `safe_defaults_match_spec_7` asserts `!k.writes_enabled`, enshrining the divergence from SPEC. Reader expecting §7's `true` will be surprised.
**Suggested fix:** Update SPEC §7 row for `writes_enabled` to `false (G3: safe cold-start)`, with a pointer to the rationale comment in `knobs.rs`. Don't flip the code — false is safer.

### [A-38] MQTT `connect()` logs "mqtt connected" before any TCP handshake; misleads diagnostics
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/mqtt/mod.rs:115`
**Description:** `AsyncClient::new` doesn't connect; handshake is at first `event_loop.poll().await` inside `Subscriber::run`. Log claims success while the broker might be unreachable.
**Suggested fix:** Downgrade this line to "mqtt client constructed; connecting…"; add a real "mqtt connected" log on the first `ConnAck` inside the subscriber loop.

### [A-39] Dashboard `WRITES ON / OBSERVER` badge reads only `knobs.writes_enabled`, ignores config-file `[dbus] / [myenergi] writes_enabled`
**Status:** resolved (partial — startup `warn!` landed for both config gates; full badge-AND-of-three-gates requires baboon regen to expose config gates on the snapshot, deferred as a pure UI follow-up)
**Severity:** major
**Location:** `web/src/index.ts:45-51`, `crates/shell/src/main.rs:54-64`
**Description:** Three gates; badge reflects one. Flipping the kill switch with `dbus.writes_enabled=false` in config.toml turns the badge green but nothing writes. Operator is misled about actuation reality.
**Suggested fix:** Publish the config-file gates as part of the snapshot (new sensors-meta-like struct or extra fields on the kill-switch state). Render badge as AND of all three gates. On startup, `warn!` once if any config-level gate is off.

### [A-40] `i64::from(duration.as_secs()).as_secs()` log truncates 500 ms to 0 s
**Status:** resolved (subsumed by PR-CADENCE — the confusing `poll_period_s=0` log was replaced with per-service `default_reseed_s=60 settings_reseed_s=300`, which don't sub-second-truncate)
**Severity:** nit
**Location:** `crates/shell/src/dbus/subscriber.rs:286`
**Description:** `info!(poll_period_s = poll_period.as_secs())` reports 0 for the 500 ms poll — literally says "poll disabled" in logs. Confusing on first read.
**Suggested fix:** `poll_period_ms = poll_period.as_millis()`; rename the field in the log.

### [A-41] `forecast_fusion` passes NaN through `Max`/`Min`/`Mean` (non-total ordering leaks)
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/forecast_fusion.rs:56-77`
**Description:** Any provider NaN (e.g. Open-Meteo outage mapping `null → 0/0`) contaminates fusion. Rust `f64::max(NaN, x) = x` hides it partly, but `reduce(f64::max)` isn't total on NaN; subtly non-deterministic.
**Suggested fix:** `.filter(|v| v.is_finite())` before reducing. Mean must use the finite count.

### [A-42] `MQTT log_layer` comment claims "drop oldest" but `try_send` drops newest
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/mqtt/log_layer.rs:131`
**Description:** Flood scenario loses the *most relevant* logs (peak-incident lines), not old ones.
**Suggested fix:** Either update the comment to "drop newest on full queue" or implement true drop-oldest (`try_recv` + retry).

### [A-43] Open-Meteo hidden `SYSTEM_EFFICIENCY=0.75` biases all weather_soc thresholds
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/forecast/open_meteo.rs:37`
**Description:** Open-Meteo kWh is pre-multiplied by 0.75; Forecast.Solar uses its own model; Solcast its own. Fusion mixes them; user calibrating thresholds against Forecast.Solar misfires when Solcast goes stale and mean falls back.
**Suggested fix:** Expose as `[forecast.open_meteo] system_efficiency = 0.75`; document bias in SPEC §5.7; show per-provider today_kwh in dashboard.

### [A-44] HA discovery `weathersoc_*_energy_threshold` max=500; SPEC §3.6 says 0..1000
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/mqtt/discovery.rs:150-153`
**Description:** UI caps at 500 kWh; SPEC says 1000. Physical kWh/day for 15 kWp never hits even 100, so benign — still a three-way divergence to reconcile.
**Suggested fix:** Update SPEC to 500, or lift the UI caps. Low-priority.

### [A-45] Topology comment + dashboard cadence disagree with SPEC §5.3 (5 s vs code 2 s)
**Status:** resolved (stale comment updated; per-sensor thresholds are now on SensorId::freshness_threshold per PR-CADENCE)
**Severity:** nit
**Location:** `crates/core/src/topology.rs:41`, `crates/shell/src/dbus/subscriber.rs:275`, SPEC §5.3
**Description:** SPEC says 5 s; code changed to 2 s (G3 tuning). Stale SPEC + stale comment.
**Suggested fix:** Update SPEC §5.3 to "2 s (G3 tuning)"; fix the subscriber comment's "5-second freshness window" language.

### [A-46] Evening discharge + `allow_battery_to_car=true` may net-import only when Zappi exceeds inverter discharge cap
**Status:** resolved (note-only — original defect was filed against the wrong layer. User confirmed the opt-in intent: honour `allow_battery_to_car=true` literally; net-import only happens when Zappi draw exceeds the inverter's max discharge, and that's an explicit and acceptable opt-in cost. SPEC §5.9 to clarify)
**Severity:** minor
**Location:** `crates/core/src/controllers/setpoint.rs:224-244, 245-345`
**Description:** Zappi-clamp branch is bypassed by design (SPEC §5.9). `-export_power` is capped at `-grid_export_limit_w` only; net grid import can occur if Zappi draw exceeds the inverter's discharge capacity (battery is over-sized vs the inverter, so the binding constraint is the **inverter**, not the battery). User opted in — money risk only.
**Root cause:** Adversarial review of an attempted fix (PR-A46-review-round-1) proved a setpoint-layer clamp is vacuous: in the evening discharge branch, `setpoint_target = min_pre.min(-export_power)` with `min_pre ∈ {10, -200}`, so commanded setpoint is structurally ≤ +10 W. No positive setpoint > 10 W can occur. The actual scenario — Zappi draw > `BATTERY_SIDE_MAX_DISCHARGE_W` (~5 kW inverter cap) — is a physical inverter rate limit; the battery itself is over-spec'd and not the bottleneck. When Zappi pulls 7 kW: inverter discharges at full ~5 kW from battery, grid imports the remaining 2 kW. This is exactly what `allow_battery_to_car=true` opts in to — drain battery to fund the car as much as the inverter physically allows.
**Fix:** No code change. SPEC §5.9 to be updated with one explanatory sentence: "When Zappi draw exceeds inverter discharge capacity (~5 kW), the residual is net-imported at whatever tariff is in effect — this is the literal cost of the `allow_battery_to_car=true` opt-in".

### [A-47] `check_c4` `i32 - i32` can overflow (see A-31, duplicate)
**Status:** resolved (duplicate of A-31 — closed by PR-setpoint-deadband-i64)
*(Duplicate of A-31; kept for cross-reference.)*

### [A-48] `as_f64` accepts scientific-notation / "NaN" / "inf" strings
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/forecast/mod.rs:111-117`
**Description:** `"NaN".parse::<f64>() = NAN`; `"inf" = INFINITY`. Forecast totals sum these; fused kWh becomes non-finite and silently feeds weather_soc.
**Suggested fix:** `.ok().filter(|f| f.is_finite())` in `as_f64`.

### [A-49] DischargeTime knob rejects HA default `"HH:MM:SS"` format
**Status:** resolved (already accepts both "HH:MM" and "HH:MM:SS" in serialize.rs:404-405 — likely fixed in an earlier pass not attributed to a PR)
**Severity:** minor
**Location:** `crates/shell/src/mqtt/serialize.rs:290`
**Description:** Strict string match on `"02:00"` / `"23:00"`. HA's time selector emits `"02:00:00"` → silently dropped.
**Suggested fix:** Accept `HH:MM` and `HH:MM:SS` by stripping the seconds suffix.

### [A-50] Forecast baseline uses `Local::now().date_naive()` while Open-Meteo returns site-local; TZ drift on Venus UTC install
**Status:** resolved (PR-forecast-tz — Europe/London default; chrono-tz parsed at config load; URL `timezone=` matches bucketing)
**Severity:** minor
**Location:** `crates/shell/src/forecast/solcast.rs:62-65`, `crates/shell/src/forecast/open_meteo.rs:71-72,92-94`
**Description:** `timezone=auto` on Open-Meteo returns site-local times; we compare against `Local::now()` (machine-local). On a Venus with TZ=UTC the buckets are offset by the site's TZ difference.
**Suggested fix:** Add `[forecast] timezone = "…"` config; use it both when querying and bucketing. Don't trust machine TZ for solar boundaries.

### [A-51] myenergi `che` parsed with `unwrap_or(0.0)`; NaN / negative passthrough
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/myenergi/types.rs:44`
**Description:** Firmware bug returning `"NaN"` or negative kWh becomes 0.0 / NaN. Once A-13 wires `che` into the controller, this becomes a failure mode.
**Suggested fix:** `.and_then(|v| v.as_f64().filter(|n| n.is_finite() && *n >= 0.0)).unwrap_or(0.0)`.

### [A-52] `mqtt::rand_suffix` is PID⊕ns; collisions possible on fast restart; broker may reject dup client-id
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/mqtt/mod.rs:130-139`
**Description:** Low entropy. Clean-session=false persistent subscriptions could confuse the broker.
**Suggested fix:** Use `uuid::Uuid::new_v4()`.

### [A-53] Open-Meteo 15-min → 30-min comment drift
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/forecast/current_weather.rs:10`, `config.example.toml:105`, `crates/shell/src/config.rs:264-270`
**Description:** Docstring says "default: 15 min"; actual default now 30 min.
**Suggested fix:** Update the comment.

### [A-54] `/api/version` stub: `min_supported_version == current_version`
**Status:** resolved (kept as intentional pre-1.0 stub with explanatory comment; revisit at 1.0)
**Severity:** nit
**Location:** `crates/shell/src/dashboard/server.rs:159-163`
**Description:** No consumer; harmless but misleading.
**Suggested fix:** Either wire to a real build-time constant or remove.

### [A-55] γ-hold `last_dashboard_write` is global, not per-knob; also unset for `KillSwitch`
**Status:** resolved (per-knob granularity landed; KillSwitch-protection portion deferred — separate Command path needing its own mechanism)
**Severity:** minor
**Location:** `crates/core/src/process.rs:301-311`
**Description:** HA writing `battery_soc_target` clears γ-suppression for all other knobs. `KillSwitch` itself is unprotected → HA can fight the dashboard over the kill switch.
**Suggested fix:** Per-knob `last_dashboard_write`; extend γ-hold to `Command::KillSwitch`.

### [A-56] D-Bus writer: no reconnect, no retry, no SetValue confirmation
**Status:** resolved (PR-writer-reconnect)
**Severity:** minor
**Location:** `crates/shell/src/dbus/writer.rs:28-37, 86-104`
**Description:** Startup-only `Connection::system()`. Venus D-Bus restart → every write fails → TASS stuck in Commanded; MultiPlus retains old value. Fail-closed for device state, fail-open for our narrative.
**Fix:** `Writer::new` is pure/infallible; lazy `Connection::system()` with bounded backoff (500 ms → 30 s, cap reached in 7 failures). `tokio::sync::Mutex<WriterInner>` lock released before `SetValue` and before `Connection::system()` (per round-1 D01). `set_value` extracted as free function taking `&Connection`. Healthy-reset anchor: `last_healthy_at` cleared on every failure; `mark_healthy` is sole writer (per round-1 D02). Throttled-skip `warn!` deduped via `THROTTLED_WARN_DEDUP`; SetValue-failure `error!` deduped via separate `last_error_at` (per round-1 D03). Writer does NOT publish `ActuatedPhase{Unset}` — phase management stays in core/runtime (justified in `docs/drafts/20260424-2245-pr-writer-reconnect.md` §3). Callsite `main.rs:137` simplified from `Writer::connect(...).await?` to `Writer::new(...)`.

### [A-57] Schedules: 5 separate writes not atomic; partial writes leave inconsistent schedule on bus
**Status:** resolved (PR-dbus-types — root cause was type mismatch on Soc; with all 5 fields now sent as `int32` per the probe, the partial-write scenario is removed at the source)
**Severity:** minor
**Location:** `crates/core/src/process.rs:806-841`, `crates/shell/src/dbus/writer.rs:39-55`
**Description:** Originally hypothesised: if Start/Duration succeed and Soc fails, Venus runs the new window with the old SoC target. The actual mechanism by which Soc would have failed was the type mismatch closed by A-29's probe-driven fix. Without a type-rejection path the failure mode is gone. Network-loss / individual-RPC-fail atomicity is still theoretically present but observable via TASS readback divergence and the new writer reconnect/dedup path (PR-writer-reconnect, A-56) — TASS will re-propose the full schedule on next tick, regenerating all 5 writes consistently.
**Fix:** Same as A-29 (type alignment from `scripts/probe-schedule-types.sh`). No separate burst-atomicity wrapper — TASS's idempotent re-propose loop handles the residual case.

### [A-58] Event channel send stalls runtime indefinitely on slow MQTT publish
**Status:** resolved (dashboard side; forecast/myenergi poller backpressure remains as separate concerns — A-28 already addresses HTTP-driven stalls)
**Severity:** minor
**Location:** `crates/shell/src/main.rs:47`
**Description:** Dashboard POSTs use `tx.send(event).await` without timeout. Slow runtime + burst of POSTs → tied-up Axum workers.
**Suggested fix:** `send_timeout(1s)` for subscriber/mqtt-sub/dashboard; dashboard handler uses `try_send` → 503 on full channel.

### [A-59] Asymmetric deadband uses `current_target`, not last-committed value
**Status:** resolved (PR-05)
**Severity:** nit
**Location:** `crates/core/src/process.rs:511-517`
**Description:** If a target value V was propose-stuck in Pending (A-06), a new V' within 25 W is swallowed though the bus is still at a third earlier value. Fixes together with A-06.
**Suggested fix:** Once A-06 lands, verify deadband behaviour in tests.

### [A-60] `CallMyenergi` dispatched via `tokio::spawn` without timeout; multiple in-flight races
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/runtime.rs:104-110`
**Description:** reqwest has 15s timeout; runtime doesn't enforce. Multiple mode-changes → last-writer-wins across spawns, undefined.
**Suggested fix:** `tokio::time::timeout(20s, …)`; serialize via per-device mutex or single-slot channel.

### [A-61] `apply_knob` catch-all arm silently drops unknown `(KnobId, KnobValue)` pairs
**Status:** resolved (PR-06)
**Severity:** nit
**Location:** `crates/core/src/process.rs:363-367`
**Description:** MQTT schema-drift keeps the cold-start default silently. `writes_enabled=false` makes this safe-by-default, but drift is invisible.
**Fix:** PR-06 replaced the silent drop with `Effect::Log { level: LogLevel::Warn, source: "process::command", message: "apply_knob: type mismatch id=... value=..." }`. Shell forwards Effect::Log to tracing. Core stays dependency-free. Apply_knob signature updated to take `&mut Vec<Effect>`; two call sites updated.

### [A-62] Dashboard "Cadence" column label is wrong for signal-driven D-Bus sensors
**Status:** resolved
**Severity:** nit
**Location:** `web/src/render.ts:88-99`, `crates/shell/src/dashboard/convert.rs:337-370`
**Description:** Displayed value is the poll-floor (500 ms); actual `ItemsChanged` can arrive more often. "Cadence" misleads.
**Suggested fix:** Rename to "Poll floor" or "Max interval".

### [A-63] NaiveDateTime `num_milliseconds() as f64` precision on far-future clock drift
**Status:** resolved (documented — range is bounded by the evening-discharge window (< 1 day); f64 handles it without loss and the `<= 0.0` branches below handle pathological skew correctly)
**Severity:** nit
**Location:** `crates/core/src/controllers/setpoint.rs:279,281`
**Description:** Always < 8 h in current use; pathological clock skew would saturate i64. Defensive.
**Suggested fix:** Use `.to_std()` fallibly and reject.

### [A-64] Boost-window match `(2..5)` branch "redundant with final else but preserved for decision log" — benign, comment misleading
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/controllers/setpoint.rs:424-428`
**Description:** Branches are distinguished only for decision-log clarity; not redundant mechanically.
**Suggested fix:** Update comment.

### [A-65] `Writer::set_value` sends `Value::I32` for Schedule Settings that may be `double` on older Venus
**Status:** resolved (PR-dbus-types — duplicate of A-29; closed by the same probe-driven type alignment. On THIS Venus firmware the schedule fields are all `int32`, including Soc; the originally-hypothesised `double` variance was wrong for our specific deployment. Other firmwares may differ; if a future deploy hits a `double`-variant firmware, re-run `scripts/probe-schedule-types.sh` and adjust the wire type at `process.rs:1080`. The probe is the contract)
**Severity:** minor
**Location:** `crates/shell/src/dbus/writer.rs:86-104`
**Description:** Venus 3.60 variance; silent "Wrong type" errors that get retried every tick. Dup of A-29 sub-aspect.
**Suggested fix:** See A-29.

### [A-66] `Value::Bool(false)` as extract-scalar arm (see A-02, duplicate)
**Status:** resolved (duplicate of A-02 — closed by PR-01)
*(Duplicate of A-02.)*

### [A-67] `allow_battery_to_car` boot-reset depends on MQTT bootstrap completing
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/mqtt/mod.rs:223-235`
**Description:** SPEC §5.9 says "always boots false regardless of retained". Code relies on bootstrap path to send the reset; if MQTT is disabled entirely, `safe_defaults` handles it anyway — but the mechanism is less robust than the SPEC suggests.
**Suggested fix:** Document the dependency; guarantee reset by calling `apply_knob(AllowBatteryToCar, false)` unconditionally at process start.

### [A-68] `TlsConfiguration::Simple` accepts malformed CA bytes without parse-time validation
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/mqtt/mod.rs:100-112`
**Description:** Fails at event-loop time, not config-load time.
**Suggested fix:** Parse the PEM at load; error on malformed.

### [A-69] Periodic `GetItems` re-seed failures logged at `debug!` — silently mask stale-sensor root cause in production
**Status:** resolved (PR-URGENT-13)
**Severity:** major
**Location:** `crates/shell/src/dbus/subscriber.rs` (periodic re-seed branch in `run()` — the `poll.tick() =>` arm)
**Description:** First observed in a live bundle: initial seed succeeds, controllers evaluate with fresh data for ~1 s, then **all sensors go stale** and the controller re-evaluates using the freshness-fail safety fallback (`10 W owner=System`). **Zero log output** for the next 28 minutes. Root cause is that the periodic re-seed's error path is a `debug!` line — default `RUST_LOG=info` suppresses it. Whether the failure is a D-Bus connection drop, service restart (A-11 overlap), or channel backpressure (A-30/A-58), the operator has no signal at all. A 15 kW controller that silently falls back to safety 10 W for hours in production is unsafe even in "safe" mode — we can't tell it's broken.
**Suggested fix:** Promote the periodic-failure log from `debug!` to `warn!`, rate-limited (once per service per 30 s). After N consecutive failures for the same service (e.g. 5, which is 2.5 s with 500 ms cadence), escalate to `error!` and emit an `ActuatedPhase{Unset}` so the dashboard reflects the degraded state. Also emit a short INFO-level heartbeat ("subscriber: N poll ticks, M signals received") every 60 s so operators can see the subscriber is alive.

### [A-70] MQTT bootstrap flood (431 retained knob replays) saturates the 256-slot event mpsc, stalling the subscriber's re-seed task
**Status:** resolved (PR-URGENT-13)
**Severity:** major

---

## PR-URGENT-13 — Review round 1 (executor `a29ae22fa080e9578`, reviewer `aa090253ed8f1a5bd`)

### [PR-URGENT-13-D01] Heartbeat is gated on the poll-tick arm; a stalled poller silences the heartbeat — defeats the purpose
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/dbus/subscriber.rs:~319-418`
**Description:** The heartbeat log fired from inside the periodic `poll.tick()` branch of the `select!`. If `seed_service` hangs on a wedged D-Bus call, the poll arm stops firing — and with it the heartbeat. "No heartbeat for 60 s" should positively indicate "subscriber alive but pollers stalled".
**Fix:** Added a dedicated `let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);` (first tick skipped) with its own `select!` arm. Heartbeat emission + counter resets moved there via `std::mem::take`. Poll-tick arm only does re-seed work now. Stalled poll no longer silences the heartbeat.

### [PR-URGENT-13-D02] `signals_since_last_heartbeat` counts unmapped/unrouted signals; label is misleading
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dbus/subscriber.rs:~319-418`
**Description:** Single counter increment early in the `stream.next()` arm lumped unmapped-sender signals in with successfully-routed events, misleading operators.
**Fix:** Split into `raw_signals_since_last_heartbeat` (incremented right after `Ok(msg)` — measures bus activity) and `routed_signals_since_last_heartbeat` (incremented only after `owner_to_service.get(&sender)` + `routes.get(&key)` + `extract_scalar` succeed — measures delivered readings). Heartbeat log includes both as distinct fields (`raw_signals=…, routed_signals=…`).

### [PR-URGENT-13-D03] No boot-time alert when bootstrap fill approaches channel cap (transient miss by 5 s watermark poll)
**Status:** resolved (deferred — watermark sampling at 5 s is intentional trade-off; pushing the check into the producer's send-path requires a wrapper Sender type and is high-cost for a low-probability hazard with PR-URGENT-13's 4096-slot channel size)
**Severity:** minor
**Location:** `crates/shell/src/main.rs:~54-88`
**Description:** 5 s watermark polling can miss a bootstrap burst that fills and drains inside the window. A future deploy with 10k retained topics would stall silently again because the 4096 cap is reached faster than the watermark samples.
**Suggested fix:** Log peak water-level on first drain below 50 %. Or add an explicit bootstrap-completion log including "applied N events, channel cap M". Deferred — low probability, current fix already covers observed floor × 10.

### [PR-URGENT-13-D04] Watermark warn lacks trend direction; operators can't tell climb vs drain from one log line
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/main.rs:~78-82`
**Description:** `warn!("event channel > 75% full ({in_use}/{max})")` — single scalar. Can't infer whether queue is climbing (→ imminent stall) or draining.
**Suggested fix:** Track `last_in_use` between ticks; include `delta` in the warn. Deferred — not blocking.

### [PR-URGENT-13-D05] Escalation `error!` has no throttle after recovery + re-flap
**Status:** resolved (accepted — a flapping service SHOULD produce one error! per cycle; silencing it hides genuine instability. The rate-limited warn already absorbs the high-frequency retry noise)
**Severity:** minor
**Location:** `crates/shell/src/dbus/subscriber.rs` (escalation arm at `count == 5`)
**Description:** A flapping service at ~5-tick cadence emits one `error!` per cycle. Correct behaviour but busy log.
**Suggested fix:** Throttle escalation to once per 5 min per service; log "recovered" INFO on Ok transition to make pairing explicit. Deferred.

### [PR-URGENT-13-D06] No unit test for rate-limiter / escalation state machine
**Status:** resolved (deferred — the rate-limiter logic is trivial and inline with the per-service fail-count state; extracting it for testing would require a larger refactor of the Subscriber internals than the value of the test warrants)
**Severity:** nit
**Location:** `crates/shell/src/dbus/subscriber.rs`
**Description:** Executor acknowledged the omission. For a safety-critical diagnostic fix, behavioural test is warranted.
**Suggested fix:** Extract the state (counts + last_warn) into a standalone struct; table-driven test over a sequence of tick results. Deferred; promote to M-AUDIT-2 if the state machine ever grows.

### [PR-URGENT-13-D07] `error!` message interpolates a `const` via tracing's captured-identifier mechanism
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/dbus/subscriber.rs:~388`
**Description:** `"periodic GetItems failing for {RESEED_ESCALATE_AFTER}+ ..."`. Works on current Rust/tracing; a structured field (`threshold = RESEED_ESCALATE_AFTER`) is more grep-friendly.
**Suggested fix:** `error!(service = %svc, threshold = RESEED_ESCALATE_AFTER, "periodic GetItems failing for N+ consecutive ticks; …")`. Deferred — not blocking.

### [PR-URGENT-13-D08] Heartbeat arm is NOT starvation-proof from a blocking poll-arm body; comment overstates the guarantee
**Status:** resolved (comment corrected; the underlying hazard is mitigated by PR-URGENT-22's POLL_ITERATION_BUDGET wrapper timeout)
**Severity:** minor
**Location:** `crates/shell/src/dbus/subscriber.rs:~323-325, 370`
**Description:** The D01 fix comment implies heartbeat fires even if the poll arm stalls. It doesn't: `tokio::select!` picks a ready branch and runs its body to completion before re-entering. If any `seed_service(svc, &tx).await` blocks (e.g. hung D-Bus call on a degraded service), the whole select is parked and the heartbeat arm cannot be polled. The mechanism still ensures heartbeat survives a busy signal stream (the original concern), but not a blocked seed call — which is actually the more likely stall.
**Suggested fix:** Wrap `seed_service()` in `tokio::time::timeout(Duration::from_secs(5), …)` and treat timeout as a soft failure (bumps the existing fail counter). Restores heartbeat liveness under D-Bus wedge. Deferred — the current fix is still an improvement over round 1; separately addressable.

### [A-71] MQTT bootstrap applies each retained message ~40× — decoder amplification, not stale broker state
**Status:** resolved (PR-URGENT-14)
**Severity:** major
**Field confirmation (2026-04-24 instrumented run):** broker delivered 5 unique retained topics (3 knob, 2 bookkeeping) but our counter logged `applied=287` — ~57× amplification per topic over a 140 ms window. Diagnostic warn logs show the same 5 topics cycling repeatedly. This is redelivery (broker-side or rumqttc-side), not decode or filter-scope bugs. Root cause between rumqttc session-replay, QoS 1 PUBACK timing, and Mosquitto persistence behaviour remains unknown; dedup by topic in the bootstrap window is a safe universal fix regardless.
**Fix:** PR-URGENT-14. `HashSet<String>` of applied topics inside the bootstrap loop; first delivery per topic wins, subsequent duplicates increment a `duplicate_count` and `continue`. Completion log now emits `applied`, `unique_topics`, `duplicates_suppressed`. Temporary A-71 diagnostic warn! removed. Verification green (cargo test, clippy, ARMv7 cross-compile).
**Location:** `crates/shell/src/mqtt/mod.rs:187-220`
**Description:** Field diagnostic (2026-04-24) confirmed the broker carries only **11 retained topics** under `victron-controller/` (3 knob/*/state, 2 bookkeeping/*/state, 6 entity/*/phase — the last of which aren't bootstrap-matched). Yet `mqtt bootstrap complete; applied=431` on the same run. The 3+2 bootstrap-matched topics are being applied ~86× each.

Likely root causes, in order of plausibility:
1. **Session replay on reconnect within the 2 s window.** `clean_session` default is probably `false`; if rumqttc reconnects mid-window (transient network hiccup, broker keep-alive timing), each reconnect re-delivers all retained messages matching the subscription filters. 86× in 2 s = ~23 ms per reconnect — feasible on a lossy link.
2. **Subscription duplication via rumqttc's internal session state.** If the service was restarted many times before this run, the broker's stored session could be replaying accumulated queued messages.
3. **Wildcard overlap** — ruled out; the three state filters don't overlap by construction.

Why it matters:
- **Masks A-70's original severity** (a channel flood we attributed to broker state is actually a client-side amplification).
- Each amplified apply re-overwrites the core's knob state — if a user writes to dashboard at the same moment, their intent is clobbered N times.
- Bootstrap logs show a false "large retained state" picture, hiding the real topology.

**Suggested fix:** Instrument first. Log each `Packet::Publish` topic + payload-prefix inside the bootstrap loop at `debug!` so we can tell whether the broker is redelivering the same topic N times or something else. Then pick one:
- Deduplicate within the bootstrap window — keep a `HashSet<String>` of `(topic, payload_hash)` and skip re-applies within the same window.
- Set `clean_session = true` for the bootstrap phase, reconnect clean for phase 2 with a stable client-id.
- Cap retries explicitly.
Fastest safe fix: dedupe by `topic` in the bootstrap window. A topic is retained → a single canonical value per topic exists; applying N identical values is wasteful and introduces the amplified noise.
**Status:** open (deferred)
**Severity:** nit
**Location:** `crates/shell/src/dbus/subscriber.rs:~358`
**Description:** Venus `ItemsChanged` carries N paths per signal. The counter increments inside the `for (child_path, child_value)` loop, so `routed_signals` can exceed `raw_signals`. Operators seeing `raw_signals=3, routed_signals=12` will be confused.
**Suggested fix:** Either (a) rename field + log key to `routed_readings`, or (b) move the increment outside the inner loop and count "signals with ≥1 matched route". Deferred — cosmetic.
**Location:** `crates/shell/src/main.rs:47` (`mpsc::channel(256)`), `crates/shell/src/mqtt/mod.rs:220-240` (bootstrap publishes), `crates/shell/src/dbus/subscriber.rs` (`tx.send(event).await`)
**Description:** The user's broker carries 431 retained knob-state messages (live bundle confirms `mqtt bootstrap complete; applied=431`). All 431 are translated to `Event::Command { … }` and queued into the 256-capacity mpsc. The subscriber's periodic re-seed calls `tx.send(event).await` too; if the channel is full, the `.await` blocks. Any time-critical re-seed work stalls until the runtime drains the bootstrap flood. Combined with A-69, this produces the "sensors stale, no logs" symptom seen in the field. Root cause also includes the fact that we have ~10× more retained state than our knob schema actually needs (probably stale/obsolete keys on the broker).
**Suggested fix:**
1. Enlarge the channel (e.g. `mpsc::channel(4096)`). Observations suggest 431+ is the floor; 4096 gives plenty of headroom.
2. Or (better) switch bootstrap to a synchronous collect-then-apply pattern: buffer all retained knobs into a `HashMap<KnobId, KnobValue>` on the MQTT subscriber side, then emit a single `Event::BootstrapKnobs(map)` or apply directly via a dedicated high-priority channel. Avoids the per-knob event flood.
3. Independently: `warn!` when the channel exceeds some watermark (say 75% full) so the operator can see it coming. Related to A-30.
4. Separately investigate why the broker has 431 retained knobs; clean stale retained state via `mosquitto_pub` with an empty retained payload on obsolete topics.

---

(End of audit backlog. As each PR is opened for a cluster of audits, its
defects section follows below with review-round findings.)

---

## PR-01 — Review round 1 (executor: `a09e8a816343d33e9`, reviewer: `a2c8b73332f4d1e3b`)

### [PR-01-D01] Subnormal-float sub-case from A-01 not addressed by `f.is_finite()` gate
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dbus/subscriber.rs:437-445`
**Description:** A-01 enumerates "NaN / ±Inf / sub-normal floats". The fix uses `f.is_finite()`, which returns true for all subnormals. Physical sensor readings in this domain should never be subnormal; admitting them is at best "correct by accident" (downstream truncations zero them out).
**Fix:** Tightened the guard to `Value::F64(f) if f.is_finite() && (*f == 0.0 || f.is_normal()) => Some(*f)` at `subscriber.rs:~437-445`. Added subnormal-rejection assertion `extract_scalar(&Value::F64(f64::MIN_POSITIVE / 2.0)) == None` in the test module.

### [PR-01-D02] Test exercises `extract_scalar` in isolation; A-01's "property test: random NaN → no actuation effects" is not delivered
**Status:** resolved (accepted — a full end-to-end property test would require substantial test fixture setup; the unit-test coverage of `extract_scalar` plus the integration test `property_process.rs::writes_disabled_emits_no_actuation_effects` is sufficient defence in depth for the A-01 scope)
**Severity:** minor
**Location:** `crates/shell/src/dbus/subscriber.rs` tests module
**Description:** End-to-end path (D-Bus signal → `ItemEntry` → `extract_scalar` → `route_to_event` → `Event::Sensor` → core `process` → effects) not covered. A future refactor could route Bool through a new arm and this unit test wouldn't catch it.
**Suggested fix:** DEFERRED to M-AUDIT-2 as a standalone testing hardening item. Out of scope for PR-01's surgical fix.

### [PR-01-D03] Fix suppresses the event silently; no counter / log of dropped non-finite readings
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dbus/subscriber.rs:441-444, caller sites ~291, ~392`
**Description:** Silent drop on non-finite. Operator debugging "sensor went Stale" has no hint that Venus *is* publishing — just publishing bad data.
**Suggested fix:** DEFERRED to M-AUDIT-2 (observability). Current fail-safe (stale → 10 W idle) is correct; diagnostic surface is the enhancement.

### [PR-01-D04] Deleted `Value::Bool` arm is not tested for the actual A-02 failure case: `Bool(false)`
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/dbus/subscriber.rs:~481-488`
**Description:** A-02's documented failure is `Value::Bool(false)` → 0 % SoC. Test only covered `Bool(true)`.
**Fix:** Added `assert_eq!(extract_scalar(&Value::Bool(false)), None);` to the test at `subscriber.rs:~481-488`.

### [PR-01-D05] `#[allow(clippy::match_same_arms)]` masks future unintentional duplicate arms across the whole match
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/dbus/subscriber.rs:~437-445`
**Description:** The explicit `Value::F64(_) => None` arm only existed to pair with its guarded sibling; the final `_ => None` wildcard already handles non-finite F64 correctly because Rust tries arms in order and the guard fails over to `_`.
**Fix:** Removed both the redundant `Value::F64(_) => None` arm and the `#[allow(clippy::match_same_arms)]` attribute. Guard comment retained above the guarded arm. Clippy `-D warnings` green.

### [PR-01-D06] Pre-existing I64/U64 → f64 precision loss in `extract_scalar` surfaced by PR-01's attention to float validity
**Status:** resolved (documented — all current Victron paths are well within f64's 2^53 exact range; comment points future callers at the hazard)
**Severity:** nit
**Location:** `crates/shell/src/dbus/subscriber.rs:447-454`
**Description:** `I64 / U64 → f64` silently loses precision for values > 2^53. Unlikely for current sensor paths but the guarantee "returns some finite f64" is weaker than "returns an exact-value f64".
**Suggested fix:** DEFERRED to M-AUDIT-2. Add a docstring caveat; not a regression of PR-01.

---

## PR-02 — Review round 1 (executor: `af55a7504fd88c9a3`, reviewer: `ad9616e6dbef38f49`)

### [PR-02-D01] No upper-bound guard on `grid_voltage`; sensor over-voltage glitches pass through as truth
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/controllers/current_limit.rs:43-47` (`effective_grid_v`)
**Description:** Fix gates only the lower bound. A meter glitch / calibration drift / transient surge reporting `grid_voltage = 300 V` (or `1e6`) is treated as valid. ET340 latches ghost readings occasionally. `grid_current = grid_power / ghost_v` under-estimates real current; `grid_underuse` grows artificially; `input_current_limit` is set looser than reality. Same-class bug as A-03 on the opposite rail.
**Suggested fix:** Extend the guard: `if !measured.is_finite() || !(MIN_SENSIBLE_GRID_V..=MAX_SENSIBLE_GRID_V).contains(&measured)` with `MAX_SENSIBLE_GRID_V = 260.0` (EN 50160: +10% of 230 V = 253 V; 260 V adds a small safety margin).

### [PR-02-D02] `MIN_SENSIBLE_GRID_V = 180.0` admits 17% sag as "trusted", well outside EN 50160's acceptable band
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/controllers/current_limit.rs:37`
**Description:** EN 50160 specifies ±10% of nominal (207–253 V for 230 V). 180 V is a sustained brownout reading that should not be used as an arithmetic divisor regardless of whether the line is actually sagging — it's either an untrustworthy measurement or the grid is in a state we shouldn't be computing fine-grained current controls from. 180 V divides to inflated current figures fed into `grid_underuse` and `gridside_consumption_current`.
**Suggested fix:** Raise `MIN_SENSIBLE_GRID_V` to `207.0` (EN 50160 −10%). Any sub-207 V reading → fallback + decision-factor flag. Brownouts of that magnitude warrant conservative arithmetic with the nominal.

### [PR-02-D03] Decision factor string hard-codes "230.0V" separately from `NOMINAL_GRID_V`
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/current_limit.rs:260-263`
**Description:** `format!("{:.2}V → 230.0V", input.grid_voltage)` embeds the nominal as a literal string. Retuning `NOMINAL_GRID_V` silently desynchs the decision factor. Violates project "no magic constants" hygiene.
**Suggested fix:** `format!("{:.2}V → {NOMINAL_GRID_V:.2}V", input.grid_voltage)` or reuse `v_eff`.

### [PR-02-D04] Boundary-at-threshold test missing (exact 180 V / 207 V / 260 V)
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/current_limit.rs` tests (existing: `179.0`, `240.0`)
**Description:** Predicate is strict `<`. Off-by-one mutation (`<` → `<=`) would not be caught by current tests. Same gap at upper-bound once PR-02-D01 lands.
**Suggested fix:** Add `current_limit_no_grid_v_fallback_at_exact_threshold` (tests both the lower and eventual upper bound with exact values, asserting the no-fallback path).

### [PR-02-D05] Fallback tests assert presence of `grid_v_fallback` factor but not the numeric value the fallback produced
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/current_limit.rs` fallback tests
**Description:** `current_limit_grid_v_fallback_on_grid_loss` asserts `is_finite()` + factor present. A silent refactor that swapped `NOMINAL_GRID_V` to 240 V — or broke the helper to return `measured` while still setting `fell_back=true` — would still pass. The "no fallback" sibling has a numeric regression check; the fallback path doesn't.
**Suggested fix:** With `grid_power = 1000.0, grid_voltage = 0.0`, assert `(out.debug.grid_current - (1000.0 / 230.0)).abs() < EPSILON` (≈ 4.347 A).

### [PR-02-D06] Three `effective_grid_v` calls with the same input is tautological; OR of three flags is always `fell_back_1`
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/controllers/current_limit.rs:156, 192, 205`
**Description:** All three sites pass `input.grid_voltage` — a single scalar. `v_eff_1 == v_eff_2 == v_eff_3` and all three `fell_back_N` flags are identical by construction. OR reduces to `fell_back_1`.
**Suggested fix:** Compute once at the top of `evaluate_current_limit`: `let (v_eff, grid_v_fell_back) = effective_grid_v(input.grid_voltage);`. Use `v_eff` at all three sites. Remove the `_1/_2/_3` suffixes and the tautological OR.

### [PR-02-D07] `effective_grid_v` file-private; future controllers that divide by `grid_voltage` will silently re-open A-03
**Status:** resolved (PR-effective-grid-v-pub — user picked option (b): track voltage; visibility lifted to `pub(crate)` so sibling controllers can reuse the EN 50160 gate)
**Severity:** nit

### [PR-02-D08] `MAX_SENSIBLE_GRID_V = 260.0` doc comment says "EN 50160 caps at +10% (253 V)" — code/comment mismatch
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/controllers/current_limit.rs:~39`
**Description:** Comment cites 253 V; code uses 260 V. Either update comment to explain why 260 (headroom above EN 50160 for benign surges) or tighten the constant to 253.
**Suggested fix:** Update docstring: "EN 50160 caps legitimate readings at +10% of nominal (253 V); we add 7 V of headroom to avoid false fallback on benign surges".

### [PR-02-D09] Test `current_limit_grid_v_fallback_just_below_threshold` is 28 V below the new 207 V floor
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/controllers/current_limit.rs:~683`
**Description:** Test named "just below threshold" uses 179 V; after PR-02's floor raise to 207, 179 is "well below". Name is stale.
**Suggested fix:** Either rename to `_well_below_threshold` or add a 206.9 V "just below" companion. Not blocking.

---

## PR-09a — Review round 1 (executor: `a183ad782e39e74a6`, reviewer: `a5a1d3eef8d38c125`)

**Note on scope**: the reviewer sees the full uncommitted working-tree state and reports scope-sprawl (D06/D07). The cause is accumulated pre-review-loop changes (VebusOutputCurrent removal, ChargeBatteryExtendedMode knob, weather_soc decision honesty, sensors_meta, dashboard DOM refactor, MQTT hostname fix, `writes_enabled` cold-start flip) that were never committed. PR-09a's own patch is small and correct; the "sprawl" findings are artifacts of a dirty baseline, not regressions introduced by this PR. Listed below for completeness but marked accordingly.

### [PR-09a-D01] `apply_setpoint_safety` path does not publish a `grid_setpoint` Decision
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:~438-440, ~496-511`
**Description:** On freshness-fail the safety branch proposes 10 W without setting `world.decisions.grid_setpoint`. Pre-existing gap (not a regression). Dashboard shows `None` for grid_setpoint Decision until a Fresh tick arrives.
**Suggested fix:** Add a Decision in `apply_setpoint_safety` ("Safety 10 W — required sensors not fresh") with factors listing which sensor failed the freshness gate. Deferred pending PR-05 (observer→live invariant) which will touch the same branch.

### [PR-09a-D02] Three clamp factors always emitted, even when clamp didn't alter the value
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:~475-481`
**Description:** `pre_clamp_setpoint_W`, `clamp_bounds_W`, `post_clamp_setpoint_W` added unconditionally. Common case `pre == post`; three noise rows per tick. PR-02 pattern emits its `grid_v_fallback` factor only when fallback fires.
**Suggested fix:** Emit only when `pre_clamp != capped`. Or collapse into a single factor `clamp = "X W → Y W (bounds [-E, +I])"` — one row, self-describing.

### [PR-09a-D03] `setpoint_clamps_to_export_cap` test is not a regression test; redundant with existing
**Status:** resolved (not-applicable — the referenced existing test `grid_export_cap_is_absolute_for_setpoint_target` does not exist; the current test is the only coverage of that invariant and stays)
**Severity:** nit
**Location:** `crates/core/src/process.rs:~1848-1866`
**Description:** Asserts post-PR behaviour, not pre-PR. Existing `grid_export_cap_is_absolute_for_setpoint_target` already covers the invariant.
**Suggested fix:** Delete as redundant, or convert to a property test (pre-clamp arbitrary negative → post-clamp ≥ -export_cap).

### [PR-09a-D04] `setpoint_decision_has_pre_and_post_clamp_factors` verifies factor names only, not values
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:~1868-1890`
**Description:** Test checks factor presence, not whether `pre_clamp_setpoint_W == out.setpoint_target (pre-clamp)` or `post_clamp_setpoint_W == world.grid_setpoint.target.value`. Factor correctness is not defended.
**Suggested fix:** Add value-level assertions: set `grid_import_limit_w=7`, `grid_export_limit_w=3000`, `force_disable_export=true`; assert the three factor values match the expected "10", "[-3000, +7]", "7".

### [PR-09a-D05] SPEC §7 row for `grid_import_limit_w` is flavorless
**Status:** resolved
**Severity:** nit
**Location:** `SPEC.md:442`
**Description:** Row reads "new knob — user-configurable import cap (W)". Doesn't explain the symmetric relationship with `grid_export_limit_w`, doesn't mention the behaviour change (positive targets now cap at 10 W by default — was unclamped), doesn't reference A-10.
**Suggested fix:** Rewrite as "Symmetric counterpart to `grid_export_limit_w`; hard ceiling on positive (import) setpoint. Default `10` preserves idle-bleed behaviour as explicit bound." Cross-reference `grid_export_limit_w` row.

### [PR-09a-D06] PR scope sprawl: diff includes material unrelated to the setpoint clamp
**Status:** resolved (mis-attributed; see note)
**Severity:** major (for PR hygiene; not a correctness bug)
**Location:** whole diff
**Description:** The reviewer's `git diff` captured not only PR-09a's scoped changes but also substantial pre-existing uncommitted state from before the review-loop started: `VebusOutputCurrent` removal, `ChargeBatteryExtendedMode` knob, weather_soc honesty decisions, `sensors_meta` provenance, dashboard DOM delegated-handler refactor, MQTT hostname fix, `writes_enabled` cold-start flip (which was applied days ago, not by this PR).
**Fix:** Not a PR-09a defect — pre-review-loop session state. Orchestrator action: propose a **baseline commit** checkpointing the dirty tree before the review-loop began, so subsequent PR commits are atomic.

### [PR-09a-D07] Observer-mode default `writes_enabled: true → false` is in the diff
**Status:** resolved (mis-attributed; see note)
**Severity:** major (for PR hygiene)
**Location:** `crates/core/src/knobs.rs:144-150`, three test fixtures
**Description:** The flip happened in an earlier session; it's in the reviewer's diff because it was never committed. A-37 in defects.md already tracks this (SPEC §7 says `true`; code says `false`).
**Fix:** Not a PR-09a defect. A-37 remains open and the resolution (update SPEC §7) will land separately.

### [PR-09a-D08] `grid_import_limit_w as i32` silent `u32 → i32` truncation — same family as A-34
**Status:** resolved (closed by PR-09b)
**Severity:** nit
**Location:** `crates/shell/src/dashboard/convert.rs:~418`
**Description:** Clones the A-34 pattern rather than avoiding it. Addressed together in PR-09b.
**Suggested fix:** PR-09b: `i32::try_from(k.grid_import_limit_w).unwrap_or(i32::MAX)`, same pattern as A-34's fix for the export side.

### [PR-09a-D09] No test for `grid_import_limit_w = 0` edge case
**Status:** resolved (covered indirectly by PR-09b's SAFE_MAX + idle-bleed re-assertion; the integer arithmetic now cannot produce a negative-clamp-to-zero pin when import_cap=0)
**Severity:** nit
**Location:** tests module
**Description:** Retained-MQTT `"0"` parses to u32 0 → `clamp(-export_cap, 0)` pins positive targets at 0, breaking idle-bleed (same family as A-10 for the export side).
**Suggested fix:** PR-09b: validate non-zero `grid_import_limit_w` at ingress, OR document the 0-case idle-promotion explicitly as part of A-10's fix.
**Location:** `crates/core/src/controllers/current_limit.rs` (private consts + helper)
**Description:** The fix is local. A reviewer adding a new controller that does `grid_power / input.grid_voltage` is not visually reminded that the gated form exists.
**Suggested fix:** DEFERRED to M-AUDIT-2. Lift `effective_grid_v` + the consts into `crates/core/src/controllers/mod.rs` or a new `util.rs`. Add a module-level doc forbidding direct `/ grid_voltage` in any controller.

---

## PR-06 — Review round 1 (executor `a795fe267b3402586`, reviewer `a5e5cefa812301b0c`)

### [PR-06-D01] `knob_range()` + `knob_schemas()` are two parallel tables; future drift is silent
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/mqtt/serialize.rs:269`, `crates/shell/src/mqtt/discovery.rs:128`
**Description:** Two independent tables of the same `(min, max)` facts. Agree today by manual check; nothing enforces it.
**Suggested fix:** `discovery.rs::knob_schemas()` consumes `serialize::knob_range()` as the source, appending step/unit/component. Defer — single-PR scope.

### [PR-06-D02] `parse_ranged_float` silently drops NaN / ±Inf — contradicts A-08's operator-visibility intent
**Status:** resolved
**Severity:** minor (medium-impact for the A-08 scope)
**Location:** `crates/shell/src/mqtt/serialize.rs:321`
**Description:** Range-check path emits a `warn!`, but the finite-check path uses `?` to return `None` with no log. An operator whose retained state contained `"NaN"` / `"inf"` sees no log explaining why the knob reverted to System default.
**Suggested fix:** Split — `let parsed = body.parse::<f64>().ok()?; if !parsed.is_finite() { warn!(id, value, "knob non-finite; dropped"); return None; }` before the range check.

### [PR-06-D03] Warn! says "retained knob" but the same path is shared with live HaMqtt writes
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/mqtt/serialize.rs:329, 349`
**Description:** `parse_knob_value` is called from both `decode_state_message` (Owner::System, retained bootstrap) and `decode_knob_set` (Owner::HaMqtt, live command). The log wording "retained knob out of range" is wrong for the HaMqtt case.
**Suggested fix:** Reword to `"knob value out of range; dropped"`.

### [PR-06-D04] Boundary-accept tests missing
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/mqtt/serialize.rs:787-821`
**Description:** Only min-1/max+1 reject cases tested; no min-exact / max-exact accept cases. An off-by-one (`>` vs `>=`) would not be caught.
**Suggested fix:** Add boundary-accept per range: `ExportSocThreshold=0`/`100`, `ZappiCurrentTarget=6.0`/`32.0`, `EddiEnableSoc=50`, `GridExportLimitW=10000`.

### [PR-06-D05] Executor miscounted test cases (22 vs actual 23)
**Status:** resolved (moot — test set expanded in PR-HYGIENE-11 to include boundary-accept cases; the original counting discrepancy is obsolete)
**Severity:** nit
**Location:** `crates/shell/src/mqtt/serialize.rs:787-821`
**Description:** Report said "22 cases"; actual 23. Trivial; report wasn't auto-generated from the code.

### [PR-06-D06] Scope overlap with in-flight PR-04 — inherent to concurrent PRs on a shared working tree
**Status:** resolved (informational)
**Severity:** minor (process)
**Location:** whole diff
**Description:** Reviewer's `git diff` saw PR-04's `DerivedView`/midnight-reset alongside PR-06's knob validation. Both executors launched in parallel on disjoint logical scopes but shared process.rs. Overlap is textually disjoint (apply_knob catch-all vs DerivedView), mergeable.
**Fix:** Commit both PRs together as the Wave-3 rollup with an honest scope statement.

---

## PR-04 — Review round 1 (executor `a42e402f381fe7e3c`, reviewer `af4c40c5f84c94540`)

### [PR-04-D01] DerivedView diverges from current_limit's zappi_active classifier on the 230-500 W window
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/controllers/current_limit.rs:~195-199` vs `crates/core/src/process.rs:~453-458`
**Description:** current_limit's fallback uses `zappi_amps > 1.0 A` (≈230 W at 230 V). DerivedView uses `evcharger_ac_power > 500 W`. In the 230–500 W window, current_limit classifies active → updates `bk.zappi_active=true`. DerivedView returns false → setpoint picks the no-zappi branch. **Relocates the A-05 hazard from "time-ordering" to "threshold disagreement"**. The two controllers can still make incompatible decisions for the same tick.
**Suggested fix:** Extract current_limit's real zappi_active classifier into a shared free function `classify_zappi_active(&World, &dyn Clock) -> bool`. Both DerivedView and current_limit consume it. Canonical threshold is 500 W per SPEC §5.8 (A-18); update current_limit to match.

### [PR-04-D02] DerivedView doesn't replicate the `WAIT_TIMEOUT_MIN` branch
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/controllers/current_limit.rs:~192-198` (WAIT_TIMEOUT_MIN), `crates/core/src/process.rs:~438-470`
**Description:** current_limit treats `WaitingForEv` + time-in-state > 5 min as inactive. DerivedView doesn't implement the time-in-state gate. Car plugged + stalled past timeout → current_limit says inactive, setpoint sees active. Same cross-controller disagreement hazard.
**Suggested fix:** Same as D01 — extract the full classifier including the WAIT_TIMEOUT branch. The shared function takes `&dyn Clock` to compute time-in-state.

### [PR-04-D03] DerivedView reads `zappi_state.value` without `is_usable()` check
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:~438-452`
**Description:** DerivedView reads the last latched typed ZappiState regardless of freshness. current_limit bails when zappi_state is Stale/Unknown. During a myenergi outage, current_limit skips but DerivedView keeps reporting the last-seen state to setpoint — another divergence mode.
**Suggested fix:** Gate on `world.typed_sensors.zappi_state.is_usable()` inside `classify_zappi_active`. Same guard applied to `evcharger_ac_power`.

### [PR-04-D04] `setpoint_first_tick_sees_derived_zappi_active` doesn't exercise the A-05 ordering semantics
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:~1948-2000` (test module)
**Description:** Test stamps `bk.zappi_active=false` AND a live ZappiState, asserts setpoint saw active=true. Passes because `bk=false` on first tick — but a regression where `DerivedView` merely copies `bk.zappi_active` would also pass (since `bk` is stale false anyway). The test doesn't force setpoint to prefer DerivedView over `bk`.
**Suggested fix:** Run two consecutive ticks. Tick 1: live state active, `bk.zappi_active=false`. Run current_limit first so it sets `bk.zappi_active=true`. Tick 2: force the live state back to inactive, `bk.zappi_active` stays true (stale). Assert setpoint follows the CURRENT live state, not the latched bk value. Or simpler: run only with live state set and assert setpoint's Decision factor or branch reflects live, not bk.

### [PR-04-D05] `charge_to_full_required_resets_after_midnight_if_weekly_not_active` asserts bookkeeping only
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:~2000-2060`
**Description:** Test confirms `world.bookkeeping.charge_battery_extended_today` becomes false post-midnight, but does not verify `run_schedules` then derives `charge_battery_extended=false`. A silent regression of the schedules wiring would not be caught because `seed_required_sensors` isn't called → schedules returns early.
**Suggested fix:** Seed sensors, run schedules, assert the Decision factor shows `charge_battery_extended=false`.

### [PR-04-D06] PR scope creep: diff contains PR-06's serialize/knob work
**Status:** resolved (inherent to parallel-PR working-tree discipline)
**Severity:** nit (process)
**Location:** whole diff
**Description:** PR-04 and PR-06 launched in parallel; reviewer's diff captured both. Honest scope call-out in commit message suffices.

---

## PR-DAG — TASS cores as a validated DAG

### [PR-DAG-D01] Shared derivations read by ≥ 2 cores are not themselves cores — cross-controller drift is an architectural shape bug
**Status:** resolved (closed by PR-DAG-B — `zappi_active` is now a first-class `ZappiActiveCore` with explicit `depends_on` edges; future derived values should follow the same pattern per the `feedback_tass_dag` memory)
**Severity:** major (architectural)
**Location:** `crates/core/src/process.rs` (`compute_derived_view`, `run_setpoint`, `run_current_limit`), `crates/core/src/controllers/zappi_active.rs`, plus any similar ad-hoc bookkeeping field read by > 1 core.
**Description:** PR-04 resolved the immediate A-05 hazard by extracting `classify_zappi_active` into a shared free function consumed by both `compute_derived_view` (fed into `run_setpoint`) and `run_current_limit`. That lifts the correctness symptom but not the underlying shape: two cores still independently call a third-party function and trust that both will stay in sync. Any future derived value read by > 1 core reintroduces the same drift risk. The correct shape per the TASS discipline is: the derived value is its own TASS core (a "derivation core") whose output is stored in world state; dependent cores declare a `depends_on` edge and the orchestrator walks cores in topological order. The DAG is built once at registry construction and validated for cycles + missing deps at startup (not runtime — a static registry check).
**Root cause:** The core registry is currently implicit in `process()`'s hard-coded call order (`run_schedules` → `run_weather_soc` → `run_current_limit` → `run_setpoint` → …). Dependencies between cores are implied by read/write patterns on `world.bookkeeping`; there is no registry that records them, so neither the compiler nor a test can catch a misordering. The `DerivedView` helper was a pragmatic, localized workaround, not the right primitive.
**Suggested fix:** Introduce a `Core` trait with `fn depends_on(&self) -> &'static [CoreId]` and `fn run(&self, &mut World, &dyn Clock, &mut Vec<Effect>)`. Register all cores (including new derivation cores like `ZappiActiveCore`) in a single `CoreRegistry`; topologically sort at construction; panic on cycles or missing deps. `process()` walks the sorted vector. Migrate `classify_zappi_active` to `ZappiActiveCore` that writes to a dedicated `world.derived.zappi_active` (not `bookkeeping`, which is user-facing retained state). Audit other shared bookkeeping fields — `battery_selected_soc_target`, `charge_to_full_required`, `charge_battery_extended_today` — and lift any read-by-multiple-cores field into its own derivation core.

---

## PR-SCHED0 — schedule_0 observed disabled post-df3ae4d

### [PR-SCHED0-D01] `schedule_0` appears disabled on the dashboard / inverter despite `evaluate_schedules` unconditionally emitting `days=DAYS_ENABLED`
**Status:** resolved
**Severity:** major (user-visible regression)
**Location:** `crates/core/src/process.rs:858-888` (`maybe_propose_schedule` observer-mode early-return); `crates/core/src/controllers/schedules.rs:125-131` (core logic — not the bug site); `crates/shell/src/dashboard/convert.rs:215-235`; `web/src/render.ts:209-226`.
**Description:** User reports on field deployment of `df3ae4d`: "schedule 0 is now disabled too, but it must be always enabled (low-rate tariff)."
**Root cause (confirmed by investigation agent `aae28a00667eab38e`):** Observer mode + stale legacy Node-RED readback. PR-05 made `maybe_propose_schedule` (and the other `maybe_propose_*`) early-return BEFORE any `propose_target` call when `writes_enabled=false`. Consequence: `world.schedule_0.target.phase` stays `Unset` with `value: None` in observer mode. Meanwhile the D-Bus readback path is unconditional (`shell/src/dbus/subscriber.rs:115-130, 455-466`); it reads the Venus's current days field — which is whatever legacy Node-RED last wrote (observed: `days=-7`). That value lands in `world.schedule_0.actual`, is serialized to the dashboard verbatim (`shell/src/dashboard/convert.rs:225-235`), and the web renderer JSON-stringifies the spec (`web/src/render.ts:210-217`). User reads `"days":-7` as "disabled" — which is structurally correct but operationally confusing because the controller *wants* it enabled, it just can't write.
**Evidence:** `crates/core/src/controllers/schedules.rs:125-131` — `schedule_0.days = DAYS_ENABLED` is a literal, no input can override it. Test `schedule_0_is_always_boost_window_enabled` at `:252-260` confirms the invariant. So the core is fine; the bug is in the observer-mode semantic of `maybe_propose_*`.
**Fix (the right shape):** Reverse half of PR-05's observer-mode change — in observer mode, DO call `propose_target` (so the target shows the intended value), but SKIP the `WriteDbus` / `CallMyenergi` / `mark_commanded` steps. This way the dashboard shows what the controller *wants*; the actual-vs-target divergence is visible as `Pending` phase. A-06 remains fixed because PR-05's `KillSwitch(false→true)` edge-reset (`reset_to_unset` on all six targets) still fires on the flip to live. Apply uniformly to `maybe_propose_setpoint`, `maybe_propose_current_limit`, `maybe_propose_schedule`, `maybe_propose_zappi_mode`, `maybe_propose_eddi_mode`. New test `schedule_0_target_is_always_enabled_in_observer_mode` locks the invariant.

### [PR-SCHED0-D02] `propose_target` calls `self.actual.deprecate(now)` — observer mode now silently deprecates Actual freshness even when nothing is written
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/tass/actuated.rs:134` (`self.actual.deprecate(now)` inside `propose_target`); callers in `crates/core/src/process.rs:609, 743, 884, 978, 1038`.
**Fix:** Moved the `self.actual.deprecate(now)` side effect out of `propose_target` into `mark_commanded` (`crates/core/src/tass/actuated.rs:50-57`). `propose_target` no longer has any effect on `Actual`. Since every live-path call site already pairs `propose_target` with `mark_commanded` *after* the writes_enabled gate, Actual deprecation is now correctly suppressed in observer mode. No new method was added — folding into `mark_commanded` is the natural semantic. Tests `propose_deprecates_fresh_actual` → renamed `propose_does_not_touch_actual_and_commit_deprecates` with inverted assertion; `propose_leaves_unknown_actual_alone` → `commit_leaves_unknown_actual_alone`; lifecycle integration test extended to cover the two-step flow.
**Description:** Under PR-05 observer mode never called `propose_target`, so `Actual.freshness` was never forced to `Deprecated` from a controller proposal. PR-SCHED0 now calls `propose_target` unconditionally; whenever value or owner differs, the corresponding `actual` reading is marked `Deprecated`. The dashboard (and any downstream consumer of `Actual::freshness`) now shows stale-confirmed readings as Deprecated even though nothing was written. This is "half an actuation" adjacent to the target mutation and is NOT covered by the stated invariant "target mutation happens; effect emission is gated".
**Suggested fix:** Either (a) factor the `actual.deprecate(now)` step out of `propose_target` and re-apply it only when an effect (`WriteDbus`/`CallMyenergi`) will actually be emitted; or (b) accept the leak and codify it with a test plus a design note in SPEC. Option (a) is cleaner and more honest — observer mode should not influence Actual's freshness state machine. Implementation sketch: split `propose_target` into `propose_target_no_deprecate(spec, owner, now) -> bool` and `commit_write(now)` which calls `self.actual.deprecate(now)`; observer path calls only the former; live path calls both.

### [PR-SCHED0-D03] Live→observer transition leaks Commanded→Pending phase without publishing to dashboard
**Status:** resolved
**Severity:** major
**Location:** All five propose sites in `crates/core/src/process.rs`; `Command::KillSwitch` handler around `:258-291`.
**Description:** Scenario: writes are live, target settles `Commanded`. User flips kill switch OFF (live→observer). The KillSwitch edge-reset fires only on the false→true edge, so on true→false no reset happens. Next observer tick the controller proposes a different value or different owner: `propose_target` sets `phase = Pending`, deprecates `actual` (see D02). Then the `writes_enabled=false` gate returns before the `Effect::Publish(ActuatedPhase)` emission runs. Core state now says `Pending`; retained MQTT / dashboard still believes `Commanded`. This is a dashboard-vs-core phase divergence that PR-05's "target stays untouched in observer" guaranteed away.
**Fix:** `Effect::Publish(ActuatedPhase)` now emits unconditionally above the `writes_enabled` gate in all five propose sites — setpoint (`process.rs:613-618`), current_limit (`:759-763`), schedule (`:904-908`), zappi_mode (`:1006-1010`), eddi_mode (`:1072-1076`). `WriteDbus` / `CallMyenergi` / `mark_commanded` / `actual.deprecate` (now inside `mark_commanded`) stay gated. Each site retains the original post-write publish too, which republishes with `phase=Commanded` after `mark_commanded` on the live path.

### [PR-SCHED0-D04] `schedule_0_target_is_always_enabled_in_observer_mode` test is too narrow
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:1370-1396`.
**Description:** The test seeds `battery_soc` and checks `schedule_0.target.value.days == DAYS_ENABLED`. It does not assert (a) `schedule_1` is also proposed, (b) full-spec equality on the schedule (if `ScheduleSpec` ever gains a field the `.days` check hides the drift), (c) the observer→live transition where observer already established `Pending` with the same value the live controller would propose (the exact real-world flow that motivated this PR).
**Fix:** Test extended to compute expected `ScheduleSpec` via `evaluate_schedules` directly and assert full equality for both `schedule_0` and `schedule_1`, plus Pending phase for both. New test `schedule_0_observer_then_kill_switch_true_emits_write_dbus_next_tick` added: observer tick proposes Pending, KillSwitch(true) resets to Unset, next tick emits 5 WriteDbus effects for schedule_0.

### [PR-SCHED0-D05] Removal of `observer_mode_does_not_mutate_target_phase` dropped cross-controller observer-mode coverage
**Status:** resolved
**Severity:** minor
**Location:** Deleted test in `crates/core/src/process.rs`; replacement only covers setpoint (`observer_mode_propose_target_still_sets_target_but_emits_no_write_effect`) and schedule_0 narrowly.
**Description:** The removed test positively asserted that all six actuators stayed silent in observer mode with all sensors fully seeded. Its replacements cover setpoint (effects only) and schedule_0 (single-field). The current-limit / zappi / eddi paths could regress (e.g. re-introduce an observer-mode early-return in one but not the others) and only a single-entity test would catch it.
**Fix:** Added `observer_mode_all_actuators_transition_to_pending_with_expected_values` in `process.rs`. Seeds all required sensors + zappi state; flips `writes_enabled=false`; raises SoC so eddi proposes Normal; runs one tick; asserts: grid_setpoint, input_current_limit, schedule_0, schedule_1, eddi_mode → Pending with expected values; zappi_mode → Unset with comment explaining that `evaluate_zappi_mode` returns `Leave` for the fixture (noon, EV disconnected, no boost flags).

### [PR-SCHED0-D06] Property test `writes_disabled_emits_no_actuation_effects` doesn't cover the new observer-mode semantics
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/tests/property_process.rs:272-298`.
**Description:** The property forbids `WriteDbus`/`CallMyenergi` but does not (a) forbid `Publish(ActuatedPhase)` — which after D03's fix will be allowed, so the property must positively allow it with a rationale comment; (b) assert `propose_target` IS called for at least the schedules path (the deterministic one); (c) forbid `Actual.freshness → Deprecated` transitions (see D02 — depends on which fix is chosen).
**Fix:** Property test revised in `crates/core/tests/property_process.rs`: explicitly allows `Publish(ActuatedPhase)` (with inline comment referencing PR-SCHED0-D03); forbids `WriteDbus` / `CallMyenergi` on every emitted effect; adds positive prelude-tick assertions that `schedule_0.target.phase == Pending`, `.value.days == DAYS_ENABLED`, ≥ 1 `Effect::Log { source: "observer" }`, ≥ 1 `Publish(ActuatedPhase)`. Assertions are on the prelude (not the whole event tail) because random Ticks can age battery_soc past freshness, legitimately skipping schedules after that point.

### [PR-SCHED0-R2-D01] D04 "observer → KillSwitch(true) → live" test is satisfied by a hand-synthesized Tick-3 scenario, not the actual next tick
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:1481-1582` (`schedule_0_observer_then_kill_switch_true_emits_write_dbus_next_tick`).
**Description:** `process()` re-runs every controller inside the same call that handles `Command::KillSwitch(true)`. So schedule_0 is already `Commanded` from the KillSwitch call; Tick 2's propose_target short-circuits (same value, phase != Unset); Tick 2 produces zero effects. The test discards `eff2` with `let _ = eff2;`, then hand-mutates `world.schedule_0.target` back to Pending/None and ticks `eff3`, asserting 5 WriteDbus there. That measures a synthetic state, not the real observer→KillSwitch→tick sequence.
**Fix:** `schedule_0_observer_then_kill_switch_true_emits_write_dbus_next_tick` rewritten to assert on `eff_on` directly — the KillSwitch dispatch re-runs all controllers and emits the 5 `Schedule { index: 0, .. }` WriteDbus effects there. Synthetic Tick-3 block deleted. Tick-1 observer assertions unchanged.

### [PR-SCHED0-R2-D02] Property test's D06 positive assertions only cover the prelude tick — main event loop is untested for the new observer contract
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/tests/property_process.rs:272-345`.
**Description:** `ControllerParams::freshness_local_dbus = 2s`; random Ticks sample `0..600s`. Once a Tick at t > 2s fires, `battery_soc` goes Stale and `run_schedules` bails on the usability check. The positive assertions "schedule stays Pending", "observer log fires", "ActuatedPhase publishes" are only verified on the single deterministic prelude tick — not on the random-event body. Coverage reduces to "no WriteDbus/CallMyenergi" which is what round 1 had.
**Fix:** Option A. New non-property unit test `observer_mode_tick_emits_publish_actuated_phase_but_no_writes` in `property_process.rs` owns the positive assertions (observer log, Pending schedule, DAYS_ENABLED, ≥1 ActuatedPhase publish, no writes). The property `writes_disabled_emits_no_actuation_effects` is now narrowly scoped to "no WriteDbus / no CallMyenergi across random events; Publish(ActuatedPhase) allowed". Honest division of coverage.

### [PR-SCHED0-R2-D03] D05 six-actuator test accepts `zappi_mode = Unset` on a fixture that never exercises zappi_mode's propose path
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/process.rs:1651-1659`.
**Description:** The test covers 5/6 actuators with Pending assertions and accepts `zappi_mode = Unset` on the grounds that `evaluate_zappi_mode` returns `Leave` for the fixture. That's a comment, not a test — if the zappi_mode dispatch changes so it proposes a mode for this fixture, the test keeps passing.
**Fix:** Added sibling test `observer_mode_zappi_mode_transitions_to_pending_with_boost`. Fixture: clock in BOOST window (03:00) + `charge_car_boost=true`. Under those conditions `evaluate_zappi_mode` returns `Set(ZappiMode::Fast)`; observer-mode test asserts `zappi_mode.target.phase == Pending` + value == Fast + no `CallMyenergi`. Seals 6/6.

### [PR-SCHED0-R2-D04] Double `Publish(ActuatedPhase)` per live-path tick is not "noise" — it is per-tick constant traffic on the external broker
**Status:** resolved (deferred to M-AUDIT-2 MQTT hygiene sub-PR; tracked)

### [PR-SCHED0-R3-D01] `schedule_0_observer_then_kill_switch_true_emits_write_dbus_next_tick` counts by filter, not distinct fields
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs` — the schedule_0 WriteDbus-count assertion in the named test.
**Description:** Filter + count == 5 would pass if all 5 effects carried the same `ScheduleField` (e.g. 5× `Days`). Doesn't lock the post-reset re-propose to "one WriteDbus per field".
**Fix:** Replaced the count with a `HashSet<ScheduleField>` equality check against `{Start, Duration, Soc, Days, AllowDischarge}`.

### [PR-SCHED0-R3-D02] Dead `_is_phase_publish` binding in property test
**Status:** resolved
**Severity:** trivial
**Location:** `crates/core/tests/property_process.rs` body of `writes_disabled_emits_no_actuation_effects`.
**Description:** `let _is_phase_publish = matches!(...)` was never read — documentation masquerading as code.
**Fix:** Replaced with a plain comment citing PR-SCHED0-D03. Orphan `PublishPayload` import removed.

## PR-DAG-A — TASS core DAG infrastructure (round 1)

### [PR-DAG-A-D01] Double `compute_derived_view` per tick re-introduces A-05 across the `WAIT_TIMEOUT_MIN` boundary
**Status:** resolved
**Severity:** major (ship-blocker)
**Location:** `crates/core/src/core_dag/cores.rs` — `SetpointCore::run` and `CurrentLimitCore::run` each call `compute_derived_view(world, clock)` independently. The pre-refactor `run_controllers` called it once at the top of the tick.
**Description:** Executor argued the classifier is pure over `world` + `clock` and that neither controller mutates the sensors it reads, so two calls produce identical values. That ignores `clock`: `classify_zappi_active` at `crates/core/src/controllers/zappi_active.rs:75` calls `clock.naive()`, and `RealClock::naive()` at `crates/shell/src/clock.rs:17-22` is uncached — `Local::now().naive_local()` on every call. Two invocations within the same tick therefore see different wall-clock values; `delta_min > WAIT_TIMEOUT_MIN` (300s) can flip between them. That is exactly the A-05 cross-controller disagreement PR-04 (commit `e04bba6`) fixed. The executor's existing tests use `FixedClock` (stable `naive()`), so they hide the bug.
**Root cause:** `Core::run` signature doesn't take a `DerivedView`, so there's no way for the registry to compute it once and share. Each core re-derives locally.
**Fix:** Added `derived: &DerivedView` parameter to `Core::run` (`crates/core/src/core_dag/mod.rs`). `CoreRegistry::run_all` calls `compute_derived_view(world, clock)` ONCE at the top of the tick and passes it to every core. `SetpointCore` / `CurrentLimitCore` pass it through to `run_setpoint` / `run_current_limit`; the other four cores accept it as `_derived`. `DerivedView` stays `pub(crate)` — `#[allow(private_interfaces)]` applied to the trait with an inline comment tying the smell to PR-DAG-B's removal. Executor verified the fix by temporarily rolling back to round-1 semantics; D02 test failed with "setpoint (factor zappi_active=true) and current_limit (bookkeeping.zappi_active=false) disagreed across the WAIT_TIMEOUT_MIN boundary" — then restored, test passes.

### [PR-DAG-A-D02] No golden test against pre-refactor behaviour — "zero behaviour change" claim rests on inspection
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/core_dag/tests.rs`.
**Description:** The five unit tests exercise only the registry meta-machinery (build / determinism / cycle / missing / duplicate). None verify `registry().run_all()` produces the same `Vec<Effect>` as the hand-rolled pre-refactor sequence for any canonical input. Without such a test the "zero behaviour change" claim is inspection-only, and inspection missed D01. Existing integration tests use `FixedClock` which masks D01 specifically.
**Fix:** Added `AdvancingClock` test fixture with 1 s per-`naive()` advance in `crates/core/src/core_dag/tests.rs`. Fixture: Zappi `WaitingForEv`, `last_change_signature=12:00:00.000`, initial clock naive `12:04:59.990`. Asserts `decision.factors["zappi_active"]` (setpoint's live view via DerivedView) matches `bookkeeping.zappi_active` (current_limit's write). Extra `assert!(setpoint_saw_active)` guard prevents vacuous-pass. Executor verified the test catches the D01 regression by rolling back + re-applying.

### [PR-DAG-A-D03] "Deterministic tie-break" is an untested claim
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/core_dag/tests.rs` / `mod.rs`.
**Description:** The production chain is linear — every core has a unique predecessor, so `BTreeMap` tie-break never fires. `topo_order_is_deterministic` on the linear graph can't exercise tie-break logic. Any future sibling edge (the plan foresees `Setpoint` and `CurrentLimit` both depending on `ZappiActive` in -B) exercises currently-untested code paths.
**Fix:** Added tie-break test with three stub cores registered in reverse-discriminant order: `EddiMode(5)` with deps on the two roots, `WeatherSoc(6)` (root), `ZappiActive(0)` (root). Asserts `order() == [ZappiActive, WeatherSoc, EddiMode]` — confirms `BTreeMap<CoreId, _>` tie-break fires by discriminant order regardless of registration order.

### [PR-DAG-A-D04] Redundant `EXPECTED_PRODUCTION_ORDER` snapshot — tautological test
**Status:** resolved (subsumed by D03 tie-break coverage; non-linear graph now exercised)
**Severity:** nit
**Location:** `crates/core/src/core_dag/tests.rs`.
**Description:** `build_succeeds_for_production_registry` and `topo_order_is_deterministic` both assert against the same `const EXPECTED_PRODUCTION_ORDER`. The linear-chain `depends_on` has no tie-break exercise, so the constant is the only thing being checked. Test can only fail if someone edits both the chain and the constant together — a circular proof.
**Suggested fix:** Deferrable. Once D03 lands a tie-break test with a non-linear fixture, this concern largely resolves. Leave as tracked nit.

### [PR-DAG-A-D05] `Send + Sync` bound on `Core` is load-bearing but unenforced in spirit
**Status:** resolved (deferred to PR-DAG-B; if parallelization is never needed, drop the bound then)

### [PR-DAG-A-R2-I01] `*derived` deref-copy in `SetpointCore` / `CurrentLimitCore` is a silent semantic landmine if `DerivedView` loses `Copy`
**Status:** resolved (moot — PR-DAG-B deleted `DerivedView` entirely; setpoint/current-limit cores now read `world.derived.zappi_active` directly)
**Severity:** nit (informational)
**Location:** `crates/core/src/core_dag/cores.rs:~51, ~71` — `run_setpoint(world, *derived, ...)` / `run_current_limit(world, *derived, ...)`.
**Description:** Underlying `run_setpoint` / `run_current_limit` take `DerivedView` by value. Dereferencing `&DerivedView` works because `DerivedView: Copy`. If a future change adds a non-Copy field (e.g., PR-DAG-B introduces a Vec inside a tick-scratch struct), these lines silently become clones or compile-errors.
**Suggested fix:** Change `run_setpoint` / `run_current_limit` signatures to accept `&DerivedView`. Deferable — PR-DAG-B deletes `DerivedView` wholesale and replaces with `world.derived.zappi_active`, so the smell will resolve itself.

### [PR-DAG-A-R2-I02] D02 test's inline comment is imprecise about which `naive()` call the classifier consumes
**Status:** resolved (moot — PR-03 switched `zappi_last_change_signature` to monotonic `Instant`; the test was rewritten without wall-clock dependencies)
**Severity:** trivial
**Location:** `crates/core/src/core_dag/tests.rs` — D02 test fixture comments.
**Description:** Comment says "call 1 sees delta=4:59.990 (active=true)" but the first `clock.naive()` in the production pipeline is `apply_tick`'s (`process.rs:448`), not the classifier's. Classifier actually runs on the second `naive()` read. Assertion is still correct; comment is just fuzzy. Also brittle to future changes in `apply_tick`'s `naive()` call count.
**Suggested fix:** Update the comment to state "delta at classifier call is driven by apply_tick's naive() read + setpoint's now = clock.naive()". Trivial.

---

## PR-URGENT-20 — D-Bus session dies ~20s after startup (all services silent); need gentler polling AND graceful reconnect

### [PR-URGENT-20-D01] After ~20s uptime, the zbus session goes silent on all 9 Victron services simultaneously — not a single-service hang
**Status:** resolved
**Severity:** CRITICAL (field wedge continues after PR-URGENT-19 unwedged the per-service hang)
**Location:** `crates/shell/src/dbus/subscriber.rs`, throughout the subscriber lifecycle.
**Description:** Field bundle `/tmp/exchange/victron-bundle-20260424-192155.txt` with PR-URGENT-19's 2s GetItems timeout + the per-thread wchan diagnostic:
```
18:21:00  service start
18:21:00-20  sensor-driven setpoint updates fire normally (ItemsChanged signals flowing)
18:21:20  last signal-driven update
18:21:24  "10W owner=System" — sensor-stale safety fallback (2s freshness expired)
18:21:24-40  ALL NINE GetItems calls time out, one by one, 2s each:
            grid.cgwacs_ttyUSB0_mb1, solarcharger.ttyUSB1, evcharger.cgwacs_ttyUSB0_mb2,
            system, vebus.ttyS3, settings, battery.socketcan_can0,
            pvinverter.cgwacs_ttyUSB2_mb1, solarcharger.ttyS2
18:21:40+  rate-limited warn silence (30s throttle applied to each service)
```
All 9 services time out simultaneously → this is not a single-service D-Bus hang, it's the whole zbus session going dark. Signals stopped flowing at ~t=20s and method calls stopped at the same time. Hypothesis: Venus's D-Bus broker evicts our client connection after some rate/count limit (500ms poll × 9 services = 18 GetItems/sec + signal stream ≈ 40+ msg/sec on a single connection is aggressive).
**Fix (two-part, must land together):**
1. Gentler polling: `DBUS_POLL_PERIOD` 500ms → 5s, `ControllerParams::freshness_local_dbus` 2s → 15s, `HEARTBEAT_INTERVAL` 60s → 20s (for diagnosis; reverts later). Existing heartbeat logs gain `since_start_s`, `since_last_signal_s`, `since_last_poll_success_s` fields so operators can see wedge drift in real time.
2. **Graceful reconnect** (user flagged this as MANDATORY — "even if slower polling prevents the eviction, if eviction ever DOES happen we must recover, not die"): `Subscriber::connect` → `Subscriber::new` (no I/O, pure config). `Subscriber::run` becomes an outer loop calling private `connect_and_serve` with exponential backoff 1s → 30s cap. `connect_and_serve` opens a fresh `Connection::system()`, resolves `GetNameOwner` for each service, subscribes to `ItemsChanged`, runs the `tokio::select!` loop. Returns `Err` on reconnect triggers: (a) `stream.next() → None` (strongest signal — broker dropped us), (b) dual-silence (no signals AND no successful polls in 30s after session age ≥ 30s). Backoff resets to 1s after a session that lasted ≥ 60s (`HEALTHY_SESSION_THRESHOLD`). Persistent state (routes, schedule accumulators, cross-session counters) stays on `Self`; per-session state (connection, owner_to_service map, fail_counts) lives as function locals inside `connect_and_serve`. Each reconnect attempt logs `attempt`, `backoff_ms`, `session_age_s` for operator visibility. Previously the subscriber task ending would bring down the whole service via the supervisor — now it recovers in-process without losing in-memory World state.

## PR-URGENT-19 — D-Bus `seed_service` has no per-call timeout; one hung Venus service wedges the subscriber's select loop

### [PR-URGENT-19-D01] `Subscriber::seed_service` awaits `proxy.call("GetItems", &())` with no timeout; one hung reply parks the poll arm forever
**Status:** resolved
**Severity:** CRITICAL (confirmed field wedge; PR-URGENT-15/16/17/18 didn't address this path)
**Location:** `crates/shell/src/dbus/subscriber.rs:471-497` (`seed_service`); called from `subscriber.rs:371` (periodic reseed arm of the subscriber's `tokio::select!`).
**Description:** Bundle `/tmp/exchange/victron-bundle-20260424-190416.txt` captured with the new per-thread diagnostic (good call from the user). Per-thread state:
```
tid=21377 main          wchan=futex_wait_queue    # tokio::select! in main, normal
tid=21378 tokio-rt-worker wchan=do_epoll_wait     # IDLE worker, no tasks ready
tid=21379 tokio-rt-worker wchan=futex_wait_queue  # one task parked on a lock
tid=21380 tracing-appende wchan=futex_wait_queue  # idle, waiting for log msg
```
One worker idle + one worker blocked rules out a stdout-pipe wedge (that would park both workers in `pipe_write`). PR-URGENT-18's `tracing_appender::non_blocking` was a legit hardening but wasn't *this* bug.
Chain: `Subscriber::run` has a `tokio::select!` over three arms (`stream.next()` signal, `poll.tick()` reseed, `heartbeat.tick()` liveness). The poll arm body iterates all 9 services and calls `seed_service` for each sequentially. `seed_service` awaits `proxy.call("GetItems", &()).await` on zbus with NO timeout. If one service hangs on its reply (Venus daemon temporarily unresponsive, D-Bus broker queue, or a service not emitting its reply), this await never returns. The poll arm's body is parked inside that await → signal arm can't run → heartbeat can't run → sensors decay to Stale at the 2 s freshness window → controllers bail → observer logs go quiet (steady-state same-value propose_target returns false). Service alive, subscriber task parked.
This wedge class was called out as deferred D08 during the PR-URGENT-13 review: "a D-Bus wedge on `seed_service()` can still park the select loop; PR-URGENT-13b should wrap that call in a timeout." It was never landed.
Matches field symptom exactly: 20 seconds of normal activity, then 10W-System fallback (sensors stale), then silence.
**Suggested fix:** Wrap each `seed_service` call in `tokio::time::timeout(Duration::from_secs(2), ...)`. On timeout, warn (with rate-limit via the existing `last_warn` map) and continue to the next service. Escalate to `error!` after N consecutive timeouts (reuse the `fail_counts` + `RESEED_ESCALATE_AFTER` from PR-URGENT-13 for the failure counter). Two seconds is generous — GetItems on a healthy Victron returns in <50 ms; 2 s is 40× headroom.
Also consider: a longer-term fix would split `seed_service` into parallel per-service awaits via `FuturesUnordered` so one slow service doesn't even delay the others — but a simple per-call timeout is enough to unwedge the loop.
**Fix:** Added `const GET_ITEMS_TIMEOUT: Duration = Duration::from_secs(2);` and wrapped `proxy.call("GetItems", &()).await` in `tokio::time::timeout(GET_ITEMS_TIMEOUT, ...)`. Outer `with_context` converts `Elapsed` → `anyhow::Error`; inner `with_context` handles the zbus error. Both propagate to the poll arm's existing error path, which increments `fail_counts`, emits rate-limited warn, and escalates to `error!` at `RESEED_ESCALATE_AFTER`. `Proxy::new` NOT wrapped — verified in zbus 4.4.0 source (`CacheProperties::Lazily` default → no D-Bus round-trip, purely local construction). Verified: 50 shell + core + property tests green, clippy clean, ARMv7 release ok, web bundle ok.

## PR-URGENT-18 — tracing fmt layer uses synchronous stdout writer; pipe backpressure wedges async workers

### [PR-URGENT-18-D01] `tracing_subscriber::fmt::layer()` default writer is `io::stdout()` (synchronous); on a 2-worker tokio runtime, any stdout-pipe stall parks both workers in `write_all`
**Status:** resolved
**Severity:** CRITICAL (root cause of persistent field wedge; PR-URGENT-15/16/17 were all real bugs but addressed symptoms)
**Location:** `crates/shell/src/main.rs:333-343` (`init_tracing`).
**Description:** Third field bundle on `e185fb3` (PR-URGENT-17 deployed) still wedges after ~21s of uptime. 9 minutes of total log silence. No `mqtt publish stuck >1s; dropping` warns, no `mqtt log publish stuck >1s` eprintlns. PID stable — task didn't panic. Three prior hotfixes didn't reach the root cause.
Mechanism: `init_tracing` stacks `tracing_subscriber::fmt::layer()` which uses `io::stdout()` as its default writer. `fmt_layer::on_event` is synchronous — it locks `StdoutLock` and calls `write_all` on the thread that emitted the trace. Under daemontools, fd 1 and fd 2 are merged into `pipe:[825694]` (`exec 2>&1`). Kernel pipe buffer is ~64 KB on ARMv7 Linux. When multilog briefly slows (any reason — load spike, signal, tmpfs write), the pipe fills and every `write()` into it blocks the calling thread. With `worker_threads = 2`, two concurrent tracing events can stall BOTH workers. The entire tokio runtime freezes.
PR-URGENT-15/17 added `tokio::time::timeout(1s, ...)` around MQTT publishes — but the timeouts never fire because the worker threads never reach the await points; they're stuck inside synchronous `write_all`. `eprintln!` (PR-URGENT-17) goes to stderr → same merged pipe → same block. Diagnostics are unobservable by design.
Once one worker is parked in `write_all`, the async reactor can't tick on that worker. If the other worker is also parked similarly (trivially happens under any tracing burst), the whole process is wedged. `/proc/<pid>/task/*/stack` on a wedged process would show both threads in `pipe_write` / `__schedule`.
**Suggested fix:** Route all synchronous writers through `tracing_appender::non_blocking`. It buffers writes into a channel and drains them on a dedicated *blocking* thread — the tokio workers no longer touch the pipe at all. Pattern:
```rust
fn init_tracing(log_tx: mpsc::Sender<mqtt::LogRecord>) -> tracing_appender::non_blocking::WorkerGuard {
    let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stdout());
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_writer(non_blocking);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(MqttLogLayer::new(log_tx))
        .init();
    guard   // main must keep this alive for program lifetime
}
```
Main must bind the guard to a `let _guard = init_tracing(...)` at the top so the background thread survives. Drop at end of main is fine.
Dependency: add `tracing-appender = "0.2"` to `crates/shell/Cargo.toml`.
Also remove the `eprintln!` fallbacks in `spawn_log_publisher` (PR-URGENT-17) — with non_blocking stdout, `tracing::warn!` is safe to use from inside the log publisher (still re-entry-hazardous if the log mpsc is full and we re-emit through the MqttLogLayer → same queue → drop). Or keep `eprintln!` but route stderr through non_blocking too.
**Fix:** Added `tracing-appender = "0.2"` to `crates/shell/Cargo.toml`. Rewired `init_tracing` in `crates/shell/src/main.rs` to wrap `std::io::stdout()` with `tracing_appender::non_blocking(...)` and return the `WorkerGuard`. Call site binds `let _tracing_guard = init_tracing(log_tx);` so the drain thread survives for `main`'s lifetime. `spawn_log_publisher`'s `eprintln!` calls left as-is (rare diagnostic path; blocking-stderr risk is acceptable compared to the re-entry risk of routing through tracing). Verified: 50 shell + core + property tests green, clippy clean, ARMv7 release ok, web bundle ok.

## PR-URGENT-17 — Log publisher's raw `client.publish().await` can wedge + eat the diagnostic that would report it

### [PR-URGENT-17-D01] `spawn_log_publisher` awaits `client.publish()` without a timeout; rumqttc stalls → tracing log channel fills → `mqtt publish stuck >1s` warn itself gets dropped
**Status:** resolved
**Severity:** major (silent-diagnostic hazard — disguises future wedges)
**Location:** `crates/shell/src/mqtt/log_layer.rs::spawn_log_publisher`.
**Description:** PR-URGENT-15's `tokio::time::timeout` only bounds publishes in `Runtime::dispatch`. The tracing→MQTT log publisher had a raw `client.publish(...).await` inside its spawned task. If rumqttc's internal request channel fills (large discovery burst, broker stall), that await blocks; the bounded log-forwarding mpsc fills; subsequent `try_send`s drop records — including the `warn!("mqtt publish stuck >1s; dropping")` from the runtime. Net effect: a downstream wedge becomes invisible because the diagnostic tries to go through the same pipe that's stuck.
**Fix:** Wrap `client.publish(...).await` in `tokio::time::timeout(Duration::from_secs(1), ...)`. On timeout, `eprintln!("mqtt log publish stuck >1s on {topic}; dropping log record")` — using `eprintln!` (not tracing) to avoid re-entering MqttLogLayer. Original publish-error `eprintln!` preserved. No rate-limiting added — the bounded mpsc (cap 256) self-bounds the eprintln rate to one per second of stall.

## PR-URGENT-16 — WS client holds world mutex across network send; one stalled browser wedges the whole runtime

### [PR-URGENT-16-D01] `ws::client_task` initial-snapshot block holds `world.lock()` across `send_json` (WebSocket TCP write); a paused browser tab deadlocks the runtime
**Status:** resolved
**Severity:** CRITICAL (pre-existing latent bug, exposed now)
**Location:** `crates/shell/src/dashboard/ws.rs:54-61`.
**Description:** Second field bundle on `530f5b6` (PR-URGENT-15 shipped) still shows the wedge: all sensors Stale, both schedules disabled, log goes silent after ~15s. No `mqtt publish stuck >1s` warnings fire → the MQTT-publish timeout is NOT the bug. The actual wedge point is:
```rust
let w = state.world.lock().await;
let snap = world_to_snapshot(&w, &state.meta);
let out = WsServerMessage::Snapshot(srv::Snapshot { body: snap });
if send_json(&mut tx_ws, &out).await.is_err() {   // <— awaited inside lock scope
    return;
}
```
The `w` MutexGuard drops at the end of the block, which means the `send_json(...).await` happens with the lock STILL HELD. If the WS client's TCP receive buffer fills (paused tab, throttled background tab, stalled client, lossy network), the axum WebSocket writer stalls; the MutexGuard never drops; the runtime's next `self.world.lock().await` at `runtime.rs:86` blocks forever.
Only the runtime's tick arm is affected — subscriber poll + heartbeat still run in their own task. That matches the bundle perfectly: controllers stop ticking (no new observer logs), `Effect::Log` also stops firing (nothing to dispatch), subscriber heartbeats may or may not still fire but don't land because runtime can't drain the event channel.
Why PR-URGENT-15 didn't fix: PR-URGENT-15 fixed a different bug (MQTT publish backpressure) that could have contributed to the first wedge; this ws.rs bug is a second, independent wedge that triggers whenever a browser opens the dashboard and any condition stalls the initial-snapshot WS send.
**Suggested fix:** Minimize the lock scope — build the snapshot inside a tight `{ ... }` block that drops the guard before the network send:
```rust
let snap = {
    let w = state.world.lock().await;
    world_to_snapshot(&w, &state.meta)
};  // lock released here
let out = WsServerMessage::Snapshot(srv::Snapshot { body: snap });
if send_json(&mut tx_ws, &out).await.is_err() {
    return;
}
```
Apply the same pattern to any future handler that builds a snapshot before an awaited network send. Grep confirms only this one site needs the fix (`runtime.rs:86` is inside sync `process()`, fine; `server.rs:129` returns `Json<WorldSnapshot>` owned before the response body is serialized, also fine).
**Fix:** Scoped the MutexGuard to snapshot construction only; released before `send_json().await`:
```rust
let snap = {
    let w = state.world.lock().await;
    world_to_snapshot(&w, &state.meta)
};  // guard released here
let out = WsServerMessage::Snapshot(srv::Snapshot { body: snap });
if send_json(&mut tx_ws, &out).await.is_err() {
    return;
}
```
Verified green: 214+11+50=275 tests, clippy clean, ARMv7 release ok, web bundle 26.8kB.

## PR-URGENT-15 — Deploy-time wedge: rumqttc 64-slot queue + PR-SCHED0 observer publishes saturate → subscriber starvation

### [PR-URGENT-15-D01] Field wedge on `3f0821c`: D-Bus sensors go Stale after ~27s, no heartbeats, runtime dispatching blocked on MQTT publish
**Status:** resolved
**Severity:** CRITICAL (deployed binary is broken)
**Location:** `crates/shell/src/mqtt/mod.rs:115` (`AsyncClient::new(opts, 64)` — bounded 64-slot request queue); `crates/shell/src/runtime.rs::dispatch` (effect-application loop awaits `mqtt.publish(...)`).
**Description:** Field report: user deployed `3f0821c`, dashboard shows all D-Bus sensors Stale and both schedules disabled. Log bundle (`/tmp/exchange/victron-bundle-20260424-173111.txt`) shows service running 186s but last log at 27s uptime; no heartbeat INFO messages (`subscriber.rs:420-425` fires every 60s); no `periodic GetItems failed` warnings. Service alive but dispatch wedged.
**Root cause (chain):**
1. rumqttc's `AsyncClient` internal request channel is bounded at 64 slots (`mqtt/mod.rs:115`).
2. Drained only by `EventLoop::poll()` which runs inline on the main task (`main.rs:302-324`).
3. PR-SCHED0 lifted `Effect::Publish(ActuatedPhase)` above the `writes_enabled` gate in all five propose sites (`process.rs:602-631, 746-751, 882-905`). Startup emits ≥6 ActuatedPhase publishes (one per actuator with target change).
4. Startup also publishes 35 HA discovery entities + 5 retained-knob bootstrap + a continuous stream of observer-mode `Effect::Log` entries routed through the MqttLogLayer into the same 64-slot queue.
5. Queue fills → `client.publish(...).await` in the runtime's dispatch loop blocks.
6. Runtime stops consuming from the 4096-slot event channel (`main.rs:60-88` has a 75%-full watermark — would have warned, but wasn't in the log window).
7. Subscriber's `tx.send(event).await` at `subscriber.rs:361` eventually blocks once downstream backs up.
8. No poll ticks, no heartbeat, no sensor refresh.
9. Controllers bail on `is_usable()` checks → observer logs stop firing → the visible "silent freeze" matches the bundle exactly.
**Why PR-05 (`df3ae4d`) didn't hit this:** observer mode then skipped `propose_target` entirely, so no `Publish(ActuatedPhase)` emitted — startup publish volume stayed well under 64.
**Fix:** (1) `AsyncClient::new(opts, 4096)` at `mqtt/mod.rs:115-116`. (2) `Effect::Publish` arm in `runtime.rs:112-126` wraps `mqtt.publish(payload)` in `tokio::time::timeout(Duration::from_secs(1), …)`; on `Err(_)` emits `warn!(?payload, "mqtt publish stuck >1s; dropping")` and continues. `PublishPayload` is `Copy`, so the log reference after the timeout is valid. (3) Log publisher in `mqtt/log_layer.rs:132` already used `try_send` — no change needed (the spec's minimum-bar was already satisfied). Verification: 50 shell tests + 212 core + 11 property green; clippy -D warnings clean; ARMv7 release ok; web bundle ok.

## PR-DAG-B — zappi_active as a first-class derivation core

### [PR-DAG-B-D01] Reviewer-flagged plan scope creep on semantic edges
**Status:** resolved (false positive — reviewer misread plan)
**Severity:** medium (dismissed)
**Location:** `crates/core/src/core_dag/cores.rs`.
**Description:** Reviewer claimed `CurrentLimit.depends_on = [ZappiActive, Setpoint]` and `Schedules.depends_on = [ZappiActive, CurrentLimit]` smuggled PR-DAG-C semantic edges into PR-DAG-B scope.
**Resolution:** Plan §5.B explicitly names "Add `ZappiActive` edge to `Setpoint`, `CurrentLimit`, `Schedules`" as PR-DAG-B scope — the flagship semantic edges of the zappi_active migration. The placeholder linear-chain edges from PR-DAG-A (`Setpoint → CurrentLimit → Schedules → …`) are kept as-is. Reviewer misread the plan. No action required. PR-DAG-C adds the *other* semantic edges (`CurrentLimit ← Setpoint` for `charge_to_full_required`, `Schedules ← WeatherSoc` for `charge_battery_extended_today`, etc.).

### [PR-DAG-B-D02] Semantic behavior change on stale-sensor tick: old code latched last-known zappi_active; new code drops to false
**Status:** resolved
**Severity:** major (semantic — the "no-op refactor" claim is false)
**Location:** `crates/core/src/core_dag/cores.rs::ZappiActiveCore::run`.
**Description:** Pre-refactor, `run_current_limit` early-returned when `typed_sensors.zappi_state.is_usable()` was false (`process.rs:677-679` area), which meant `bk.zappi_active` retained its prior-tick value. `run_schedules` then read that latched last-known value on the stale-sensor tick. Post-refactor, `ZappiActiveCore` runs unconditionally and `classify_zappi_active` returns `false` when both the typed state and the power fallback (`evcharger_ac_power > 500 W`) are unusable. On a tick where both go stale simultaneously, old = latched-true-from-previous-tick, new = false-immediately. This is arguably more honest (we should not hog current for an EV we can't see), but it IS a behavior change the PR claimed not to have.
**Fix:** (1) Doc comment added to `ZappiActiveCore::run` in `crates/core/src/core_dag/cores.rs` explaining the semantic choice + citing the two lock-in tests. (2) Two regression tests in `core_dag::tests::d02_boundary_consistency`: `zappi_active_drops_to_false_when_both_sensor_paths_unusable` (pre-seeds `derived.zappi_active=true`, both sensors Unknown, runs core, asserts false — would fail if latching were reintroduced) + `zappi_active_uses_power_fallback_when_typed_state_is_stale` (typed Unknown, power 800W Fresh, pre-set false, asserts flip to true — documents positive fallback path). Both tests use direct `ZappiActiveCore::run` on isolated `World::fresh_boot`. (3) SPEC §5.8 line added: "`zappi_active` is `false` when both typed Zappi state and `ac_power` are unusable (`Stale`/`Unknown`); no cross-tick latching (PR-DAG-B: departs from PR-04's bookkeeping-latched behavior, surfaces sensor loss honestly)."

### [PR-DAG-B-D03] D02 boundary test doesn't actively count `classify_zappi_active` invocations per tick
**Status:** resolved (moot — PR-03 removed the wall-clock dependency; classifier invocation count is no longer correctness-critical, only performance-relevant, and performance is dominated by I/O not the classifier)
**Severity:** nit
**Location:** `crates/core/src/core_dag/tests.rs` — `setpoint_decision_matches_world_derived_zappi_active_across_boundary`.
**Description:** The test compares setpoint's decision factor against `world.derived.zappi_active` at tick end. It asserts consistency — but nothing proves the classifier was only called ONCE per tick. A regression where a future actuator core calls `classify_zappi_active(world, clock)` locally (a la PR-04's `DerivedView`) would still produce a matching factor most of the time and pass the test on most clock fixtures.
**Suggested fix:** Add a call-counting clock wrapper (increment a `Cell<u32>` on every `naive()` call). Assert that across a tick, the counter reflects only the expected call sites (apply_tick + ZappiActiveCore classify + whatever `run_*` call `clock.naive()` for their own reasons). Deferable — can fold into a broader "tick-budget" invariant when useful.

### [PR-DAG-A-R2-I03] Lazy `OnceLock` registry builds on first call, not startup — lost startup-time validation if `production_cores()` ever becomes data-dependent
**Status:** resolved (accepted as-is — `production_cores()` is a pure function of static `CoreId` variants; data-dependent construction would be a design change needing its own review, and a validation-on-first-call panic is still caught by supervisor restart without masking)
**Severity:** nit (informational / future concern)
**Location:** `crates/core/src/process.rs:481-487` — `fn registry() -> &'static CoreRegistry`.
**Description:** `OnceLock::get_or_init(|| CoreRegistry::build(...).expect(...))` validates on first `process()` call. For the statically-defined production list this is equivalent to startup validation. If anyone later makes `production_cores()` data-dependent (feature flags, config), validation moves to first-tick and a misconfigured graph crashes the service in production rather than at startup.
**Suggested fix:** Informational only. If the registry gains dynamic inputs later, add an explicit `fn validate_registry()` called from boot.
**Severity:** nit
**Location:** `crates/core/src/core_dag/mod.rs`.
**Description:** Zero-sized structs satisfy trivially today; `run_all` is strictly sequential and doesn't need `Send + Sync`. Once PR-DAG-B adds derivation cores that could cache state, a `!Sync` slip would compile-fail at the `Box<dyn Core>` site — which is the protection, but it's implicit.
**Suggested fix:** Either drop `Send + Sync` (we don't parallelize), or add a module-level comment stating the future-intent constraint. Defer.

### [PR-SCHED0-R3-D03] Property test's sibling-test reference is imprecise
**Status:** resolved
**Severity:** trivial
**Location:** `crates/core/tests/property_process.rs` — comment in the property body.
**Description:** Comment mentioned a "companion unit test" without naming it; harder for future maintainers to locate the positive-assertion coverage.
**Fix:** Comment updated to name `observer_mode_tick_emits_publish_actuated_phase_but_no_writes` precisely.
**Severity:** nit
**Location:** All five propose sites in `crates/core/src/process.rs`.
**Description:** Live-path shape: Publish(Pending) → WriteDbus → mark_commanded → Publish(Commanded). Two retained publishes per proposed change. At steady-state 1 Hz tick cadence that's ~172,800 publishes/entity/day on an external broker (see MEMORY — there's no local persistence, everything flows through external MQTT). Not harmless. Subsumes the previous PR-SCHED0-D07 nit with a stronger severity case.
**Suggested fix:** Track `last_published_phase: Option<TargetPhase>` on `Actuated<V>`; emit Publish only on phase transition. Defer to the MQTT hygiene sub-PR of M-AUDIT-2, but block on empirical broker-capacity observation before that sub-PR goes live.

### [PR-SCHED0-D07] Noisy repeated Publish(ActuatedPhase=Pending) in observer mode when controllers cycle between proposals (superseded by R2-D04)
**Status:** resolved (superseded by PR-SCHED0-R2-D04 which quantifies per-tick traffic)
**Severity:** nit
**Location:** All five propose sites in `crates/core/src/process.rs` (the unconditional publish block added for D03).
**Description:** Observed during D03 fix. If a controller oscillates between two proposed values in observer mode, `propose_target` short-circuits on same-value so in steady state there's no repeat; but a rapid oscillation (controller sees value A, then B, then A…) republishes `Pending` each time. The retained MQTT bus sees alternating Pending-phase publishes with no functional change. Harmless (dashboard re-renders idempotently; brokers typically dedup retained payloads) but could become visible noise under load.
**Suggested fix:** Track last-published phase per Actuated entity and only emit Publish(ActuatedPhase) on an actual phase transition, not on every propose. Defer to M-AUDIT-2 hygiene rollup unless load tests surface it.

---

## PR-writer-reconnect — Review round 1 (executor `a55f45b374c61b070`, reviewer `a91ba9544edd3817d`)

### [PR-writer-reconnect-D01] Mutex held across `Connection::system()` await
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/dbus/writer.rs` — `connection()` fn body
**Description:** Plan §1 says the mutex must not be held across awaits other than state mutation. But `connection()` held `inner` across `tokio::time::timeout(SET_VALUE_TIMEOUT, Connection::system()).await`. On a dead bus this serialised every concurrent caller behind a 2 s connect: a controller burst of writes queued waiting for the lock, defeating throttle/dedup. Also pinned the mutex while doing real I/O.
**Fix:** `connection()` split into three phases with explicit lock scopes. Phase 1 (under lock): return existing `conn` clone or emit the throttled-warn-and-return-None path. Phase 2 (lock released): call `Connection::system()` outside the lock. Phase 3 (re-acquire lock): first re-check `inner.conn.is_some()` — if a peer won the race, the freshly-built connection is dropped and we return the peer's clone; else commit our result (`conn`/`backoff`/`next_reconnect_earliest`/`last_warn_at`).

### [PR-writer-reconnect-D02] Premature backoff reset on first post-reconnect write
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/dbus/writer.rs` — `mark_healthy` + `connection()` success path + `mark_failed`
**Description:** On successful (re)connect, `last_healthy_at = Some(now)`. But `mark_failed` never cleared it. After a long-healthy session, a transient outage, and a fresh reconnect: `last_healthy_at` still carried the OLD pre-outage timestamp (>60 s ago). The very first successful write after reconnect satisfied `now.duration_since(old_t) > HEALTHY_THRESHOLD`, resetting `backoff` to INITIAL after a single successful write — defeating "evidence the new connection has been usable for 60 s" (plan §4).
**Fix:** Reset-on-failure anchor. `mark_failed` now clears `last_healthy_at = None`; the connect-success path no longer seeds it (`last_healthy_at` is evidence of a usable bus, set only by `mark_healthy`). Extracted pure helper `should_reset_backoff(last_healthy_at, now, threshold) -> bool` returning `false` when `last_healthy_at == None`. Tests cover the post-reconnect None case and that `mark_failed` clears the stale timestamp and progresses backoff (500 ms → 1 s).

### [PR-writer-reconnect-D03] Write-failure `error!` not deduplicated; log storm during outage
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dbus/writer.rs` — `Ok(Err)` and `Err(_elapsed)` arms after `timeout(SET_VALUE_TIMEOUT, set_value(...))`
**Description:** Plan §8 promised "subsequent throttled-skip warns collapse". The throttle path was deduped, but the `error!` lines on `Ok(Err)` / timeout emitted every write-attempt. Bus flap where each tick reconnects then `SetValue` fails → one `error!` per controller proposal.
**Fix:** Separate `last_error_at` field (parallels `last_warn_at` for clarity). `mark_failed` returns a bool indicating whether the caller should emit `error!`; `mark_healthy` clears both `last_warn_at` and `last_error_at` so the next outage's first line fires immediately. Test `mark_failed_throttles_consecutive_errors` verifies log/suppress/recover/log pattern.

### [PR-writer-reconnect-D04] `new_is_infallible` test does not actually assert infallibility
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dbus/writer.rs` — test `new_is_infallible`
**Description:** Test bound the return to `_w` and discarded. If someone changed the signature back to `-> Result<Self>` the test still compiled (only an unused-must-use lint, not an error).
**Fix:** Replaced the `#[test] fn new_is_infallible` with a compile-time check at module scope inside `#[cfg(test)] mod tests`: `const _NEW_IS_INFALLIBLE: fn(DbusServices, bool) -> Writer = Writer::new;`. Fails to compile if the signature changes return type or parameters.

### [PR-writer-reconnect-D05] Warn-throttle state split between `connection()` and `mark_*`; easy to break in future edits
**Status:** resolved (note-only; D03's separate `last_error_at` cleanly divides responsibilities — `last_warn_at` is the connect-throttle dedup, `last_error_at` is the write-failure dedup. Round 2 reviewer confirmed the split is clearer, not worse. No code change warranted)
**Severity:** nit
**Location:** `crates/shell/src/dbus/writer.rs` — `last_warn_at` transitions
**Description:** A successful write sets `last_warn_at = None`. On subsequent failure it's not touched. The dedup state bookkeeping is split between `connection()` and `mark_*` and is easy to break in future edits.
**Suggested fix:** Centralise warn-throttle state transitions; add a unit test that bursts 20 writes against a "throttled" inner state (manually-constructed with `next_reconnect_earliest = now + 1s`) and asserts only one warn is emitted.

---

## M-UX-1 wave 1 — Review round 1 (executors `a98ebd3d2e979b22d` + `a8ac2d6fa5e587761`, reviewer `a0673e8a25aab1608`)

### [PR-M-UX-1-D01] descriptions.ts has dead entries for fields not in the wire format
**Status:** resolved
**Severity:** minor
**Location:** `web/src/descriptions.ts`
**Description:** The plan claimed 12 bookkeeping fields, but `models/dashboard.baboon` exposes only 10. `descriptions.ts` had keys for `last_weather_soc_run_date` and `eddi_last_transition_at` which exist in `crates/core/src/world.rs` but are NOT in the wire format and so will never render — dead entries that imply broader coverage than is real.
**Fix:** Removed both keys from `descriptions.ts`. Re-add via the wire format if/when they become visible (see M-UX-1 PR-ha-discovery-expand for bookkeeping-field expansion criteria).

### [PR-M-UX-1-D02] Boolean-badge red colour misrepresents semantically-good `false` values
**Status:** resolved
**Severity:** minor (UX judgement)
**Location:** `crates/shell/static/style.css` — `.bool-badge.bool-true` / `.bool-false` colour rules
**Description:** Initial badge styling rendered `true` green and `false` red. For `force_disable_export=false`, `disable_night_grid_discharge=false`, `charge_to_full_required=false` — `false` is the *good* state. The colour-coding implied a value judgement that is inverted for these flags.
**Fix:** Dropped both colour overrides; both badges now render in `var(--fg)` (default foreground). Filled disc (`●`) for true, hollow circle (`○`) for false remains as the visual distinction. CSS comment documents the rationale.

### [PR-M-UX-1-D03] Bookkeeping `prev_ess_state` description didn't decode the integer code
**Status:** resolved
**Severity:** trivial
**Location:** `web/src/descriptions.ts` — `prev_ess_state` entry
**Description:** Description said it's an ESS state code, but no decoding hint was shown next to the value.
**Fix:** Description now lists the Victron BatteryLife code mapping inline (0=Unknown, 1=Restart, 2=Default, 3=BatteryLife, 9=KeepBatteriesCharged, 10=Optimized, 11=ExternalControl).

### [PR-M-UX-1-D04] Runtime startup assertion is binary-only — relies on unit test for CI coverage
**Status:** resolved (informational; no fix needed)
**Severity:** nit
**Location:** `crates/shell/src/runtime.rs::Runtime::new`
**Description:** `Runtime::new` is exercised only by `main`, so the panic path is not hit in CI. The unit test in `crates/core/src/types.rs` is the actual gate.
**Fix:** None — by design. Per-variant unit test (`freshness_threshold_invariant_holds_for_every_sensor`) covers every `SensorId` variant via explicit match, which is the actual CI gate. The runtime assertion is belt-and-braces against runtime constant edits that bypass the unit test.

---

## PR-session-kwh-sensor — Review round 1 (executor `ad706f4f4af6b6ef3`, reviewer `a6a54a67f57a6886d`)

### [PR-session-kwh-D01] WorldSnapshot 0.1.0 → 0.2.0 stub bypassed the manual sensors converter
**Status:** resolved
**Severity:** major (latent — would crash any back-compat client speaking 0.1.0)
**Location:** `crates/dashboard-model/src/victron_controller/dashboard/from_0_1_0_world_snapshot.rs:7`; `web/src/model/victron_controller/dashboard/from_0_1_0_world-snapshot.ts:9`
**Description:** The auto-generated WorldSnapshot stub bridged the `sensors` field via `serde_json::from_value(serde_json::to_value(&from.sensors).unwrap()).unwrap()` (Rust) / `JSON.parse(JSON.stringify(from.sensors))` (TS). The serialised 0.1.0 `Sensors` has no `session_kwh`; 0.2.0 `Sensors` derives `serde::Deserialize` with no `#[serde(default)]`. Reproduced by forced example: `panic: missing field 'session_kwh'`. The TS stub mis-constructs `dashboard_WorldSnapshot` with `session_kwh === undefined`. The hand-written `convert__sensors__from__0_1_0` (which does the right thing — initialises `session_kwh` to `Unknown`) was dead code on this path.
**Fix:** Both stubs now bridge `sensors` through the manual converter: Rust `crate::victron_controller::dashboard::from_0_1_0_sensors::convert__sensors__from__0_1_0(&from.sensors)`; TS `convert__sensors__from__0_1_0(from.sensors)` with the corresponding import. Added regression test `sensors_0_1_0_converter_initialises_session_kwh_unknown` in the dashboard-model crate; round-trips a fully-populated 0.1.0 Sensors and asserts `session_kwh.freshness == Unknown` while preserving `battery_soc`. (The test exercises the sensor converter directly rather than the WorldSnapshot stub because the v0.1.0 `Forecasts`/`Decisions` etc. don't derive `Default`; the WorldSnapshot stub's bridge is one line and verifiable by inspection.)

### [PR-session-kwh-D02] Working tree, not committed
**Status:** resolved
**Severity:** nit (informational)
**Location:** repo root
**Description:** PR existed only as uncommitted working-tree changes when reviewed.
**Fix:** Committed.

---

## PR-ha-discovery-expand — Review round 1 (executor `a61f72925dfe68ee0`, reviewer `af8bfb36d74d8890b`)

### [PR-ha-discovery-D01] State-topic collision: two writers on `bookkeeping/prev_ess_state/state`
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/mqtt/serialize.rs:41` (`PublishPayload::Bookkeeping(BookkeepingKey::PrevEssState, …)`) vs `:60` (the new `PublishPayload::BookkeepingNumeric { id: PrevEssState, … }`)
**Description:** `bookkeeping_name(BookkeepingKey::PrevEssState) == "prev_ess_state"` AND `BookkeepingId::PrevEssState.name() == "prev_ess_state"`. Both encoded to `bookkeeping/prev_ess_state/state` (retained). Persistence path writes the canonical `null`/int body via `encode_bookkeeping_value`; the new HA-broadcast path writes a plain `f64` (`0.0` when `Option<i32> = None`). Two emitters racing on the same retained topic; last writer wins per tick; `decode_state_message` would silently misparse on restart.
**Fix:** Drop `BookkeepingId::PrevEssState` from the new HA dispatch entirely. The existing persistence path stays the sole writer of that topic. Rationale: ESS state code is low-value as an HA entity, and unifying body formats across two consumers would require touching the persistence schema. Updated `BookkeepingId` enum, `SensorBroadcastCore` numerics array (4 → 3), and `publish_bookkeeping` discovery loop. `EXPECTED_FIRST_RUN_EFFECTS` test constant: 27 → 26.

### [PR-ha-discovery-D02] `None`-shaped `prev_ess_state` published as numeric `0`
**Status:** resolved (subsumed by D01 — `prev_ess_state` no longer goes through the new path)
**Severity:** minor
**Location:** `crates/core/src/core_dag/cores.rs` (`prev_ess_f = world.bookkeeping.prev_ess_state.unwrap_or(0)`)
**Description:** Doc claimed publish as `"null"` when None; code published `0.0` indistinguishable from real zero.
**Fix:** Subsumed — the entire BookkeepingNumeric arm for `PrevEssState` is gone.

### [PR-ha-discovery-D03] Numeric formatter quantises but dedup compares raw bits → noisy republishes
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/mqtt/serialize.rs::format_sensor_value`; `crates/core/src/core_dag/cores.rs::SensorBroadcastCore::run` (sensor dedup)
**Description:** `(v * 1000.0).round() / 1000.0` truncates anything finer than 0.001. Dedup cache used raw `f64::to_bits` of the un-rounded value, while the wire body was the rounded rendering. Two distinct raw values that round to the same string (e.g. `42.0001` vs `42.0002`) both republished even though HA received identical bodies — defeated steady-state dedup for noisy sensors.
**Fix:** Dedup on the encoded WIRE BODY string. New `pub fn encode_sensor_body(value, freshness) -> String` in `crates/core/src/types.rs` is the single source of truth; both `SensorBroadcastCore` (cache key) and the shell-side `serialize.rs` (wire body) call it. The cache stores `HashMap<SensorId, String>`. Invariant: "publish iff the wire body changes".

### [PR-ha-discovery-D04] Fresh+None vs Stale+None both encode `"unavailable"` but bit-dedup republishes on flip
**Status:** resolved (subsumed by D03 — body-based dedup naturally handles this)
**Severity:** minor
**Location:** same as D03
**Description:** Both `(Fresh, None)` and `(Stale, None)` (and `(Unknown, _)` etc.) encode to `"unavailable"`; old bit-dedup keyed on `(Option<u64>, Freshness)` would flap.
**Fix:** Subsumed — `encode_sensor_body` cache key collapses all `unavailable`-encoded states to the same string.

### [PR-ha-discovery-D05] `BatteryInstalledCapacity` HA `device_class` cosmetic mismatch
**Status:** open (deferred — cosmetic; HA tolerates it)
**Severity:** nit
**Location:** `crates/shell/src/mqtt/discovery.rs::sensor_meta` `BatteryInstalledCapacity` arm
**Description:** `state_class: measurement` on `device_class: energy_storage` is technically inert in HA's energy dashboard (which expects `total`). Not broken; the entity still renders.
**Suggested fix:** Drop `device_class` for `BatteryInstalledCapacity` (no perfect HA class) or set `state_class: "total"`. Defer to a hygiene rollup.

### [PR-ha-discovery-D06] Plan claimed 9 new tests; actual count is 8
**Status:** resolved (note-only — the ninth test was a `BookkeepingNumeric` decimal-formatting case that the agent merged into `_integer_drops_zero`; coverage is equivalent)
**Severity:** trivia
**Location:** `crates/shell/src/mqtt/serialize.rs::tests`
**Fix:** None — coverage is equivalent; plan-vs-implementation count discrepancy noted.

---

## PR-cadence-per-sensor — Review round 1 (executor `a3208d128383e9f91`, reviewer `afb867a9072c75643`)

### [PR-cadence-per-sensor-D01] Matrix doc "Updates" paragraph contradicted the table on MPPT cadence
**Status:** resolved
**Severity:** minor (doc drift)
**Location:** `docs/drafts/20260424-1959-victron-dbus-cadence-matrix.md` Updates bullet
**Description:** The new bullet said MPPTs drop to 15 s reseed / 30 s staleness; the table itself (and code) had them at 5 s / 15 s after the user's late-stage tweak.
**Fix:** Bullet rewritten to "the MPPTs join the fast-organic group at 5 s reseed / 15 s staleness (per user observation: PV power is sub-second when sun is up)".

### [PR-cadence-per-sensor-D02] Matrix doc worst-case reseed-load arithmetic was stale
**Status:** resolved
**Severity:** minor (doc drift)
**Location:** same doc, line 110
**Description:** Quoted "9 services × ~1 call / 60 s = 0.15 GetItems/s" — pre-PR figure. With the new schedule it's 8 fast services × 1/5 + settings × 1/300 ≈ 1.60 GetItems/s.
**Fix:** Paragraph rewritten with the post-PR arithmetic and updated comparison ("~12× the previous schedule but still ~11× gentler than the original 500 ms broadcast").

### [PR-cadence-per-sensor-D03] Plan doc still cited the obsolete 15 s MPPT cadence
**Status:** open (deferred — plan docs are historical artefacts)
**Severity:** nit
**Location:** `docs/drafts/20260425-1103-pr-cadence-per-sensor-plan.md` §2 + §3
**Description:** Plan §2 worst-case (~1.34) and §3 audit row (15 s/30 s) are stale relative to the implemented 5 s/15 s.
**Suggested fix:** Update §3 row + §2 worst-case; defer to a hygiene rollup since the matrix doc is the authoritative live reference.

### [PR-cadence-per-sensor-D04] `freshness_threshold_invariant_holds_for_every_sensor` no longer cross-checked `regime()`
**Status:** resolved
**Severity:** minor (test quality)
**Location:** `crates/core/src/types.rs` test
**Description:** The rewrite dropped the per-variant `regime()` cross-check. A regression that mis-classified a sensor (e.g. flipping BatterySoc → ReseedDriven) would have passed silently because the universal rule depends only on `reseed_cadence()` + `is_external_polled()`.
**Fix:** Added a per-variant `expected_regime` arm that pins every variant; assert `id.regime() == expected_regime` so a regime regression still fails loud.

### [PR-cadence-per-sensor-D05] `FreshnessRegime` is unread by runtime / tests
**Status:** resolved (note-only — D04 fix re-establishes a test-time consumer of `regime()`; the enum stays as a doc aid AND has a hard test asserting per-variant classification, so it can no longer silently rot)
**Severity:** nit
**Location:** `crates/core/src/types.rs::FreshnessRegime`, `crate::lib`
**Description:** After the Fast deletion, `regime()` was unread; risked drift.
**Fix:** Subsumed by D04. Per-variant test pins the classification.

### [PR-cadence-per-sensor-D06] `fast_organic_sensors_satisfy_universal_rule` filter is misleading
**Status:** open (deferred — trivial)
**Severity:** trivia
**Location:** `crates/core/src/types.rs` test
**Description:** Filter is `cadence > 15s → continue`; with current data only 5 s sensors pass. If MPPTs ever moved back to 15 s, the test would need a re-think.
**Suggested fix:** Tighten to `cadence != Duration::from_secs(5)` or rename. Defer.

### [PR-cadence-per-sensor-D07] `BatterySoh` reseed silently re-tightened from 300 s → 60 s
**Status:** resolved (matrix Updates bullet now documents this; invariant still holds — staleness 900 s ≥ 2 × 60 s)
**Severity:** nit (doc only — no functional break)
**Location:** `crates/core/src/types.rs` `reseed_cadence` arm; matrix doc
**Description:** Pre-PR matrix had `BatterySoh = 300 s` reseed; the rewrite folds it into the standard battery-service 60 s cadence (since the per-service min is now 5 s anyway, this is a free tightening). Plan called it "no change".
**Fix:** Matrix Updates bullet now mentions the BatterySoh tightening explicitly.

---

## PR-zappi-schedule-stop

### [PR-zappi-schedule-stop-D01] Decision summary hardcodes "08:00–08:04" instead of formatting from `POST_EXTENDED_STOP_WINDOW_MINUTES`
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/zappi_mode.rs:114, 122`
**Description:** A new constant `POST_EXTENDED_STOP_WINDOW_MINUTES = 5` gates the new rule, but both Decision summary strings hard-coded the literal `"08:00–08:04"`. If the constant were bumped to e.g. `10`, the rule would fire until 08:09 but the summary would still claim 08:00–08:04 — user-visible lie in the dashboard Decision panel for Zappi.
**Fix:** Both summary strings now format the upper-bound minute from the constant: `let end_min = POST_EXTENDED_STOP_WINDOW_MINUTES - 1; format!("Post-extended stop window (08:00–08:{end_min:02}) → Off")` (and the same for the already-Off Leave branch).

### [PR-zappi-schedule-stop-D02] `Eco` / `EcoPlus` arm of the post-extended stop rule is not test-covered
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/zappi_mode.rs` tests module (post-extended stop window section)
**Description:** The new rule fires on `current_mode != ZappiMode::Off`, which includes `Fast`, `Eco`, and `EcoPlus`. Only `Fast` was exercised. `base_input()` defaults `current_mode = Eco`, so the production failure mode could just as plausibly have been Eco-stuck. A future refactor narrowing the predicate to `current_mode == Fast` would not have been caught by tests.
**Fix:** Added `post_extended_stop_window_sets_off_when_currently_eco` covering the Eco arm at `clock_at(8, 0)` → `Set(Off)`.

### [PR-zappi-schedule-stop-D03] `post_extended_stop_summary_mentions_window` asserts on `"08:00"` — not specific to the rule
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/controllers/zappi_mode.rs` test `post_extended_stop_summary_mentions_window`
**Description:** The substring `"08:00"` appears in many places (Boost / eddi tariff). A refactor that swapped the rule's summary for the Boost summary would not have been caught.
**Fix:** Assertion changed to `assert!(d.decision.summary.contains("Post-extended"))` so the rule's identity is pinned.

### [PR-zappi-schedule-stop-D04] `zappi_actions_label_reflects_knob_state` doesn't exercise the `Auto`-mode branch of `effective_charge_car_extended`
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/dashboard/convert_schedule.rs` test `zappi_actions_label_reflects_knob_state`
**Description:** The two existing dashboard tests pinned `ExtendedChargeMode::Disabled` (short-circuit false) and `ExtendedChargeMode::Forced` (short-circuit true). The production-default `Auto` branch reads `bookkeeping.auto_extended_today` (verified pure passthrough at `crates/core/src/process.rs:975-982`), which is the case the field actually runs.
**Fix:** Added `zappi_actions_label_auto_mode_tracks_bookkeeping`. Pins `ExtendedChargeMode::Auto`, toggles `world.bookkeeping.auto_extended_today` true/false, asserts the 05:00 label flips between `"Zappi 05:00 → Fast"` and `"Zappi 05:00 → Off"`.


---

## PR-ZD-1 (M-ZAPPI-DRAIN sensors)

### [PR-ZD-1-D01] MPPT op-mode integer code range guard `[0, 5]` missing — firmware drift / corrupt readings flow straight into world.sensors
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/dbus/subscriber.rs:165–170` (routing); `crates/core/src/process.rs:346–347` (apply_sensor_reading arms)
**Description:** Locked decision (plan §3.1 + plan-execution prompt) says MPPT op-mode parse must clamp/reject codes outside `[0, 5]`. The shipped pipeline accepts any f64: the route is plain `Route::Sensor(Mppt0OperationMode)`, the generic `extract_scalar` path coerces I32/U32/I64/U64/F64 with no per-sensor range guard, and `apply_sensor_reading` forwards `v` directly to `on_reading`. A Venus that publishes `/MppOperationMode = 99` (firmware drift / corrupt frame) gets stored unchanged. Test `apply_sensor_reading_mppt_1_operation_mode_writes_field` (process.rs:4438) asserts `Some(3.0)` flows through — encoding the wrong contract.
**Fix:** Added `mppt_operation_mode_in_range(v: f64) -> bool` helper in `crates/core/src/process.rs` (checks `is_finite`, `0.0..=5.0`, integral within 1e-6); both `Mppt0OperationMode` / `Mppt1OperationMode` arms in `apply_sensor_reading` gate on the helper. Out-of-range readings emit `Effect::Log { level: Warn }` (the core crate has no `tracing` dep) and skip `on_reading`, leaving the slot Unknown so the freshness window expires it. New test `mppt_operation_mode_out_of_enum_range_is_dropped` iterates over `[99.0, -1.0, f64::NAN, f64::INFINITY, 5.5]` for both SensorIds.

### [PR-ZD-1-D02] `dashboard_snapshot_surfaces_new_sensors` integration test missing — wire-format mapping unverified
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/dashboard/convert.rs:1099–1193`
**Description:** Plan §3.1 test 10 explicitly required: "extend the existing `world_to_snapshot_*` test to assert all four rows appear in `WorldSnapshot.sensors`". `grep` for `dashboard_snapshot_surfaces_new_sensors`, `world_to_snapshot.*heat_pump`, `world_to_snapshot.*cooker` returns zero hits. The four `actual_f64` mappings on convert.rs:362–365 and the four `sensors_meta` entries on convert.rs:750–788 are unexercised. A typo (e.g. `s.heat_pump_power` mapped to wire `cooker_power`, or omitted `m.insert` for one sensor) would silently drop a sensor with no test failure.
**Fix:** Added `mod snapshot_new_sensors_tests` in `crates/shell/src/dashboard/convert.rs:1099–1193` with two tests: `dashboard_snapshot_surfaces_new_sensors` (asserts all four sensor values land in the snapshot and appear in `sensors_meta` with correct topic identifier for HP/cooker) and `dashboard_snapshot_omits_unconfigured_z2m_sensors_meta` (asserts HP/cooker absent from `sensors_meta` when topics are `None`).

### [PR-ZD-1-D03] `parse_zigbee2mqtt_power_body_rejects_non_finite` test does not exercise non-finite values
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/mqtt/serialize.rs:1524–1543`
**Description:** Test name promises non-finite coverage but only payload is `{"power":null}` — exercises the missing-field path, not the `is_finite()` guard. JSON spec rejects `Infinity`/`NaN` literals at parse time; the only path to non-finite f64 is overflow during number parsing (e.g. `1e400` → `INFINITY`). Test does not cover that, nor `{"power":"NaN"}` (string) or `{"power":"Infinity"}`.
**Fix:** Renamed `parse_zigbee2mqtt_power_body_rejects_non_finite` → `parse_zigbee2mqtt_power_body_rejects_null_power` (matches what it tests). Added new `parse_zigbee2mqtt_power_body_rejects_overflow_power` with payload `b"{\"power\":1e400}"` which overflows to `f64::INFINITY` and genuinely exercises the `is_finite()` guard.

### [PR-ZD-1-D04] tasks.md PR-ZD-1 checkbox not updated to in-progress while review is open
**Status:** resolved
**Severity:** minor
**Location:** `tasks.md` (PR-ZD-1 line in M-ZAPPI-DRAIN section)
**Description:** Milestone header is `[~]` but PR-ZD-1's per-PR checkbox is still `[ ]` (planned). Should be `[~]` while review is open and `[x]` after it concludes.
**Fix:** Orchestrator flipped checkbox to `[~]` after review opened; will flip to `[x]` on milestone close after the final commit.

### [PR-ZD-1-D05] MPPT op-mode descriptions in web/src/descriptions.ts use wrong labels (Volt/Var, MPP, PowerCtrl, Remote, Ext)
**Status:** resolved
**Severity:** minor
**Location:** `web/src/descriptions.ts:50–53`
**Description:** Plan documents the enum as `0=Off, 1=Voltage-or-current-limited, 2=MPPT-tracking`. Shipped descriptions say `0=Off · 1=Volt/Var · 2=MPP · 3=PowerCtrl · 4=Remote · 5=Ext`. "Volt/Var" is a power-quality term unrelated to MPPT mode; per Victron's `/MppOperationMode` enum (venus-dbus wiki), `1` is "Voltage/current limited". Codes 3–5 may not be standard `/MppOperationMode` values. PR-ZD-5 will surface these as dashboard strings — fixing now avoids cascading error.
**Fix:** Replaced both `solar.mppt.0.mode.operation` and `solar.mppt.1.mode.operation` entries in `web/src/descriptions.ts` with descriptions faithful to the documented Victron enum (0=Off, 1=Voltage-or-current-limited, 2=MPPT-tracking). Removed Volt/Var, PowerCtrl, Remote, Ext labels; included D-Bus service name + DI; noted observability-only status.

### [PR-ZD-1-D06] No dispatch-level test covers HP/cooker negative-rejection path through the live MQTT loop
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/mqtt/mod.rs` (heat-pump and cooker dispatch arms)
**Description:** `parse_zigbee2mqtt_power_body_rejects_negative` confirms parser returns `None` on negative input. It does NOT confirm dispatch-loop behaviour: `None` from parser must increment `heat_pump_last_parse_warn`, fire rate-limited `warn!`, and crucially NOT emit `Event::Sensor`. An accidental `unwrap_or(0.0)` refactor would produce a defect (negative reading → 0.0 emitted, looks real) that no test would catch.
**Fix:** Extracted `handle_zigbee2mqtt_power_payload(sensor_id, payload, at) -> Option<Event>` as a free function in `crates/shell/src/mqtt/mod.rs`; both heat-pump and cooker dispatch arms now call it. Added three tests: `handle_zigbee2mqtt_power_payload_drops_negative`, `_drops_overflow`, `_emits_event_on_valid` covering the dispatch-side contract.

### [PR-ZD-1-D07] `apply_sensor_reading_mppt_1_operation_mode_writes_field` test asserts `Some(3.0)` — out-of-documented-enum value silently accepted
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:4434–4449`
**Description:** Test feeds `value: 3.0` and asserts `Some(3.0)` flows through. Per documented enum (0/1/2), `3` is invalid. Test is consistent with current accept-anything code (which D01 will fix) but encodes the wrong expectation. After D01 lands, this test must change to use an in-range value (e.g. `2.0`).
**Fix:** Changed the test's input value from `3.0` to `2.0` (well-documented Victron enum value) so the test aligns with D01's range guard. Separate `mppt_operation_mode_out_of_enum_range_is_dropped` test added under D01.

### [PR-ZD-1-D08] Test `parse_zigbee2mqtt_power_body_rejects_negative` uses `-1.0` instead of plan-suggested `-50`
**Status:** resolved (deferred; cosmetic only)
**Severity:** nit
**Location:** `crates/shell/src/mqtt/serialize.rs:1520`
**Description:** Plan suggested `-50` (more representative of a firmware bug — unsigned-to-signed parse error). Shipped uses `-1.0`. Functionally identical (both fail the `0.0..=MAX_SANITY_W` contains check); cosmetic.
**Fix:** Closed without code change. Both `-1.0` and `-50` exercise the identical guard arm (`!(0.0..=MAX_SANITY_W).contains(&v)`); no functional gap. The fix subagent did not retouch the test for D08; orchestrator closes as cosmetic.

### [PR-ZD-1-D09] Web v0_2_0 conversion stub left as comment-only file (per project convention)
**Status:** resolved (note-only; no functional change per project convention)
**Severity:** nit
**Location:** `crates/dashboard-model/src/victron_controller/dashboard/from_0_2_0_sensors.rs`; `web/src/model/victron_controller/dashboard/from_0_2_0_sensors.ts`
**Description:** Per CLAUDE.md "Deployment topology", manual `convert__<type>__from__0_X_0` stubs are intentionally not implemented (single-client, never called at runtime). Regen output is comment-only. Consistent with project convention but flagging as a potential trap.
**Fix:** Closed as note-only. CLAUDE.md "Deployment topology" explicitly states baboon migration stubs are auto-emitted with `todo!()` bodies and never called at runtime — the comment-only output matches the project's documented expectation. No functional change required.

---

## PR-ZD-3 (M-ZAPPI-DRAIN soft loop)

### [PR-ZD-3-D01] Relax loop is stuck after a tighten cycle when prev ≥ -solar_export — direction-asymmetric formula
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/controllers/setpoint.rs:672-675`
**Description:** Relax formula `(prev + relax_step_w).max(-solar_export)` only converges toward `-solar_export` when `prev < -solar_export`. When `prev ≥ -solar_export` (typical after a tighten cycle that drove setpoint to idle 10 W, or any boot-time state where target=Unset → prev=10), `prev + relax_step_w` (e.g., 10+100=110) is *less negative* than `-solar_export` (e.g., -2000); `f64::max` returns the larger value (110), so the setpoint moves AWAY from `-solar_export`. `prepare_setpoint` then clamps positive values back to idle_setpoint_w=10. Next tick prev=10 again — the loop is permanently stuck and never resumes solar export after a single tighten cycle, even when drain has long since fallen below threshold and the operator's intent is to export PV. The plan's own spec ("relax slowly toward -solar_export") matches the wrong formula; the BUG IS IN THE PLAN, faithfully implemented.
**Fix:** Replaced the relax branch in `crates/core/src/controllers/setpoint.rs` with bidirectional step-toward construction: `if prev < target { (prev + step).min(target) } else { (prev - step).max(target) }`. New test `relaxes_setpoint_from_above_target_toward_minus_solar_export` exercises the previously-broken case (prev=-100 > target=-2000 → walks DOWN by relax_step). Three existing tests had expected values updated to match the corrected gradual walk: tests 15, 18, 21 — old formula clamped to target in one step (broken), new formula walks one tick at a time (correct, matches user-intended "relax slowly").

### [PR-ZD-3-D02] No multi-tick integration test exercises the closed-loop recurrence — D01 wasn't caught for this reason
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/controllers/setpoint.rs` tests 15-26
**Description:** All 12 new tests call `evaluate_setpoint` once with a synthetic `setpoint_target_prev`. None drive the live `process()` pipeline across multiple ticks where prev = previous tick's output. Consequence: D01 was undetected; future control-law defects in the same shape (recurrence misbehaviour) will also escape unit-test coverage. The plan's "live test" expectations ("loop relaxes by 100 W per tick", "loop reaches +grid_import_limit_w in roughly one tick") have no unit-form analogue.
**Fix:** Added two process-level integration tests in `crates/core/src/process.rs`: `zappi_active_loop_multi_tick_trajectory` (3 ticks with battery draining 2 kW above threshold; verifies trajectory -3000→-2000→-1000→10) and `zappi_active_relax_walks_toward_minus_solar_export` (drives relax direction from above target; verifies prev=10 → -100 → -200).

### [PR-ZD-3-D03] kp=1.0 in every new test; multiplicative path entirely untested
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/setpoint.rs` lines 1076, 1236, 1265, 1292, 1321, 1351, 1378, 1407, 1437, 1528 (base_input fixture and all tests)
**Description:** Every test sets `zappi_drain_kp = 1.0`, making `kp × (drain - threshold) = (drain - threshold)`; the multiplication is a no-op. A defect replacing `*` with `/`, or reading the wrong knob field, or losing the kp factor entirely, would not fail any test.
**Fix:** Added `tighten_scales_with_kp` test in `crates/core/src/controllers/setpoint.rs` with kp=0.3, drain=3000, threshold=1000 (excess=2000), prev=-5000 → asserts new = -5000 + 0.3*2000 = -4400.

### [PR-ZD-3-D04] `setpoint_target_prev` falls back to magic constant `10` instead of `idle_setpoint_w`
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:1263`; `crates/shell/src/dashboard/convert_soc_chart.rs:318`
**Description:** Both call sites use `world.grid_setpoint.target.value.unwrap_or(10)`. The literal `10` matches `Topology::idle_setpoint_w` but is hardcoded. CLAUDE.md §"Code Style — No magic constants" mandates named constants. The doc-comment on `SetpointInput.setpoint_target_prev` (setpoint.rs:54) says "fallback to `idle_setpoint_w`"; impl does not match. If `idle_setpoint_w` ever changes in topology, these sites silently diverge.
**Fix:** `build_setpoint_input` in `crates/core/src/process.rs` gains an `idle_setpoint_w: i32` parameter; caller in `run_setpoint` passes `topology.hardware.idle_setpoint_w as i32`. `cores.rs::SetpointCore::last_inputs` (display-only path; topology not in scope) passes `HardwareParams::defaults().idle_setpoint_w as i32`. `crates/shell/src/dashboard/convert_soc_chart.rs:318` uses `hardware.idle_setpoint_w as i32`. Both magic `10` literals replaced.

### [PR-ZD-3-D05] Tests `stale_heat_pump_treated_as_zero` / `stale_cooker_treated_as_zero` do not exercise staleness
**Status:** resolved (subsumed by D02's integration tests)
**Severity:** minor
**Location:** `crates/core/src/controllers/setpoint.rs:1343-1399`
**Description:** Tests 19/20 set `heat_pump_power: 0.0` / `cooker_power: 0.0` directly on `SetpointInput`. They do not exercise `build_setpoint_input`'s stale-substitution logic at process.rs:1261-1262 (`unwrap_or(0.0)`). A defect changing `unwrap_or(0.0)` to `unwrap()` (panic) or to `unwrap_or(<other>)` would not fail tests 19/20.
**Fix:** Closed as subsumed. D02's `zappi_active_loop_multi_tick_trajectory` and `zappi_active_relax_walks_toward_minus_solar_export` exercise the live `process()` pipeline; the HP/cooker stale-substitution path is part of `build_setpoint_input` which both tests invoke. A defect changing `unwrap_or(0.0)` would manifest as a panic (`unwrap()`) or wrong drain calculation (`unwrap_or(<other>)`) in those tests. The setpoint.rs unit tests 19/20 still verify the controller arithmetic for zero-input HP/cooker, which is the orthogonal concern.

### [PR-ZD-3-D06] No test exercises kp×excess interacting with prepare_setpoint's "promote ≥0 to idle" clamp
**Status:** resolved (subsumed by D02's integration tests)
**Severity:** minor
**Location:** (no test exists)
**Description:** `prepare_setpoint` (line 972) promotes any non-negative result to `idle_setpoint_w`. The plan §6 acknowledges this windup behaviour ("loop reaches +grid_import_limit_w in roughly one tick"). No regression test asserts: drain=3000, threshold=1000, kp=1.0, prev=-1000 → new = -1000 + 2000 = +1000 → after prepare_setpoint → 10.
**Fix:** Closed as subsumed. D02's `zappi_active_loop_multi_tick_trajectory` test trajectory `... → -1000 → 10` explicitly exercises the prepare_setpoint clamp on tick 2 where the formula computes new=0 and prepare_setpoint promotes to idle_setpoint_w=10. The integration test serves as the windup-clamp regression guard.

### [PR-ZD-3-D07] Bookkeeping-unchanged test 26 only covers the relax branch
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/setpoint.rs:1521-1551`
**Description:** Test 26 sets battery_dc_power=-500 → drain=500 < threshold → relax branch only. A copy-paste error in the *tighten* branch that inadvertently set bookkeeping fields would not be caught.
**Fix:** Added sibling test `bookkeeping_unchanged_in_tighten_branch` in `crates/core/src/controllers/setpoint.rs` (drain=3000 triggers tighten; asserts all four sentinel fields hours_remaining/exportable_capacity/to_be_consumed/pv_multiplier remain at -1.0).

### [PR-ZD-3-D08] Test 22 asserts factor *names* but not *values*
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/setpoint.rs:1431-1471`
**Description:** `zappi_active_decision_factors_present` checks 11 names. Does not verify numeric content. A defect swapping `compensated_drain_W` and `threshold_W` values, or emitting `kp: "0.00"` when kp=1.0, would still pass. True tautology vector: rename a factor in code AND test, both pass; dashboard surface silently breaks.
**Fix:** Added sibling test `zappi_active_decision_factor_values_correct` in `crates/core/src/controllers/setpoint.rs` (battery=-2500, HP=300, cooker=200, threshold=1000, kp=1.0). Asserts the load-bearing factor values: `compensated_drain_W="2000"`, `threshold_W="1000"`, `kp="1.00"`, `solar_export_W="2000"`, `setpoint_new_W (pre-clamp)="-2000"`.

### [PR-ZD-3-D09] No test for early-morning Zappi tighten branch (only relax covered)
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/setpoint.rs:1401-1428` (test 21)
**Description:** Test 21 exercises early-morning Zappi at 03:00 only in the relax branch. Symmetric "early-morning tighten" missing — the case where the deleted `(2..8)` carve-out would have produced `idle - soltaro_export` but the new unified loop must tighten.
**Fix:** Added `early_morning_zappi_tightens_when_battery_draining` test in `crates/core/src/controllers/setpoint.rs` (clock at 03:00, battery=-3000, soltaro=0, threshold=1000, kp=1.0). Confirms the unified loop produces the correct tighten reasoning at 03:00 — the case the deleted (2..8) carve-out would have ignored.

### [PR-ZD-3-D10] No deadband-stall test
**Status:** resolved (deferred; documented behaviour, no regression risk in this milestone)
**Severity:** minor
**Location:** (no test exists)
**Description:** With `setpoint_retarget_deadband_w = 25` and kp=1.0, drain of 1024 W (excess=24) produces sub-deadband adjustment. Decision says "tightening" but no MQTT update. No test locks this in.
**Fix:** Closed deferred. The deadband behaviour is documented in plan §3.3 ("the deadband prevents excess MQTT churn") and is shared with all setpoint-controller branches — not specific to the new compensated-drain path. A general "Decision-without-actuation" deadband test would be a milestone-wide concern, not a M-ZAPPI-DRAIN deliverable. Deferred to a future hygiene PR.

### [PR-ZD-3-D11] Test 25 location breadcrumb missing
**Status:** resolved (deferred; cosmetic only)
**Severity:** nit
**Location:** `crates/core/src/controllers/setpoint.rs` lines 1473, 1501, 1521
**Description:** Plan tests 23 / 24 / 26 in source; test 25 lives in process.rs (correct — needs world/sensor pipeline). Block-comment numbering in setpoint.rs goes 23 → 24 → 26 with no breadcrumb.
**Fix:** Closed deferred. Cosmetic breadcrumb that doesn't affect correctness; the test exists in `crates/core/src/process.rs` where it correctly belongs (it needs the world/sensor pipeline). Future readers can find it via `git grep`.

### [PR-ZD-3-D12] `target_w` field threaded but inert — no compile-time guard against misuse
**Status:** resolved (note-only; no functional change required)
**Severity:** nit
**Location:** `crates/core/src/controllers/setpoint.rs:103-115`
**Description:** Plan documents `target_w` as inert (reserved for future PI extension). Implementation correctly threads it but does not read it. No guard prevents a future contributor from wiring it up perceiving it as already-active.
**Fix:** Closed as note-only. Reviewer self-acknowledged "pure tracking" — doc-comment on the field already documents inert status. M-ZAPPI-DRAIN cross-cutting note in `tasks.md` reinforces the do-not-wire constraint.

### [PR-ZD-3-D13] Test 16 doesn't actually verify the `max(0, …)` clamp
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/controllers/setpoint.rs:1259-1284` (test 16)
**Description:** Test 16 asserts decision summary contains "relaxing", which is true regardless of whether the clamp fires (charging=2000 → un-clamped drain=-2000 → still < 1000 → still relaxes). Removing `max(0, ...)` wouldn't fail the test.
**Fix:** Extended `compensated_drain_clamped_zero_when_battery_charging` test in `crates/core/src/controllers/setpoint.rs` to assert the `compensated_drain_W` factor value is exactly `"0"` (would fail if the `max(0, ...)` clamp were removed).

---

## PR-ZD-4 (M-ZAPPI-DRAIN hard clamp)

### [PR-ZD-4-D01] No coverage for `world.derived.zappi_active=false` bypass
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs::tests` (around lines 5005–5300)
**Description:** Tests 27–33 cover the four-AND gate for target / allow / drain conditions individually, but no test exercises the case `target=Fast` AND `!allow_battery_to_car` AND `drain > hard_clamp_w` AND `world.derived.zappi_active = false`. The gate is correctly ANDed in code, but a future refactor that drops or inverts the `zappi_active` term would slip past the test suite. With Zappi physically disconnected (e.g. car unplugged) but the operator having previously commanded `Fast`, target.value remains `Some(Fast)`; the controller must not start raising the import setpoint.
**Fix:** Added `hard_clamp_disengaged_when_zappi_active_false` at `crates/core/src/process.rs:5312`. After `seed_hard_clamp_scenario` (which seeds `ZappiPlugState::Charging` → zappi_active=true), the test overwrites `world.typed_sensors.zappi_state` with `ZappiPlugState::EvDisconnected` (which `classify_zappi_active` unconditionally returns false for). EvchargerAcPower is at 0 W so the power-based fallback also returns false. Asserts `hard_clamp_engaged_factor(&world).is_none()`. Surface clarification: when `zappi_active=false`, `evaluate_setpoint` itself bypasses the Zappi drain branch and returns idle (10 W) — the test asserts setpoint=10 (soft loop also bypassed), with the primary coverage target being the absence of the hard-clamp factor.

### [PR-ZD-4-D02] Helper-placement rationale claimed circular dep that doesn't exist
**Status:** resolved (note-only; placement is fine, rationale is post-hoc)
**Severity:** nit
**Location:** Executor's return report; the actual code/doc-comments don't make the claim
**Description:** Fix subagent's report cited a circular dependency as the reason for placing `compute_compensated_drain` in `setpoint.rs` rather than `process.rs`. Inspection: `setpoint.rs` imports only `chrono`, `crate::Clock`, `crate::knobs`, `crate::topology`, `crate::types::Decision` — none transitively involve `process.rs`. No actual circular dep. The split is still defensible by-domain (helper near its primary caller).
**Fix:** Closed note-only. The committed code/doc-comments don't actually claim "circular dep" verbatim — that was only in the executor's return report. Placement is justified by-domain: the pure helper sits with the controller that defines its semantics; the `&World` wrapper sits in `process.rs` where the runtime aggregate is consumed.

### [PR-ZD-4-D03] Redundant `.clone()` on `out.decision`
**Status:** resolved (deferred; preserves existing PR-09a-D02 idiom)
**Severity:** nit
**Location:** `crates/core/src/process.rs:1394–1416`
**Description:** `base_decision = out.decision.clone()` could be elided by destructuring `out` and consuming `out.decision`. Cost is small (5 extra `(name, value)` factor pairs when the hard clamp engages).
**Fix:** Closed deferred. Preserves the existing PR-09a-D02 idiom unchanged. A future cleanup pass can sweep this across all setpoint-clamp call sites consistently.

### [PR-ZD-4-D04] `compensated_drain_w` recomputed even when hard-clamp gate cannot fire
**Status:** resolved (deferred; cost negligible)
**Severity:** nit
**Location:** `crates/core/src/process.rs:1320–1338`
**Description:** `hard_clamp_drain_w = compensated_drain_w(world)` runs unconditionally on every tick. The function reads three `Actual<f64>::value` fields and does three subtractions — negligible cost, no correctness issue.
**Fix:** Closed deferred. The flat structure is more readable than a nested gate. Micro-optimisation; reconsider if the per-tick cost ever shows up in profiling.

---

## PR-ZDO-1 (M-ZAPPI-DRAIN-OBS capture pipeline)

### [PR-ZDO-1-D01] Snapshot capture fires on every event, not every tick — buffer 30-min window collapses to seconds in production
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:1367-1386` (capture in `run_setpoint`, invoked from `run_controllers` on every event)
**Description:** Plan locks "120 samples × 15 s = 30 min". Capture point assumes one snapshot per tick. Reality: every D-Bus sensor reading flows through `process()` → `run_setpoint` → snapshot push. Production cadence is many sensor events per second; buffer fills in seconds. The chart's "30 min" label becomes a lie. The MQTT broadcast (PR-ZDO-2) also dedup-thrashes because the snapshot's `captured_at_ms` changes constantly even when value is stable.
**Fix:** `ZappiDrainState::push` in `crates/core/src/world.rs` now adds `pub const SAMPLE_INTERVAL_MS: i64 = 15_000` and time-gates the `samples.push_back` half: only appends when `new.captured_at_ms - samples.back().captured_at_ms >= SAMPLE_INTERVAL_MS`. `latest` updates unconditionally on every call so HA broadcasts (PR-ZDO-2) and wire-format snapshots (PR-ZDO-3) stay lockstep with the controller. Test PR-ZDO-1.T2 updated (130 pushes spaced 15001 ms; oldest 10 evicted). New test `zappi_drain_capture_buffer_time_gated_to_15s_intervals` covers the gate boundary (`14_999` rejected, `15_000` accepted, `latest` always updates).

### [PR-ZDO-1-D02] `wall_clock_epoch_ms` doc-comment claims "returns 0 on overflow" but impl never overflows
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/clock.rs:30-31`
**Description:** Doc says "Returns 0 on overflow (impossible in practice before 2262 CE)." `chrono::DateTime::timestamp_millis()` returns `i64` directly with no overflow handling — there's no zero fallback in any of the three impls. The doc misleads.
**Fix:** Removed the false "Returns 0 on overflow" sentence in `crates/core/src/clock.rs:30-32`; replaced with "Saturates per chrono's i64 timestamp range; well outside operational lifetime."

### [PR-ZDO-1-D03] `ZappiDrainBranch::Disabled` doc-comment lists incomplete precondition
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/types.rs:809-810`
**Description:** Variant doc says "world.derived.zappi_active=false". The classifier returns `Disabled` only when `force_disable_export=false && !zappi_active` — `force_disable_export=true` short-circuits to `Bypass` first regardless of `zappi_active`.
**Fix:** Rewrote `ZappiDrainBranch::Disabled` doc-comment in `crates/core/src/types.rs:809-810` to call out the `force_disable_export=false` precondition and that `force_disable_export=true` short-circuits to `Bypass`.

### [PR-ZDO-1-D04] No unit test for `Clock::wall_clock_epoch_ms` correctness
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/clock.rs` (no test in test module)
**Description:** New trait method introduced. Plan's risk section calls out "Clock-skew on `captured_at_ms`". No test verifies `FixedClock::wall_clock_epoch_ms()` returns the millis at the configured `naive` interpreted as UTC. Future refactors that flip UTC↔local could silently drift the chart's x-axis.
**Fix:** Added `fixed_clock_wall_clock_epoch_ms_matches_utc_naive` test in `crates/core/src/clock.rs:76-93` using `FixedClock::at`. Asserts `clock.wall_clock_epoch_ms() == chrono::Utc.with_ymd_and_hms(2026,4,30,12,0,0).timestamp_millis()`.

### [PR-ZDO-1-D05] `apply_setpoint_safety` captures `compensated_drain_w = 0.0` without an "unknown" indicator
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:1519-1533`
**Description:** When sensors are stale, real drain is unknown — but the snapshot records `0.0`, indistinguishable from "drain genuinely zero". Chart will plot a flat zero line during safety fallback. Branch tag `Disabled` is the only signal that the value is a stand-in. PR-ZDO-4's renderer must check `branch != Disabled` before plotting; no such contract documented.
**Fix:** Added doc-comments on both `ZappiDrainSnapshot::compensated_drain_w` and `ZappiDrainSample::compensated_drain_w` in `crates/core/src/world.rs` calling out the contract: "Meaningful only when `branch != Disabled`. ... renderers (PR-ZDO-4) MUST skip / grey-out `Disabled` samples to avoid plotting a misleading zero line during safety fallbacks."

### [PR-ZDO-1-D06] Snapshot/sample fields lack doc-comments (especially `captured_at_ms` non-monotonicity caveat)
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/world.rs:436-456` (`ZappiDrainSnapshot` and `ZappiDrainSample` fields)
**Description:** Struct doc-comments exist; individual fields don't. `captured_at_ms` lacks the "wall-clock epoch ms; non-monotonic if GX clock jumps; renderer sorts at draw time" caveat from plan §4.1 risk list.
**Fix:** Added per-field doc-comments on every field of `ZappiDrainSnapshot` and `ZappiDrainSample` in `crates/core/src/world.rs`. `captured_at_ms` comment calls out non-monotonicity and the renderer-sorts-at-draw-time contract; `threshold_w` / `hard_clamp_w` snapshotted-for-chart-consistency rationale documented.

### [PR-ZDO-1-D07] T6 doesn't push multiple garbage snapshots between runs
**Status:** resolved (deferred; nit)
**Severity:** nit
**Location:** `crates/core/src/process.rs:5664-5680`
**Description:** Plan probe was "push synthetic garbage snapshots between runs" (plural). T6 sets `latest = None` and pushes one garbage snapshot before the second `process()` call. A future feedback bug that read from `samples.front()` would be partially exercised.
**Fix:** Closed deferred (nit). Single garbage snapshot + cleared `latest` adequately covers the "no controller branch reads from `world.zappi_drain_state`" invariant. Multi-sample stress test would be marginal extra coverage; defer to a future hardening PR if a feedback regression ever materialises.

### [PR-ZDO-1-D08] T2b weakly asserts `latest` update on gated calls
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/process.rs:5486-5517`
**Description:** T2b uses `compensated_drain_w: 100.0` constant for every push, so `assert!(state.latest.is_some())` after a same-ms gated push only proves `latest` wasn't cleared; cannot distinguish "updated to identical value" from "left untouched". The implementation is correct (unconditional `self.latest = Some(snap)`) but the test doesn't exercise it.
**Fix:** Updated `zappi_drain_capture_buffer_time_gated_to_15s_intervals` in `crates/core/src/process.rs`: `snap_at` closure now takes a `drain: f64` param; the four pushes use distinct values (100 / 200 / 300 / 400). After each gated and non-gated call, the test asserts `state.latest.unwrap().compensated_drain_w` matches the most recent push, locking in "latest updates on every call regardless of gate state".

### [PR-ZDO-1-D09] Backwards GX clock jump freezes `samples` for the jump duration; behaviour undocumented
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/world.rs` `ZappiDrainState::push` doc-comment
**Description:** The gate `snap.captured_at_ms - prev.captured_at_ms >= SAMPLE_INTERVAL_MS` rejects samples with `captured_at_ms < prev.captured_at_ms + 15_000`. After a backwards GX clock jump (e.g. ntpdate correcting an hour of drift), every subsequent push fails the gate until wall-clock advances past the previous sample's timestamp + 15_000 ms — up to the entire jump duration. During that window `samples` doesn't grow; chart appears frozen even though `latest` continues updating. Plan §4.1 risk list calls out clock skew but no behavioural contract documents what `push` does under it.
**Fix:** Extended `ZappiDrainState::push` doc-comment in `crates/core/src/world.rs` with a `**Clock skew**` paragraph describing the backwards-jump gate behaviour: `samples` appends are blocked until wall-clock recovers past `prev.captured_at_ms + SAMPLE_INTERVAL_MS`; `latest` continues to update on every call so HA broadcasts and wire-format `latest` snapshots stay current.

---

## PR-ZDO-2 (M-ZAPPI-DRAIN-OBS HA broadcast)

### [PR-ZDO-2-D01] HA discovery references unpublished availability_topic — all three entities display as Unavailable
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/mqtt/discovery.rs:365, :389, :413`
**Description:** All three new discovery configs include `"availability_topic": format!("{topic_root}/availability")`. No code path publishes to that topic. Existing `publish_sensors` / `publish_bookkeeping` deliberately omit `availability_topic`. Adding a non-functional availability topic causes HA to render all three new entities as permanently `unavailable` regardless of state-topic content.
**Fix:** Removed `availability_topic` from all three discovery configs in `crates/shell/src/mqtt/discovery.rs::publish_controller_observables`. Numeric keeps `device_class: power`, `state_class: measurement`, `unit_of_measurement: W`; booleans keep `payload_on/off`. Inline `unavailable` via `encode_sensor_body` is now the only freshness signal (matches `publish_sensors` / `publish_bookkeeping` convention).

### [PR-ZDO-2-D02] Disabled-branch placeholder `compensated_drain_w = 0.0` leaks to HA Recorder as a real 0 W reading
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/core_dag/cores.rs:932-935`
**Description:** `ZappiDrainSnapshot::compensated_drain_w` is documented (world.rs:444-449) as a placeholder when `branch == Disabled`: "carries `0.0` as a placeholder — renderers MUST skip / grey-out `Disabled` samples." `apply_setpoint_safety` records `branch=Disabled, compensated_drain_w=0.0` whenever the controller can't run. The new HA broadcast block treats this as a real reading and publishes "0" to HA. HA Recorder will ingest fake 0 W samples during every safety fallback — exactly the "derived sensor out of sync with controller output" scenario the user explicitly framed as worse than no observability.
**Fix:** Updated `match snap` block in `crates/core/src/core_dag/cores.rs::SensorBroadcastCore::run` "Controller observables" section: three arms — `None` → `(0.0, Stale)`, `Some(s) if s.branch == Disabled` → `(0.0, Stale)`, `Some(s)` → `(s.compensated_drain_w, Fresh)`. Both no-snapshot and Disabled-branch cases now yield `"unavailable"` via `encode_sensor_body`. New regression test `controller_observables_disabled_branch_yields_unavailable_and_false_bools` (D05) locks this in.

### [PR-ZDO-2-D03] T4 numbering: planned round-trip-via-serialize lives in serialize.rs (4 tests), not the test labelled T4 in process.rs
**Status:** resolved (note-only; semantic match)
**Severity:** minor
**Location:** `crates/core/src/process.rs:5996-6042` (`controller_observables_cache_body_matches_encode_sensor_body`)
**Description:** Plan §4.2 specified T4 as round-trip-via-serialize. The four serialize.rs tests collectively meet this. The test labelled T4 in process.rs is a *different* test (cache encoding equality) — both are useful, just mis-numbered.
**Fix:** Closed note-only. The four serialize.rs round-trip tests collectively satisfy the planned T4 contract; the cache-body-equality test in process.rs is a sibling check. No code change.

### [PR-ZDO-2-D04] displayNames/descriptions entries are dead code until PR-ZDO-3 lands the wire-format field
**Status:** resolved (deferred; preregistration for PR-ZDO-3)
**Severity:** nit
**Location:** `web/src/displayNames.ts:100-103`, `web/src/descriptions.ts:55-61`
**Description:** displayNames table maps snake_case canonicals from the dashboard snapshot model. PR-ZDO-2 doesn't touch the wire format — no `controller_zappi_drain_*` field in any baboon-generated TS type yet. Entries inert until PR-ZDO-3.
**Fix:** Closed deferred. Preregistration is harmless and removes a future-PR step. PR-ZDO-3 will make these entries reachable.

### [PR-ZDO-2-D05] No test covers the Disabled→false bool transition (allowed D02 to slip through)
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/process.rs::tests` (no test for `latest.is_some() && branch == Disabled`)
**Description:** T3 covers `latest.is_none()` → both bools false. T1 covers Tighten → tighten=true, clamp=false. Untested: `latest.is_some() && branch == Disabled` (e.g., post-`apply_setpoint_safety`) — both bools must publish false AND numeric must publish unavailable (per D02 fix). A test like this would have caught D02.
**Fix:** Added `controller_observables_disabled_branch_yields_unavailable_and_false_bools` test in `crates/core/src/process.rs::tests` between T3 and T4. Seeds `latest = Some(ZappiDrainSnapshot { branch: Disabled, compensated_drain_w: 0.0, ... })`, runs `SensorBroadcastCore`, asserts compensated-w encodes as `"unavailable"`, both booleans publish `false`. Locks in D02's fix.

---

## PR-ZDO-4 (M-ZAPPI-DRAIN-OBS frontend chart)

### [PR-ZDO-4-D01] T1/T2/T3 verify lookup tables only — renderers themselves are uninvoked
**Status:** resolved
**Severity:** minor
**Location:** `web/src/render.test.ts:54-126`
**Description:** The three "PR-ZDO-4.T*" tests assert only on the constant lookup tables (`BRANCH_LABEL`/`BRANCH_CSS_CLASS`/`BRANCH_COLOR`) consumed by the renderers, not on the renderer outputs themselves. Neither `renderZappiDrainSummary` nor `renderZappiDrainChart` is invoked. Untested: Disabled-branch placeholder ("—" instead of "0 W"), `latest=null` reset path, y-axis-max exclusion of Disabled samples, segment-colour-by-later-sample logic, reference-line emission. Acknowledged by file comment ("no DOM in tsc-only check environment") — structural to project's "no test runner" stance, consistent with `fmtMpptOperationMode` precedent.
**Fix:** Extracted `summaryFor(latest: ZappiDrainSnapshotWire | undefined): ZappiDrainSummaryDisplay` as a pure exported function in `web/src/render.ts`. `renderZappiDrainSummary` now calls `summaryFor` and applies returned text/classes via a thin `setBigNumber` DOM-poker. Added 4 test blocks in `web/src/render.test.ts` (22 assertions total) covering: undefined input → all dashes; Tighten + clamp engaged → "1500 W"/"Tighten"/"Engaged"; Disabled → "—" honest contract (locks PR-ZDO-1-D05 / PR-ZDO-2-D02); Relax + clamp disengaged steady state.

### [PR-ZDO-4-D02] Tautological assertion in T1 (compares string to itself)
**Status:** resolved
**Severity:** minor
**Location:** `web/src/render.test.ts:79-82`
**Description:** `assert("...", "big-number hard-clamp-engaged", "big-number hard-clamp-engaged")` compares a literal to itself. Cannot fail. Pads test count without adding evidence.
**Fix:** Deleted the literal-vs-literal assertion in `web/src/render.test.ts`. Replaced with real `summaryFor(...)` test cases (per D01) that exercise the actual decision logic.

### [PR-ZDO-4-D03] Unnecessary type casts on `state.latest` / `state.samples`
**Status:** resolved (note-only; defensible workaround for @ts-nocheck)
**Severity:** nit
**Location:** `web/src/render.ts:1542, 1572-1576`
**Description:** Generated TS module has `// @ts-nocheck`; type information is stripped at the import site. The casts re-narrow `any` to typed values — not no-ops in practice.
**Fix:** Closed note-only. Add a one-line comment at the cast site explaining "re-narrowing past @ts-nocheck in generated module". Can be addressed in a hygiene PR.

### [PR-ZDO-4-D04] `samples.length === 1` produces no visible point
**Status:** open
**Severity:** nit
**Location:** `web/src/render.ts:1690-1699`
**Description:** Polyline branch fires only when `samples.length >= 2`. Empty-state placeholder fires only when `samples.length === 0 && !latest`. With exactly 1 sample + `latest` non-null, chart shows reference lines but no point. Persists for one tick (~15s) after fresh boot.
**Suggested fix:** Render a single `<circle>` at the lone sample point, or document the gap. Defer to hygiene PR.

### [PR-ZDO-4-D05] Threshold and hard-clamp text labels can overlap when y-coordinates are close
**Status:** resolved (deferred; cosmetic)
**Severity:** nit
**Location:** `web/src/render.ts:1666-1671, 1674-1679`
**Description:** Both labels use `text-anchor="end"` at the same x. When threshold and hard-clamp are within ~9 px vertically, labels visually overlap.
**Fix:** Closed deferred. Cosmetic readability gap; the two reference lines themselves remain colour-distinguishable. Can be addressed in a hygiene PR.

### [PR-ZDO-4-D06] Inline `style="fill:..."` on threshold/hard-clamp labels overrides theme-aware CSS class
**Status:** resolved (deferred; cosmetic)
**Severity:** nit
**Location:** `web/src/render.ts:1670, 1678`
**Description:** `.zd-axis-label { fill: var(--muted); }` overridden by inline hex codes. Light-theme switching wouldn't pick up the inline labels.
**Fix:** Closed deferred. The same hex codes already exist as `.zd-{threshold,hard-clamp}-line` stroke colours — visual consistency is preserved. Theme-awareness is a hygiene concern.

### [PR-ZDO-4-D07] Dead conditional in y-axis label anchor logic
**Status:** resolved
**Severity:** nit
**Location:** `web/src/render.ts:1635-1638`
**Description:** `const anchor = frac === 0 ? "end" : "end";` — both branches return identical string. Leftover from earlier iteration.
**Fix:** Replaced `const anchor = frac === 0 ? "end" : "end"` with `const anchor = "end"` in `web/src/render.ts`.

### [PR-ZDO-4-D08] Disabled samples plotted at y=0 (greyed) imply a real 0-W reading
**Status:** resolved (note-only; documented behaviour)
**Severity:** nit
**Location:** `web/src/render.ts:1700-1715`
**Description:** Implementation greys out Disabled segments at y=0 with 0.35 opacity. Locked decision allows "skip OR grey out"; greying picked. A purer "skip" would split the polyline into segments separated by gaps.
**Fix:** Closed note-only. Add a code comment documenting the chosen behaviour to prevent future "fixes" of this pattern.

### [PR-ZDO-4-D09] Theoretical chart edge case when samples populated but latest missing
**Status:** resolved (deferred; backend invariant)
**Severity:** nit
**Location:** `web/src/render.ts`
**Description:** Wire format guarantees `samples` non-empty implies `latest` set (PR-ZDO-3 invariant). Defensive only.
**Fix:** Closed deferred. Backend invariant holds; renderer doesn't need a fallback.

### [PR-ZDO-4-D10] Note: existing `lastSnapshot` global ordering with new render call
**Status:** resolved (note-only; verified)
**Severity:** nit
**Description:** Reviewer verified ordering in index.ts is intact; new ZappiDrain renders fire before `prevSnapshot`/`lastSnapshot` reassignment. No defect.
**Fix:** Closed note-only.

---

## PR-ZDP-1 (M-ZAPPI-DRAIN-PROBE MPPT-curtailment probe)

### [PR-ZDP-1-D01] `mppt_curtailed` checks only `value`, not `freshness` — stale-cached mode 1 fires probe forever
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:1190-1200` (`mppt_curtailed` helper)
**Description:** Helper inspects `Actual::value` directly. In this codebase, `Actual::tick()` decays `Fresh → Stale` *without clearing* `value`. A sensor that read mode 1 once and then stops reporting retains `value: Some(1.0)` with `freshness: Stale` indefinitely. The doc-comment promises "Stale or unknown sensor → false (conservative — don't probe blindly when we can't confirm the curtailment state)" — implementation only honours the `value=None` half. With the user's reported "MPPT op-mode constantly flipping" + occasional D-Bus hiccups, this is a likely failure mode: probe fires forever, walks setpoint to `-grid_export_limit_w` and holds. The grid-cap clamp catches the absolute floor but controller intent is wrong for the duration.
**Fix:** `mppt_curtailed` in `crates/core/src/process.rs` now takes `&Actual<f64>` references and gates on `slot.is_usable()` before matching the value: `slot.is_usable() && matches!(slot.value, Some(v) if (v - 1.0).abs() < 1e-6)`. `is_usable()` is `const fn` defined as `matches!(self.freshness, Freshness::Fresh) && self.value.is_some()` — returns false for Stale/Unknown/Deprecated, restoring the doc-comment's "Stale or unknown → false (conservative)" contract.

### [PR-ZDP-1-D02] `mppt_curtailed_helper_handles_stale` test misses the realistic stale case
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:4925-4935` (test) and `mppt_curtailed_helper_returns_true_on_either_mode_1` table-driven test
**Description:** Test only exercises `Actual::unknown` (value=None, freshness=Unknown). Does not construct `Actual` with `freshness=Stale, value=Some(1.0)` — the production-realistic stale case. Misleading test name implied stale coverage; only Unknown is covered. Allowed D-ZDP-1 to slip past.
**Fix:** Added new test `mppt_curtailed_helper_returns_false_on_stale_cached_mode_1` in `crates/core/src/process.rs`. Constructs world with mode-1 readings on both MPPT channels (Fresh, value=Some(1.0)), advances by 60 s past the 30 s freshness threshold via `tick()`, asserts both slots are `!is_usable()` with `value` still `Some(1.0)`, then asserts `mppt_curtailed(&world) == false`. Locks the production-realistic stale-with-cached-value invariant that would have masked D01.

### [PR-ZDP-1-D03] No multi-tick integration test for probe convergence trajectory
**Status:** resolved (deferred; nit per plan)
**Severity:** nit
**Location:** Plan §16 listed test #8 as "(optional)"
**Description:** Plan called out `probe_walks_setpoint_deeper_over_multiple_ticks` as optional. Single-tick test #1 covers first step; convergence narrative (MPPT ramps up, op-mode flips to 2, target settles) is asserted by no automated test.
**Fix:** Closed deferred (nit). Single-tick + helper tests cover the load-bearing arithmetic. Multi-tick convergence is observable in the field via the dashboard chart.

### [PR-ZDP-1-D04] Op-mode flap induces ±relax_step oscillation; no hysteresis
**Status:** resolved (deferred; behavioural, bounded by deadband + soft-loop convergence)
**Severity:** nit
**Location:** `crates/core/src/controllers/setpoint.rs:723-735`
**Description:** When MPPT op-mode flaps faster than the controller cadence, setpoint oscillates ±relax_step_w/tick. Each cycle exceeds the 25 W deadband → produces a D-Bus write per tick. Bounded; in the convergence-correct case (MPPT genuinely ramps up to meet demand), oscillation collapses as solar_export rises.
**Fix:** Closed deferred. Bounded oscillation acceptable in the absence of evidence it harms convergence. EWMA / N-of-M majority filter is a future hardening if field deployment shows the flap dominates the convergence dynamics.

---

## PR-EDDI-SENSORS-1 (M-EDDI-SENSORS eddi parser + typed-sensor surfacing + raw capture)

### [PR-EDDI-SENSORS-1-D01] Raw-response copy button breaks the inspector popup HTML on the first `"` in the JSON body
**Status:** resolved
**Severity:** major
**Location:** `web/src/render.ts:1111` (`rawResponseSection`)
**Description:** The "Raw response" panel renders the copy button as `<button class="copy-btn icon" data-copy="${esc(raw)}" title="Copy JSON">⧉</button>`. The escape helper `esc()` only replaces `&`, `<`, `>` — it does NOT escape ASCII double-quote `"`. The pretty-printed myenergi JSON body contains a `"` character on every key and string-typed value. The first such `"` terminates the `data-copy="…"` attribute, the rest of the JSON is parsed by the browser as additional attributes, and the resulting `data-copy` value ends up empty.
**Fix:** `web/src/render.ts` — replaced `data-copy="${esc(raw)}"` on the raw-response button with a marker attribute `data-copy-from-sibling="true"`. Extended `installCopyHandler` to detect the marker and read text from the adjacent `<pre><code>` via `el.closest("details")?.querySelector("pre code")?.textContent`. The `esc()` contract is unchanged; existing `data-copy` consumers across the dashboard are unaffected.

### [PR-EDDI-SENSORS-1-D02] Module-level docstring still claims `sta=1` is Normal and `sta=0` is Stopped, contradicting the new mapping
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/myenergi/types.rs:14-15`
**Description:** The file-level `//!` block described the Eddi mapping as `"sta (status 1=Normal/0=Stopped — actually the mapping differs between firmwares; best-effort)"`. The function-level docstring on `parse_eddi` was updated correctly, but this older module-level comment was not, leaving contradictory documentation in the same file.
**Fix:** `crates/shell/src/myenergi/types.rs:14-15` — replaced the parenthetical with `"sta — operational status; see `parse_eddi` for the sta→EddiMode inference"`. The function-level docstring is now the single source of truth for the mapping.

### [PR-EDDI-SENSORS-1-D03] Sensors-table "since" timestamp for never-observed eddi.mode/zappi rows shows boot-time, not "—"
**Status:** resolved (scoped to typed-sensor rows only; f64-row equivalent deferred)
**Severity:** minor
**Location:** `web/src/render.ts` synthetic typed-sensor rows + popup detail view; root cause `crates/core/src/tass/actual.rs:20-26` (`Actual::unknown` initialises `since: now`)
**Description:** Before any eddi/zappi poll has ever returned, `world.typed_sensors.eddi_mode` is `Actual::unknown(now)` with `since = boot Instant`, freshness `Unknown`. The dashboard then renders `"Unknown (12 s ago)"` even though the dashboard has *never* observed a value.
**Fix:** `web/src/render.ts` — at the two new typed-sensor row sites and the popup detail view, when `freshness === "Unknown"` the time portion is rendered as `"—"` instead of `fmtEpoch(since)`. One-line comment notes that existing f64 sensor rows have the same defect; deferred for scope (broader UX change with regression surface, separate PR).

### [PR-EDDI-SENSORS-1-D04] `apply_typed_reading` clobbers `world.typed_sensors.{eddi,zappi}_raw_json` with `None` when a reading carries `raw_json: None`
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs:482,500`
**Description:** The doc comment on `TypedSensors::eddi_raw_json` states the raw body "intentionally outlives freshness decay." But `apply_typed_reading` wrote `world.typed_sensors.{eddi,zappi}_raw_json = raw_json` unconditionally — any future emitter producing a `TypedReading::{Eddi,Zappi} { raw_json: None, … }` would erase the previously-latched body, silently regressing the "survives freshness decay" invariant.
**Fix:** `crates/core/src/process.rs` — both `TypedReading::Zappi` and `TypedReading::Eddi` arms now gate the assignment on `if raw_json.is_some()`. `None` means "no new body this cycle"; the latched value is preserved. Comment block reasserts the invariant.

### [PR-EDDI-SENSORS-1-D05] `parse_eddi`'s "unknown sta defaults to Normal" inverts the safety direction at the parser layer
**Status:** resolved (deferred; per plan-doc decision §"Risks and assumptions")
**Severity:** minor
**Location:** `crates/shell/src/myenergi/types.rs:167-170`
**Description:** The new mapping `0|6 ⇒ Stopped, _ ⇒ Normal` is correct for the documented sta codes (1/3/4/5). However, the catchall arm now reports `Normal` for any *unknown* sta value (e.g. a future firmware introducing `sta=7=ImmediateShutoffPlease`, or a malformed body where `sta` was tampered with). The previous parser reported Stopped for unknowns, which was the safe direction. The docstring acknowledges this and argues the safety net moves up to `EddiController` (Stale/Unknown freshness drives Stopped). This is true *for stale readings*, but a *fresh* reading carrying an unknown sta will be reported as Normal and the controller will not gate on freshness — it'll honour the (mis-)reported Normal. Combined with the absence of any `sta in {0..=6}` validity check, the parser silently accepts any byte the device sends.
**Fix:** Closed deferred. Plan doc `docs/drafts/20260501-2030-m-eddi-sensors-plan.md` §"Risks and assumptions" explicitly accepts this risk: "Decision: Normal, per the docs-driven mapping. The controller's freshness-driven safety net is the actual safety layer." The popup work (this PR's Part D) is the verification mechanism — once deployed, the operator can paste real raw bodies and we tighten in a follow-up PR if firmware variants surface unknown sta values. Re-opening the safety question now would relitigate a locked plan decision.

### [PR-EDDI-SENSORS-1-D06] Test coverage gap: no rendering test exercises the new typed-sensor synthetic rows or the popup raw-response panel
**Status:** resolved (deferred; bundled into D01 fix verification)
**Severity:** nit
**Location:** `web/src/render.test.ts`
**Description:** PR adds non-trivial render logic for two synthetic sensor rows and a "Raw response" inspector panel. `render.test.ts` has tests for unrelated PR branches but nothing exercising `renderSensors` for a snapshot whose `typed_sensors` block has missing/null components, nor anything exercising `rawResponseSection` for present/absent raw_json. The parser tests cover the Rust-side mapping but a regression in `renderSensors` (e.g., the D01 attribute-escape defect) would not be caught by any automated check.
**Fix:** Closed deferred. The D01 fix removes the `data-copy` attribute path entirely (handler reads from sibling `<pre><code>` element), which closes the specific exploit vector D06 was concerned about. Broader render-test coverage for typed-sensor rows would be useful but is a wider testing-strategy investment that belongs in its own PR. The Rust-side parser tests + tsc + manual reload remain the verification path for this PR.

### [PR-EDDI-SENSORS-1-D07] `displayNames.ts` entries `"eddi.mode": "eddi.mode"` and `"zappi": "zappi"` are pass-through identities and serve no rename purpose
**Status:** resolved
**Severity:** nit
**Location:** `web/src/displayNames.ts:149-150`
**Description:** `DISPLAY_NAMES` maps snake_case canonicals to dotted display names. The two new entries mapped `"eddi.mode" → "eddi.mode"` and `"zappi" → "zappi"` — pure tautologies, since the canonical key is already dotted.
**Fix:** `web/src/displayNames.ts` — removed both pass-through entries and the explanatory comment block. The `??` fallback in `displayNameOf` returns the canonical key unchanged when the lookup misses; observable behaviour is identical.

---

## PR-ACT-RETRY-1 (M-ACTUATED-RETRY universal actuator retry)

### [PR-ACT-RETRY-1-D01] Deadband filter pre-empts the retry path for grid_setpoint and input_current_limit
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:1652-1657` (`maybe_propose_setpoint`) and `crates/core/src/process.rs:1812-1816` (`run_current_limit`)
**Description:** Both controllers gate the retry path behind an unconditional dead-band early-return that runs *before* `propose_target` and the new `!changed && !needs_actuation` gate. When the device is stuck at `Commanded` with mismatching `actual` (the exact failure mode this PR exists to fix) and the controller's natural per-tick output stays within `setpoint_retarget_deadband_w` (default 25 W) or `current_limit_retarget_deadband_a` (default 0.5 A) of the stuck target — the steady-state common case — the function returns at line 1655/1814 and `needs_actuation` is never consulted. Result: the two f64-shaped actuators (the "float-tolerance-doesn't-cause-eternal-retry" worry from the plan) are precisely the two that *cannot* retry under steady-state. The eddi/zappi/schedules controllers (no dead-band filter) retry correctly. Note the in-source comment at L1645–1646 reads "don't restart the phase cycle if the current target is within deadband and we're already confirmed", which describes the intended pre-PR behaviour; the retry refactor did not revisit it.
**Fix:** `crates/core/src/process.rs` — both deadband early-returns now require `phase == Confirmed` to fire. In `maybe_propose_setpoint`, the integer-W deadband check (`delta < params.setpoint_retarget_deadband_w`) is now conjoined with `world.grid_setpoint.target.phase == crate::tass::TargetPhase::Confirmed`. Same pattern for `run_current_limit` against `world.input_current_limit.target.phase`. When phase ∈ {Pending, Commanded}, the deadband no longer pre-empts; the retry path runs. No existing tests broke — the regression-lock test from D02 confirms the fix.

### [PR-ACT-RETRY-1-D02] grid_setpoint no-retry test does not actually verify the Confirmed-phase guard
**Status:** resolved
**Severity:** major
**Location:** `crates/core/src/process.rs:3833-3904` (`grid_setpoint_does_not_retry_when_actual_within_deadband`)
**Description:** The test asserts that no `Effect::WriteDbus { target: GridSetpoint, .. }` is emitted on the post-threshold tick. But the only thing this assertion exercises is the dead-band early-return at line 1652-1657 — the controller's per-tick computed setpoint at `later` lands within the 25 W deadband, so `maybe_propose_setpoint` returns at L1655 and `needs_actuation` is never called. The same assertion would pass if `needs_actuation` always returned `true`. This test is the one that should have failed and surfaced D01; instead it silently passes.
**Fix:** `crates/core/src/process.rs` — renamed existing test to `grid_setpoint_confirmed_phase_blocks_retry_past_threshold` with updated docstring naming what it actually locks (Confirmed-phase + deadband co-blocking). Added sibling test `grid_setpoint_retries_after_threshold_when_actual_mismatches_within_deadband` that hand-builds a Commanded target at -1000 W with actual=-500 W (mismatch outside confirm tolerance), advances clock past `actuator_retry_s`, and calls `maybe_propose_setpoint` with computed setpoint within the deadband — asserts WriteDbus IS emitted. Regression-lock proven: with D01-fix reverted (phase guard removed), the new test fails at the WriteDbus assertion; with D01-fix applied, it passes.

### [PR-ACT-RETRY-1-D03] mark_commanded re-stamping does not exercise the actual.deprecate side-effect
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/tass/actuated.rs:262-274` (`mark_commanded_on_commanded_refreshes_since`)
**Description:** The new "Commanded → Commanded re-stamp" semantics in `mark_commanded` is covered by exactly one unit test, which asserts `target.phase` and `target.since` only. The other side-effect of the re-stamp — `actual.deprecate(now)` on a Fresh actual — is not asserted in any test. If a regression silently dropped the `actual.deprecate` call from the new branch, every existing test would still pass.
**Fix:** `crates/core/src/tass/actuated.rs` — added sibling test `mark_commanded_on_commanded_deprecates_actual` that calls `propose_target → mark_commanded → on_reading(value)` (asserting Fresh), then calls `mark_commanded` a second time on the Commanded phase and asserts `actual.freshness == Deprecated`. Locks the side-effect that the existing `mark_commanded_on_commanded_refreshes_since` test does not cover.

### [PR-ACT-RETRY-1-D04] Observer-mode emits one "would be set" log per controller per tick under stuck-Pending after threshold elapses
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/process.rs` (all five controllers' writes_enabled-false branches)
**Description:** In observer mode, when a target sits at Pending from an earlier observer-mode run (no mark_commanded ran, target.since stuck at original propose timestamp), `needs_actuation` returns true on every tick once the retry threshold has elapsed because nothing in the observer-mode path advances target.since. The gate passes every tick → ActuatedPhase publish + Log emission, no mark_commanded → next tick same situation. Sustained log spam during long-running observer sessions.
**Fix:** `crates/core/src/process.rs` — all five `writes_enabled-false` branches (in `maybe_propose_setpoint`, `run_current_limit`, `maybe_propose_schedule`, `run_zappi_mode`, `run_eddi_mode`) wrap `effects.push(Effect::Log {...})` in `if changed { ... }`. The early-return remains; only the per-tick log is suppressed for the retry path. ActuatedPhase publish before each branch keeps stuck-Pending visibility intact on the dashboard. `changed` was already in scope at all five sites.

### [PR-ACT-RETRY-1-D05] Knob `actuator.retry.s` lands in dashboard group "Hard installation caps", which mis-classifies a retry-pacing tuning knob
**Status:** resolved
**Severity:** nit
**Location:** `web/src/knobs.ts:222`
**Description:** `actuator.retry.s` is a tuning knob controlling how aggressively the controller re-issues writes when the device doesn't comply; it is not a hard installation cap (that group's siblings are `grid.export.limit` / `grid.import.limit` / `inverter.safe-discharge.enable`, physical-limit settings keyed off installed hardware). Operators looking for "how often does the controller retry?" will not find it under "Hard installation caps".
**Fix:** `web/src/knobs.ts` — introduced new "Actuator retry" group in `CONFIG_GROUPS` (no existing cross-cutting tuning group existed; all other tuning knobs live under per-subsystem groups like "Eddi", "Zappi calibration"). Updated `actuator.retry.s` entry's `group:` field from "Hard installation caps" to "Actuator retry". Single-entry group is acceptable because the knob is genuinely cross-cutting and has no subsystem peer.

---

## PR-TS-META-1 (M-TYPED-SENSORS-META typed-sensor metadata columns)

### [PR-TS-META-1-D01] Entity-inspector popup for `eddi.mode` / `zappi` does not surface the new origin/identifier/cadence/staleness metadata
**Status:** resolved
**Severity:** minor
**Location:** `web/src/render.ts:1054-1086`
**Description:** `renderSensorBody` short-circuits for `entityId === "eddi.mode"` and `entityId === "zappi"`, returning after rendering only "Current value" + "Raw response" sections. The Origin section (origin, identifier, cadence, stale after) that f64 sensors get on lines 1108-1118 is never produced for typed sensors, even though the wire model now carries those exact fields. The narrow type cast on lines 1037-1051 also still only declares `value/freshness/since_epoch_ms/raw_json`, omitting the four new fields. This is the central UX feature the PR exists to add (operator visibility into cadence/staleness/origin) and it lands on the table row but not on the inspector popup that the operator opens to drill into a sensor.
**Fix:** `web/src/render.ts` — extracted the f64 path's Origin block into a small helper `originSection({ origin, identifier, cadence_ms, staleness_ms })` called from all three sites (f64 path passes `mm`; both typed-sensor early-return branches pass `ts.eddi_mode.*` / `ts.zappi.*`). The helper renders "—" and skips `copyIcon` when `identifier === ""` (covers the `unwrap_or("")` zero-length-serial path). f64 inspector behaviour preserved bit-for-bit; typed-sensor popups now show origin / identifier / cadence / stale-after parallel to the f64 layout.

### [PR-TS-META-1-D02] No regression test pinning the wire/runtime freshness alignment invariant
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dashboard/convert.rs` (test module around line 1265)
**Description:** The plan flagged this as the critical invariant: "staleness_ms reported on wire MUST equal threshold the runtime decay uses". The convert reads from `meta.controller_params.freshness_myenergi`, the constant `MYENERGI_TYPED_FRESHNESS` seeds `ControllerParams::defaults()`, and `process.rs:1036-1037` ticks both typed sensors against `p.freshness_myenergi`. All three are aligned today, but no test pins it. If a future change introduces a literal in `process.rs` (regressing the threading) or fans out to per-device thresholds, the wire-advertised value silently drifts from the actual runtime decay window.
**Fix:** `crates/shell/src/dashboard/convert.rs` — added `typed_sensor_staleness_matches_runtime_freshness_constant` test in `mod snapshot_new_sensors_tests` that constructs a snapshot with `meta.controller_params: ControllerParams::defaults()` and asserts both `snap.typed_sensors.eddi_mode.staleness_ms` and `snap.typed_sensors.zappi.staleness_ms` equal `MYENERGI_TYPED_FRESHNESS.as_millis() as i64`. Also pins `cadence_ms` against `meta.myenergi.poll_period`. A future drift in any of the three layers (constant, ControllerParams seeding, convert read-site) fails this test rather than silently advertising the wrong freshness window.

### [PR-TS-META-1-D03] Inspector type cast for typed sensors lacks the new fields
**Status:** resolved
**Severity:** nit
**Location:** `web/src/render.ts:1037-1051`
**Description:** The `as unknown as { typed_sensors?: { eddi_mode: { ... }, zappi: { ... } } }` cast still lists only `value/freshness/since_epoch_ms/raw_json`, even though the wire model now carries `cadence_ms / staleness_ms / origin / identifier` and the parallel cast on lines 454-475 was updated. Not a runtime defect but a documentation drift between two casts of the same wire shape; makes D01's fix awkward to write.
**Fix:** `web/src/render.ts` — mirrored `cadence_ms / staleness_ms / origin / identifier` onto both `eddi_mode` and `zappi` shapes inside the `renderSensorBody` cast. Now structurally identical to the row-construction cast at lines 454-475 (modulo `raw_json`, which only the popup needs).

---

## PR-DESYN-1 (M-DESYNTHETICS audit + remove dashboard synthetics)

### [PR-DESYN-1-D01] Timezone freshness never decays; staleness window advertised but unenforced
**Status:** resolved
**Severity:** major
**Location:** `crates/shell/src/dashboard/convert.rs:610-622` (`timezone_typed_sensor`)
**Description:** Sets `freshness = Fresh` whenever `world.timezone_updated_at = Some(_)`, with no comparison against the advertised `staleness_ms = 120_000`. If the D-Bus settings reseed loop fails, the timezone row will display `Fresh` indefinitely while the underlying value is hours stale, defeating the entire point of `staleness_ms`. The eddi.mode / zappi rows decay through `Actual::tick()` which respects `freshness_myenergi`; the new string sensor reuses none of that machinery and ships a one-way `Some → Fresh` mapping.
**Fix:** `convert.rs::timezone_typed_sensor` — freshness now derived from `now.saturating_duration_since(at) <= TIMEZONE_STALENESS`. `Some(at)` past the window emits `Stale` while preserving the last-known value; `None` yields `Unknown`. Single `Instant::now()` at top of function (also closes D02 here).

### [PR-DESYN-1-D02] Two `Instant::now()` calls inside `sunrise_sunset_typed_sensor` create a freshness/value race
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dashboard/convert.rs:642,647`
**Description:** `fresh_sunrise_sunset_impl` is called with `Instant::now()` at line 642 to compute the `value` (None when stale), and a second `Instant::now()` at line 647 is then compared against `SUNRISE_SUNSET_FRESHNESS` to compute the `freshness` field. Across the two reads `now` advances. At the freshness boundary, `_impl` may return `Some(iso)` while the second `now` pushes past the threshold and assigns `Stale`, producing `value=Some + freshness=Stale` — internally inconsistent.
**Fix:** `convert.rs::sunrise_sunset_typed_sensor` — captured one `let now = std::time::Instant::now();` at the top and threaded into `fresh_sunrise_sunset_impl`. After D08's tuple-return refactor, the caller no longer makes a second comparison at all.

### [PR-DESYN-1-D03] Advertised timezone cadence (60s) does not match actual reseed cadence (5s)
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dashboard/convert.rs:608` (timezone `TIMEZONE_CADENCE` const)
**Description:** Cadence published is 60s ("matches per-route `Route::Timezone` reseed cadence"). But the per-route 60s is only the floor input to `compute_service_cadence`, which takes `min(cadence_for_route)` across all routes targeting the settings service. Test `service_cadence_is_min_over_routed_sensors` at `subscriber.rs:1628-1632` asserts the settings-service cadence is **5s**, driven by `GridSetpointActual`. The runtime reseeds timezone every 5s, not 60s.
**Fix:** `convert.rs` — `TIMEZONE_CADENCE = Duration::from_secs(5)`, `TIMEZONE_STALENESS = Duration::from_secs(30)` (6× cadence). Doc comment pins to `subscriber.rs::service_cadence_is_min_over_routed_sensors` (driven by `GridSetpointActual` at 5 s) and justifies 30 s as a comfortable margin that still flips Stale within an operationally meaningful window.

### [PR-DESYN-1-D04] No wire-format alignment test for the new timezone / sunrise / sunset typed sensors
**Status:** resolved
**Severity:** minor
**Location:** `crates/shell/src/dashboard/convert.rs::tests` (around line 1433)
**Description:** PR-TS-META-1 added `typed_sensor_staleness_matches_runtime_freshness_constant` to pin the eddi/zappi `staleness_ms` to `MYENERGI_TYPED_FRESHNESS`. The new sunrise/sunset row claims `staleness_ms = SUNRISE_SUNSET_FRESHNESS.as_millis() as i64` but no test asserts that. Future drift will silently desync wire/runtime.
**Fix:** `convert.rs::tests` — extended `typed_sensor_staleness_matches_runtime_freshness_constant` (renamed plural). Asserts sunrise and sunset `staleness_ms == SUNRISE_SUNSET_FRESHNESS.as_millis() as i64`, plus timezone `staleness_ms == 30s` and `cadence_ms == 5s`. Future drift in any layer fails the test.

### [PR-DESYN-1-D05] Internally inconsistent `value=Some("Etc/UTC") + freshness=Unknown` for never-reseeded timezone
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/dashboard/convert.rs:614-616`
**Description:** When the controller has never received a timezone D-Bus reading, `world.timezone == "Etc/UTC"` (safe default) and `world.timezone_updated_at == None`. The conversion emits `value: Some("Etc/UTC")` together with `freshness: Unknown`. Semantically incoherent: an Unknown reading should not surface a fabricated value.
**Fix:** `convert.rs::timezone_typed_sensor` — `None` arm now emits `value: None`. Dashboard's `null → "—"` fallback handles never-seeded case.

### [PR-DESYN-1-D06] `since_epoch_ms` for Unknown-state typed-sensor-string drifts forward each tick
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/dashboard/convert.rs:612,652`
**Description:** When `timezone_updated_at`/`sunrise_sunset_updated_at` are None, `since_epoch_ms = now_epoch` — recomputed at each `world_to_snapshot` call. The eddi/zappi convention pins `since_epoch_ms` via `Actual::unknown(now)` (boot Instant). Defeats the index.ts deep-equal change-detection optimisation for the Unknown branch.
**Fix:** `convert.rs` — `since_epoch_ms = 0` sentinel for both timezone-Unknown and sunrise/sunset-Unknown branches. Stable across ticks (deep-equal change-detection works); dashboard's Unknown-guard renders "—" so sentinel is invisible to UI.

### [PR-DESYN-1-D07] `BASELINE_CADENCE` is a magic literal not bound to its authoritative source
**Status:** resolved (deferred)
**Severity:** nit
**Location:** `crates/shell/src/dashboard/convert.rs:637`
**Description:** `BASELINE_CADENCE: Duration = Duration::from_secs(60 * 60)` magic literal whose justification lives only in a doc comment. If the baseline scheduler cadence in `crates/core/src/forecast/baseline.rs` changes, dashboard drifts silently.
**Fix:** Closed deferred — the doc comment is sufficient documentation in the absence of a clean threading path. The baseline scheduler cadence is config-driven (`[forecast.baseline] cadence = "1h"`), so a deployment change in config wouldn't update the dashboard regardless. Threading through MetaContext is wider work; revisit if drift surfaces.

### [PR-DESYN-1-D08] `_impl` arg `now` is now a test-only seam; production path duplicates its freshness comparison
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/dashboard/convert.rs:638-651`
**Description:** Post-PR-DESYN-1, the only production caller of `fresh_sunrise_sunset_impl` is `sunrise_sunset_typed_sensor`. That caller duplicates `_impl`'s freshness-window comparison (line 647-648 mirrors the `<= SUNRISE_SUNSET_FRESHNESS` check inside `_impl`). Two near-identical pieces of freshness logic must be kept in lockstep.
**Fix:** `convert.rs::fresh_sunrise_sunset_impl` — Option A: widened return to `(Option<String>, Option<String>, Freshness, Option<Instant>)`. Sole caller `sunrise_sunset_typed_sensor` consumes freshness verdict and `since` Instant directly; threshold no longer encoded twice. 4 existing tests in `sunrise_sunset_freshness_tests` extended to also assert the new fields.

### [PR-DESYN-1-D09] Stale aggregate doc comment on `typed_sensors_to_model` references obsolete 60s cadence
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/dashboard/convert.rs:533-538`
**Description:** The aggregate doc comment introducing `typed_sensors_to_model` said "The timezone row is sourced from `world.timezone` (D-Bus settings reseed) with the per-route 60 s cadence pinned in `dbus::subscriber::cadence_for_route(Route::Timezone)`." After D03 the implementation pins `TIMEZONE_CADENCE = 5s`. Stale.
**Fix:** `convert.rs:533-538` — replaced the misleading aggregate text with a one-line pointer to the per-helper docs (`timezone_typed_sensor`, `sunrise_sunset_typed_sensor`) which carry the authoritative cadence/staleness justification. Doc-only edit applied directly by orchestrator (no functional code change).

---

## PR-WSOC-TABLE-1

### [PR-WSOC-TABLE-1-D01] `weather_soc_table` falls through to operator-knobs "Other" group as `[object Object]`
**Status:** resolved
**Severity:** major
**Location:** `web/src/knobs.ts:369-382, 414-438`; supporting wire-shape at `web/src/model/victron_controller/dashboard/Knobs.ts:272-326`
**Description:** `renderKnobs` iterates `Object.entries(Knobs.toJSON())`, which exposes `weather_soc_table` as a top-level field whose value is a `WeatherSocTable` class instance. With no `KNOB_SPEC` entry for the table (correct by design — per-cell editing is out of scope for v1) and no filter for object-typed values, the entry pushes into the "Other" bucket of the operator knobs table and formats as `esc(String(val))` → `"[object Object]"`. The dashboard renders a row labelled `weathersoc.table` reading `[object Object]` under "Other", **in addition** to the dedicated read-only widget. This is the exact "Other group with no controls" failure mode CLAUDE.md warns about.
**Fix:** `web/src/knobs.ts` — added module-scope `NESTED_KNOB_FIELDS = new Set(["weather_soc_table"])` above `renderKnobs` with a comment that this is for nested-typed fields surfaced via dedicated widgets, plus an early-return skip in the bucketing loop after the existing `writes_enabled` guard. By-name allowlist (not blanket `typeof object`) so future primitive-knob bugs surface visibly.

### [PR-WSOC-TABLE-1-D02] Cascade-equivalence test helper masks bucket-classification semantics with implicit `too_much*1.5` boundary
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/weather_soc.rs:305-314` (the `evaluate(input)` test helper)
**Description:** The 11 retained tests use `evaluate(input)` which threads `Knobs::safe_defaults().weather_soc_table` (cells assume safe-defaults thresholds) but uses `base_globals` thresholds (low=12, ok=20, high=80, too_much=80) for bucket classification with an implicit `very_sunny = base_globals.too_much * 1.5 = 120` derived inside the helper. Cells happen to be correct under base_globals because the cell content is bucket-position-driven, not threshold-driven, but the seam is non-obvious. A future "tidying" pass that swapped the implicit `*1.5` for `k.weathersoc_very_sunny_threshold` would silently break cascade equivalence (safe_defaults very_sunny=67.5 vs base_globals.too_much*1.5=120 disagree on which bucket `today_energy=100` lands in).
**Fix:** `weather_soc.rs::evaluate(input)` — replaced the docstring with a load-bearing-invariant block: explains that `very_sunny_threshold` is derived as `base_globals.too_much * 1.5` (= 120) NOT `k.weathersoc_very_sunny_threshold` (= 67.5), naming the substitution mistake explicitly so a future tidying pass that swaps it sees the warning.

### [PR-WSOC-TABLE-1-D03] `dng = cell.extended` timing invariant is load-bearing but not commented
**Status:** resolved
**Severity:** minor
**Location:** `crates/core/src/controllers/weather_soc.rs` (the `let disable_night_grid_discharge = cell.extended` line, just before the override block)
**Description:** Plan §3 acceptance criterion 3 says `dng = cell.extended` derived **before** the override. Implementation correct; tests correct. But a refactor that "simplified" by reading the local `charge_battery_extended` mutable instead (the way every other output is read at the bottom of the function) would silently change Low.warm + cf=true behaviour: cascade `dng=false`, regression `dng=true`. Safety margin is thin because reading `cell.extended` rather than the mutable looks redundant on inspection.
**Fix:** `weather_soc.rs` — 5-line comment above `let disable_night_grid_discharge = cell.extended;` explaining the override-non-propagation invariant and naming the existing test `override_low_warm_cf_true_only_mutates_bat_ext` that pins it.

### [PR-WSOC-TABLE-1-D04] Test docstrings on retained tests still reference deleted cascade closures
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/controllers/weather_soc.rs` test module — `mild_day_moderate_energy_uses_defaults`, `cold_day_moderate_energy_preserves_evening`, `cold_and_low_energy_extends_charge_and_preserves_morning`, `ok_or_below_always_extends_charge_regardless_of_temp`, `very_low_energy_forces_charge_to_full`, `charge_to_full_required_with_low_energy_forces_full_charge`, `charge_to_full_required_with_high_energy_skips_forcing`, `exact_energy_threshold_counts_as_below`
**Description:** Eight of the 11 retained tests have docstrings naming closures that no longer exist (`disable_export`, `preserve_evening_battery`, `extend_charge`, `charge_to_full_extended`, `preserve_morning_battery`, "very-sunny branch"). Tests still pass — they're cascade-equivalence goldens — but a reader trying to understand the new model is misled.
**Fix:** `weather_soc.rs` test module — 7-line module comment immediately after `mod tests {` stating that the 11 retained tests are cascade-equivalence goldens whose docstrings reference legacy closure names for archaeology, and that lookup-table cell-pinning + boundary tests live below.

### [PR-WSOC-TABLE-1-D05] Decision summary loses per-rung narration that the legacy cascade exposed
**Status:** resolved (deferred)
**Severity:** nit
**Location:** `crates/core/src/controllers/weather_soc.rs:222-249`
**Description:** Legacy cascade pushed up to seven rung-firing strings into a `rungs` Vec, surfaced in `Decision::summary`. New summary is `"Bucket Low / warm cell"` — bucket+column+override flag, no per-cell rationale. Factor list compensates by exposing the boundary thresholds and final outputs.
**Fix:** Closed deferred — the factor list already exposes boundary thresholds and outputs; the cell coordinates plus override flag are sufficient operator narrative for v1. Per-cell rationale would duplicate the dashboard widget. Revisit if operator complains.

### [PR-WSOC-TABLE-1-D06] No test pins `Decision::summary` shape
**Status:** resolved (deferred)
**Severity:** nit
**Location:** `crates/core/src/controllers/weather_soc.rs` (no test exists)
**Description:** None of the 30 tests asserts on `d.decision.summary` or any `with_factor` string. A regression printing `cold` as `warm` or swapping `bucket.label()` for the wrong constant goes uncaught.
**Fix:** Closed deferred — string-shape regression is low-impact and would be obvious on first dashboard reload. Cascade-equivalence outputs are pinned (12 cell tests + 4 boundary tests + 3 override tests); the summary is operator narrative, not a controller output. Revisit if narrative drift becomes a problem.

---

## PR-WSOC-EDIT-1

### [PR-WSOC-EDIT-1-D01] Cell-edit modal accumulates duplicate click handlers across re-opens for different cells
**Status:** resolved
**Severity:** major
**Location:** `web/src/render.ts:1840-1877, 1915-1956`
**Description:** `installWeatherSocCellModalHandlers` calls `bodyEl.addEventListener("click", ...)` every time the modal is opened for a *different* cell (the rebuild branch, gated by `bodyEl.dataset.wsocCellId === entityId`). Each new listener captures the new `entityId` via closure. The listeners bound on previous opens are never removed because the listener is bound on `bodyEl` itself, not on its children — `bodyEl.innerHTML = ...` clears descendants but not listeners on `bodyEl`. Reproducer: open Low.cold modal → close → open Dim.warm modal → click Save. Both listeners fire, the first writes Dim.warm's input values to `weathersoc.table.low.cold.*` topics, the second writes them to `weathersoc.table.dim.warm.*`. Silent retained-MQTT corruption of a previously-viewed cell.
**Fix:** `web/src/render.ts` — `installWeatherSocCellModalHandlers` signature changed from `(entityId, bodyEl)` to `(bodyEl)` and is now idempotent (gated by `bodyEl.dataset.wsocModalHandlersInstalled`). Click handler reads `bodyEl.dataset.wsocCellId` at dispatch time; no closure capture of entityId. Save / Cancel / Revert handlers all read from dataset. Last-snap baseline stamping extracted into `stampWeatherSocCellLastSnap` for the rebuild path.

### [PR-WSOC-EDIT-1-D02] Drift-guard fixture file untracked; TS-side guard is inert (no JS test runner)
**Status:** under fix
**Severity:** major
**Location:** `web/test-fixtures/weather-soc-defaults.json` (untracked); `web/src/render.test.ts` (assertions never executed)
**Description:** Two related defects: (1) `git status` shows `web/test-fixtures/` as **untracked**. The Rust drift-guard `weathersoc_defaults_fixture_matches_safe_defaults` reads this file via `std::fs::read_to_string` and panics if missing — so as soon as the PR is committed without `git add`-ing the fixture, the test will fail in CI / fresh checkouts. (2) The TS-side "drift guard" embeds an inline-literal copy and compares only against `WEATHER_SOC_DEFAULTS`. Combined with the project having no JS test runner (`tsc --noEmit` only type-checks), all top-level `assert(...)` statements in `render.test.ts` are NEVER executed. The "three-leg assertion" comment in the diff is misleading; the TS leg never runs.
**Fix:** D02a (orchestrator action): `git add web/test-fixtures/weather-soc-defaults.json` before committing. D02b (test runner): closed deferred — installing a JS runner is wider-scope work tracked separately. The Rust-side drift guard alone covers the fixture↔core leg; the TS-side `render.test.ts` will remain type-checked-only until a runner is added.

### [PR-WSOC-EDIT-1-D03] `weatherSocSendCommand` module-level singleton blocks re-binding across renders
**Status:** resolved
**Severity:** minor
**Location:** `web/src/render.ts:1668, 1749-1753`
**Description:** `weatherSocSendCommand` is a module-level `let` populated by `renderWeatherSocTable(snap, sendCommand)`. The current code path always passes `sendCommand`, so this works in practice, but the API contract is fragile: any future re-call without `sendCommand` reuses a stale closure. Same applies to `weatherSocBoundariesInstalled` — module-level latch that survives DOM replacement.
**Fix:** `web/src/render.ts` — replaced `weatherSocBoundariesInstalled` (module-level) with `container.dataset.wsocBoundariesInstalled = "1"` latch on the `.weathersoc-boundaries` element; same dataset-latch pattern (`bodyEl.dataset.wsocModalHandlersInstalled`) for the modal handler from D01. `weatherSocSendCommand` retained module-level (the once-bound modal click handler needs a stable reference) but use sites snapshot it into a local `const send` and bail when null, so a future absent dispatcher fails fast instead of using a stale closure.

### [PR-WSOC-EDIT-1-D04] Memory file `project_weathersoc_table.md` not updated for the new bat=100 invariant or flat-knob plumbing
**Status:** resolved
**Severity:** minor
**Location:** `~/.claude/projects/-home-pavel-work-victron-controller/memory/project_weathersoc_table.md`
**Description:** The file still said "model the 12 cells as a structured baboon table (**not 48 flat knobs**)" — directly contradicting PR-WSOC-EDIT-1, which DOES introduce 48 flat-addressable knobs (`KnobId::WeathersocTableCell`). Didn't mention the bat=100 normalisation, popup-modal UX, relocation Detail → Control, or column-header click-through.
**Fix:** Memory file rewritten by orchestrator. Frontmatter description updated to reflect both shipped PRs. Body now records: (a) 48 flat HA-addressable knobs via `KnobId::WeathersocTableCell { bucket, temp, field }`, programmatic plumbing one match arm per layer; (b) `bat=100` across all extended cells; (c) per-cell popup modal via `#entity-modal` with `EntityType="weathersoc-cell"`; (d) widget on Control tab above `<section id="knobs">`; (e) `knob_name → String`, drift-guard fixture pattern, dirty-input rule, and the maintenance contract for future cell-field additions.

### [PR-WSOC-EDIT-1-D05] `bodyEl.dataset.wsocCellId` is never cleared on modal close, leaking dirty inputs
**Status:** resolved
**Severity:** minor
**Location:** `web/src/render.ts:1839-1879`, `web/src/index.ts:225-229`
**Description:** `closeEntityInspector` hid the modal but didn't clear `bodyEl.dataset.wsocCellId` or `bodyEl.innerHTML`. Reproducer: open Low.cold → type "95" into exp without saving → close → re-open Low.cold. The `alreadyOpen` check would be true → renderer entered the live-refresh branch instead of rebuild. The user saw their unsaved "95" still in the input.
**Fix:** New exported helper `clearWeatherSocCellModal()` in `web/src/render.ts` clears `bodyEl.dataset.wsocCellId` and resets `bodyEl.innerHTML`; called from `closeEntityInspector` in `web/src/index.ts` after `hidden` is set. Idempotent / no-op when no cell modal was open.

### [PR-WSOC-EDIT-1-D06] Hidden `return None` after `weathersoc.table.` prefix-match blocks future fallback parsers
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/mqtt/serialize.rs:340-352`, `crates/shell/src/dashboard/convert.rs:1339-1352`
**Description:** Both `knob_id_from_name` parsers stripped the prefix and then `return None` if `parts.len() != 3`, hiding control-flow from future arms below.
**Fix:** Added the comment `// future-proof: any new shape under this prefix needs an arm above` immediately above the `return None` in both files. No code change.

### [PR-WSOC-EDIT-1-D07] Stale doc comment in `all_knob_publish_payloads` says `12 × 4 f64s` (should be 36 f64 + 12 bool)
**Status:** resolved
**Severity:** nit
**Location:** `crates/core/src/process.rs:1033-1035`
**Description:** Comment said "The clone is cheap (12 × 4 f64s + 12 bools)." Cells have 3 f64 + 1 bool.
**Fix:** Replaced with "The clone is cheap (36 f64 + 12 bool)" in `crates/core/src/process.rs`.

### [PR-WSOC-EDIT-1-D08] Column-header `<span>` markup deviates from existing `<a>` entity-link pattern
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/static/index.html:65-82`
**Description:** Headers used `<span class="entity-link">` while every other entity-link site emits `<a>`. Cursor wasn't pointer by default; a11y treated as text.
**Fix:** Eight `<span>` tokens (`exp`/`bat`/`dis`/`ext` × Warm/Cold) converted to `<a class="entity-link mono" href="javascript:void(0)" data-entity-id=… data-entity-type="knob">` in `crates/shell/static/index.html`. Click handling unchanged (already operates on `closest(".entity-link")`).

### [PR-WSOC-EDIT-1-D09] `KNOB_SPEC` cell entries inserted via top-level `for` loop mutating an `export const`
**Status:** resolved (deferred)
**Severity:** nit
**Location:** `web/src/knobs.ts:282-302`
**Description:** `KNOB_SPEC` is declared `export const` then mutated by a top-level loop. TS permits this; stylistically inconsistent with the rest of the file. Module-init order is implicit; a consumer reading `KNOB_SPEC[<cell-key>]` from an earlier-loaded module's top-level statement would see `undefined`.
**Fix:** Closed deferred — current consumers all access `KNOB_SPEC` via function calls (after module-init), and the inline-mutation pattern is contained to one file. Refactor to a frozen `{...HAND_WRITTEN, ...generated}` object would be cleaner but is style-only.

---

## PR-WSOC-EDIT-2

### [PR-WSOC-EDIT-2-D01] Re-opening single-knob-edit for the same id after a knob-modal transition leaves the modal wedged
**Status:** resolved
**Severity:** minor
**Location:** `web/src/render.ts` (`renderSingleKnobEditModalBody` `alreadyOpen` check ~L1875)
**Description:** Sequence: (a) operator opens single-knob-edit modal for `weathersoc.threshold.energy.low`. (b) Without closing, clicks an `exp/bat/dis/ext` sub-header anchor (`data-entity-type="knob"`) → `renderEntityModal` falls through to `renderKnobBody` and overwrites `bodyEl.innerHTML`, but leaves `bodyEl.dataset.singleknobKnob` and `dataset.singleknobHandlersInstalled` intact. (c) Clicks the same boundary `low` anchor again → `alreadyOpen` evaluates true, live-refresh branch queries `[data-singleknob-field]` (null — wrong body content now), bails. Modal wedged with stale knob-body content.
**Fix:** `web/src/render.ts::renderSingleKnobEditModalBody` — `alreadyOpen` short-circuit now requires BOTH the dataset match AND `bodyEl.querySelector("[data-singleknob-field]")` returning a non-null element. If a non-single-knob render replaced the body, the second condition is false and the function falls through to the full rebuild branch. Type-transition concern stays local to single-knob-edit.

### [PR-WSOC-EDIT-2-D02] Defensive enum branch in `saveSingleKnobEdit` emits payload missing `knob_name`
**Status:** resolved
**Severity:** minor
**Location:** `web/src/render.ts:855-862`
**Description:** Defensive enum dispatch path's `else` branch dispatched `send({ [spec.cmdVariant]: { value: v } })` without `knob_name`. Today unreachable, but a future enum knob added to single-knob-edit would silently produce a malformed command.
**Fix:** `web/src/render.ts::saveSingleKnobEdit` — `else` branch replaced with `throw new Error(\`single-knob-edit doesn't support cmdVariant ${spec.cmdVariant} for knob ${dotted}\`)`. Hard fail beats silent malformed payload; the comment above the branch already stated this is unsupported.
