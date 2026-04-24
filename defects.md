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
**Status:** open
**Severity:** major
**Location:** `crates/core/src/process.rs:860`
**Description:** SPEC §3.5 + dashboard Decision all show the night auto-stop rule. But `run_zappi_mode` feeds literal `0.0` into the controller. The real `che` kWh is parsed by the myenergi poller (`types.rs:30`) and dropped. For users setting `zappi_limit ≤ 65`, the car charges until the tariff window closes regardless — hours of unnecessary grid pull.
**Suggested fix:** Plumb `session_kwh` from `ZappiObservation` through `TypedReading::Zappi` / `ZappiState` into `run_zappi_mode`. Compute `session_charged_pct` from a user knob (see A-14 for the unit bug).

### [A-14] `zappi_limit` documented as % but legacy semantic was kWh — wrong comparison unit
**Status:** open
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
**Status:** open
**Severity:** minor
**Location:** `crates/shell/src/dbus/writer.rs:86-104`
**Description:** Soc field is f64 in our code but some Venus firmwares expect i32; silent "Wrong type" errors that TASS re-proposes every tick. Worse with partial schedule (half fields written, half rejected).
**Suggested fix:** `GetProperties` at connect to cache variant signature per path; or submit best-guess then fall back. Stop retrying after N failures; raise kill switch.

### [A-30] Event channel `mpsc::channel(256)` has no watermark; stale-batched events stamped `Fresh`
**Status:** open
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
**Status:** open
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

### [A-46] Evening discharge + `allow_battery_to_car=true` can net-import at peak tariff if Zappi draws > export_cap
**Status:** open
**Severity:** minor
**Location:** `crates/core/src/controllers/setpoint.rs:224-244, 245-345`
**Description:** Zappi-clamp branch is bypassed by design (SPEC §5.9). `-export_power` is capped at `-grid_export_limit_w` only; nothing prevents a positive net import when Zappi draw exceeds PV + export cap. User opted in — money risk only.
**Suggested fix:** Extra clamp: `setpoint_target.min(-zappi_current * grid_voltage)`. Or disable evening-discharge branch whenever `grid_power > small_margin` (already importing).

### [A-47] `check_c4` `i32 - i32` can overflow (see A-31, duplicate)
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
**Status:** open
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
**Status:** open
**Severity:** minor
**Location:** `crates/shell/src/dbus/writer.rs:28-37, 86-104`
**Description:** Startup-only `Connection::system()`. Venus D-Bus restart → every write fails → TASS stuck in Commanded; MultiPlus retains old value. Fail-closed for device state, fail-open for our narrative.
**Suggested fix:** Periodic health check + reconnect with backoff. Publish `ActuatedPhase{Unset}` for every target when disconnected.

### [A-57] Schedules: 5 separate writes not atomic; partial writes leave inconsistent schedule on bus
**Status:** open
**Severity:** minor
**Location:** `crates/core/src/process.rs:806-841`, `crates/shell/src/dbus/writer.rs:39-55`
**Description:** If Start/Duration succeed and Soc fails, Venus runs the new window with the old SoC target. TASS readback doesn't converge; dashboard shows Commanded forever.
**Suggested fix:** Serialise the 5-write burst in the writer; on any failure, reset `target = unset` so TASS re-proposes. Treat the burst as atomic at the controller layer.

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
**Status:** open
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
**Status:** open
**Severity:** minor
**Location:** `crates/shell/src/dbus/writer.rs:86-104`
**Description:** Venus 3.60 variance; silent "Wrong type" errors that get retried every tick. Dup of A-29 sub-aspect.
**Suggested fix:** See A-29.

### [A-66] `Value::Bool(false)` as extract-scalar arm (see A-02, duplicate)
*(Duplicate of A-02.)*

### [A-67] `allow_battery_to_car` boot-reset depends on MQTT bootstrap completing
**Status:** resolved
**Severity:** nit
**Location:** `crates/shell/src/mqtt/mod.rs:223-235`
**Description:** SPEC §5.9 says "always boots false regardless of retained". Code relies on bootstrap path to send the reset; if MQTT is disabled entirely, `safe_defaults` handles it anyway — but the mechanism is less robust than the SPEC suggests.
**Suggested fix:** Document the dependency; guarantee reset by calling `apply_knob(AllowBatteryToCar, false)` unconditionally at process start.

### [A-68] `TlsConfiguration::Simple` accepts malformed CA bytes without parse-time validation
**Status:** open
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
**Status:** open (deferred)
**Severity:** minor
**Location:** `crates/shell/src/main.rs:~54-88`
**Description:** 5 s watermark polling can miss a bootstrap burst that fills and drains inside the window. A future deploy with 10k retained topics would stall silently again because the 4096 cap is reached faster than the watermark samples.
**Suggested fix:** Log peak water-level on first drain below 50 %. Or add an explicit bootstrap-completion log including "applied N events, channel cap M". Deferred — low probability, current fix already covers observed floor × 10.

### [PR-URGENT-13-D04] Watermark warn lacks trend direction; operators can't tell climb vs drain from one log line
**Status:** open (deferred)
**Severity:** minor
**Location:** `crates/shell/src/main.rs:~78-82`
**Description:** `warn!("event channel > 75% full ({in_use}/{max})")` — single scalar. Can't infer whether queue is climbing (→ imminent stall) or draining.
**Suggested fix:** Track `last_in_use` between ticks; include `delta` in the warn. Deferred — not blocking.

### [PR-URGENT-13-D05] Escalation `error!` has no throttle after recovery + re-flap
**Status:** open (deferred)
**Severity:** minor
**Location:** `crates/shell/src/dbus/subscriber.rs` (escalation arm at `count == 5`)
**Description:** A flapping service at ~5-tick cadence emits one `error!` per cycle. Correct behaviour but busy log.
**Suggested fix:** Throttle escalation to once per 5 min per service; log "recovered" INFO on Ok transition to make pairing explicit. Deferred.

### [PR-URGENT-13-D06] No unit test for rate-limiter / escalation state machine
**Status:** open (deferred)
**Severity:** nit
**Location:** `crates/shell/src/dbus/subscriber.rs`
**Description:** Executor acknowledged the omission. For a safety-critical diagnostic fix, behavioural test is warranted.
**Suggested fix:** Extract the state (counts + last_warn) into a standalone struct; table-driven test over a sequence of tick results. Deferred; promote to M-AUDIT-2 if the state machine ever grows.

### [PR-URGENT-13-D07] `error!` message interpolates a `const` via tracing's captured-identifier mechanism
**Status:** open (deferred)
**Severity:** nit
**Location:** `crates/shell/src/dbus/subscriber.rs:~388`
**Description:** `"periodic GetItems failing for {RESEED_ESCALATE_AFTER}+ ..."`. Works on current Rust/tracing; a structured field (`threshold = RESEED_ESCALATE_AFTER`) is more grep-friendly.
**Suggested fix:** `error!(service = %svc, threshold = RESEED_ESCALATE_AFTER, "periodic GetItems failing for N+ consecutive ticks; …")`. Deferred — not blocking.

### [PR-URGENT-13-D08] Heartbeat arm is NOT starvation-proof from a blocking poll-arm body; comment overstates the guarantee
**Status:** open (deferred)
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
**Status:** open
**Severity:** minor
**Location:** `crates/shell/src/dbus/subscriber.rs` tests module
**Description:** End-to-end path (D-Bus signal → `ItemEntry` → `extract_scalar` → `route_to_event` → `Event::Sensor` → core `process` → effects) not covered. A future refactor could route Bool through a new arm and this unit test wouldn't catch it.
**Suggested fix:** DEFERRED to M-AUDIT-2 as a standalone testing hardening item. Out of scope for PR-01's surgical fix.

### [PR-01-D03] Fix suppresses the event silently; no counter / log of dropped non-finite readings
**Status:** open
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
**Status:** open
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
**Status:** open (deferred)
**Severity:** nit
**Note:** Rendered moot if the user picks option (a) on the grid_voltage design question (drop voltage tracking entirely; use 230 V constant + direct grid_current sensor).

### [PR-02-D08] `MAX_SENSIBLE_GRID_V = 260.0` doc comment says "EN 50160 caps at +10% (253 V)" — code/comment mismatch
**Status:** open
**Severity:** nit
**Location:** `crates/core/src/controllers/current_limit.rs:~39`
**Description:** Comment cites 253 V; code uses 260 V. Either update comment to explain why 260 (headroom above EN 50160 for benign surges) or tighten the constant to 253.
**Suggested fix:** Update docstring: "EN 50160 caps legitimate readings at +10% of nominal (253 V); we add 7 V of headroom to avoid false fallback on benign surges".

### [PR-02-D09] Test `current_limit_grid_v_fallback_just_below_threshold` is 28 V below the new 207 V floor
**Status:** open
**Severity:** nit
**Location:** `crates/core/src/controllers/current_limit.rs:~683`
**Description:** Test named "just below threshold" uses 179 V; after PR-02's floor raise to 207, 179 is "well below". Name is stale.
**Suggested fix:** Either rename to `_well_below_threshold` or add a 206.9 V "just below" companion. Not blocking.

---

## PR-09a — Review round 1 (executor: `a183ad782e39e74a6`, reviewer: `a5a1d3eef8d38c125`)

**Note on scope**: the reviewer sees the full uncommitted working-tree state and reports scope-sprawl (D06/D07). The cause is accumulated pre-review-loop changes (VebusOutputCurrent removal, ChargeBatteryExtendedMode knob, weather_soc decision honesty, sensors_meta, dashboard DOM refactor, MQTT hostname fix, `writes_enabled` cold-start flip) that were never committed. PR-09a's own patch is small and correct; the "sprawl" findings are artifacts of a dirty baseline, not regressions introduced by this PR. Listed below for completeness but marked accordingly.

### [PR-09a-D01] `apply_setpoint_safety` path does not publish a `grid_setpoint` Decision
**Status:** open (deferred)
**Severity:** minor
**Location:** `crates/core/src/process.rs:~438-440, ~496-511`
**Description:** On freshness-fail the safety branch proposes 10 W without setting `world.decisions.grid_setpoint`. Pre-existing gap (not a regression). Dashboard shows `None` for grid_setpoint Decision until a Fresh tick arrives.
**Suggested fix:** Add a Decision in `apply_setpoint_safety` ("Safety 10 W — required sensors not fresh") with factors listing which sensor failed the freshness gate. Deferred pending PR-05 (observer→live invariant) which will touch the same branch.

### [PR-09a-D02] Three clamp factors always emitted, even when clamp didn't alter the value
**Status:** open
**Severity:** minor
**Location:** `crates/core/src/process.rs:~475-481`
**Description:** `pre_clamp_setpoint_W`, `clamp_bounds_W`, `post_clamp_setpoint_W` added unconditionally. Common case `pre == post`; three noise rows per tick. PR-02 pattern emits its `grid_v_fallback` factor only when fallback fires.
**Suggested fix:** Emit only when `pre_clamp != capped`. Or collapse into a single factor `clamp = "X W → Y W (bounds [-E, +I])"` — one row, self-describing.

### [PR-09a-D03] `setpoint_clamps_to_export_cap` test is not a regression test; redundant with existing
**Status:** open (deferred)
**Severity:** nit
**Location:** `crates/core/src/process.rs:~1848-1866`
**Description:** Asserts post-PR behaviour, not pre-PR. Existing `grid_export_cap_is_absolute_for_setpoint_target` already covers the invariant.
**Suggested fix:** Delete as redundant, or convert to a property test (pre-clamp arbitrary negative → post-clamp ≥ -export_cap).

### [PR-09a-D04] `setpoint_decision_has_pre_and_post_clamp_factors` verifies factor names only, not values
**Status:** open
**Severity:** minor
**Location:** `crates/core/src/process.rs:~1868-1890`
**Description:** Test checks factor presence, not whether `pre_clamp_setpoint_W == out.setpoint_target (pre-clamp)` or `post_clamp_setpoint_W == world.grid_setpoint.target.value`. Factor correctness is not defended.
**Suggested fix:** Add value-level assertions: set `grid_import_limit_w=7`, `grid_export_limit_w=3000`, `force_disable_export=true`; assert the three factor values match the expected "10", "[-3000, +7]", "7".

### [PR-09a-D05] SPEC §7 row for `grid_import_limit_w` is flavorless
**Status:** open
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
**Status:** open (deferred to PR-09b)
**Severity:** nit
**Location:** `crates/shell/src/dashboard/convert.rs:~418`
**Description:** Clones the A-34 pattern rather than avoiding it. Addressed together in PR-09b.
**Suggested fix:** PR-09b: `i32::try_from(k.grid_import_limit_w).unwrap_or(i32::MAX)`, same pattern as A-34's fix for the export side.

### [PR-09a-D09] No test for `grid_import_limit_w = 0` edge case
**Status:** open (deferred to PR-09b)
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
**Status:** open (deferred)
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
**Status:** open (deferred)
**Severity:** nit
**Location:** `crates/shell/src/mqtt/serialize.rs:787-821`
**Description:** Only min-1/max+1 reject cases tested; no min-exact / max-exact accept cases. An off-by-one (`>` vs `>=`) would not be caught.
**Suggested fix:** Add boundary-accept per range: `ExportSocThreshold=0`/`100`, `ZappiCurrentTarget=6.0`/`32.0`, `EddiEnableSoc=50`, `GridExportLimitW=10000`.

### [PR-06-D05] Executor miscounted test cases (22 vs actual 23)
**Status:** open (deferred)
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
**Status:** open
**Severity:** major (architectural)
**Location:** `crates/core/src/process.rs` (`compute_derived_view`, `run_setpoint`, `run_current_limit`), `crates/core/src/controllers/zappi_active.rs`, plus any similar ad-hoc bookkeeping field read by > 1 core.
**Description:** PR-04 resolved the immediate A-05 hazard by extracting `classify_zappi_active` into a shared free function consumed by both `compute_derived_view` (fed into `run_setpoint`) and `run_current_limit`. That lifts the correctness symptom but not the underlying shape: two cores still independently call a third-party function and trust that both will stay in sync. Any future derived value read by > 1 core reintroduces the same drift risk. The correct shape per the TASS discipline is: the derived value is its own TASS core (a "derivation core") whose output is stored in world state; dependent cores declare a `depends_on` edge and the orchestrator walks cores in topological order. The DAG is built once at registry construction and validated for cycles + missing deps at startup (not runtime — a static registry check).
**Root cause:** The core registry is currently implicit in `process()`'s hard-coded call order (`run_schedules` → `run_weather_soc` → `run_current_limit` → `run_setpoint` → …). Dependencies between cores are implied by read/write patterns on `world.bookkeeping`; there is no registry that records them, so neither the compiler nor a test can catch a misordering. The `DerivedView` helper was a pragmatic, localized workaround, not the right primitive.
**Suggested fix:** Introduce a `Core` trait with `fn depends_on(&self) -> &'static [CoreId]` and `fn run(&self, &mut World, &dyn Clock, &mut Vec<Effect>)`. Register all cores (including new derivation cores like `ZappiActiveCore`) in a single `CoreRegistry`; topologically sort at construction; panic on cycles or missing deps. `process()` walks the sorted vector. Migrate `classify_zappi_active` to `ZappiActiveCore` that writes to a dedicated `world.derived.zappi_active` (not `bookkeeping`, which is user-facing retained state). Audit other shared bookkeeping fields — `battery_selected_soc_target`, `charge_to_full_required`, `charge_battery_extended_today` — and lift any read-by-multiple-cores field into its own derivation core.

---

## PR-SCHED0 — schedule_0 observed disabled post-df3ae4d

### [PR-SCHED0-D01] `schedule_0` appears disabled on the dashboard / inverter despite `evaluate_schedules` unconditionally emitting `days=DAYS_ENABLED`
**Status:** under fix
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
**Status:** open
**Severity:** nit (informational)
**Location:** `crates/core/src/core_dag/cores.rs:~51, ~71` — `run_setpoint(world, *derived, ...)` / `run_current_limit(world, *derived, ...)`.
**Description:** Underlying `run_setpoint` / `run_current_limit` take `DerivedView` by value. Dereferencing `&DerivedView` works because `DerivedView: Copy`. If a future change adds a non-Copy field (e.g., PR-DAG-B introduces a Vec inside a tick-scratch struct), these lines silently become clones or compile-errors.
**Suggested fix:** Change `run_setpoint` / `run_current_limit` signatures to accept `&DerivedView`. Deferable — PR-DAG-B deletes `DerivedView` wholesale and replaces with `world.derived.zappi_active`, so the smell will resolve itself.

### [PR-DAG-A-R2-I02] D02 test's inline comment is imprecise about which `naive()` call the classifier consumes
**Status:** open
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
**Status:** open
**Severity:** nit
**Location:** `crates/core/src/core_dag/tests.rs` — `setpoint_decision_matches_world_derived_zappi_active_across_boundary`.
**Description:** The test compares setpoint's decision factor against `world.derived.zappi_active` at tick end. It asserts consistency — but nothing proves the classifier was only called ONCE per tick. A regression where a future actuator core calls `classify_zappi_active(world, clock)` locally (a la PR-04's `DerivedView`) would still produce a matching factor most of the time and pass the test on most clock fixtures.
**Suggested fix:** Add a call-counting clock wrapper (increment a `Cell<u32>` on every `naive()` call). Assert that across a tick, the counter reflects only the expected call sites (apply_tick + ZappiActiveCore classify + whatever `run_*` call `clock.naive()` for their own reasons). Deferable — can fold into a broader "tick-budget" invariant when useful.

### [PR-DAG-A-R2-I03] Lazy `OnceLock` registry builds on first call, not startup — lost startup-time validation if `production_cores()` ever becomes data-dependent
**Status:** open
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
