# M-ZAPPI-DRAIN — compensated-drain feedback loop replacing the PV-only Zappi clamp

**Milestone:** M-ZAPPI-DRAIN
**Status:** planned → ready to execute
**Driving feedback (user, 2026-04-29):** the hard "limit grid export to current
solar production when zappi is active" rule (`crates/core/src/controllers/setpoint.rs:617–637`)
is brittle: it pessimises export on cloudy days, ignores the operator's
intentional grid-side loads (heat pump, cooker), and the early-morning Soltaro
sub-branch is a quirk rather than a behaviour. Replace with a closed-loop
controller using **compensated battery drain** as the feedback signal.

**Orchestrator-locked decisions** (from plan-doc open questions, locked by
review-loop orchestrator on 2026-04-29):

- **MPPT op-mode index orientation** follows existing power-sensor numbering:
  `Mppt0OperationMode` ↔ `mppt_0` = `ttyUSB1` (DI 289);
  `Mppt1OperationMode` ↔ `mppt_1` = `ttyS2` (DI 274).
  This keeps op-mode `_0`/`_1` aligned with `MpptPower0`/`MpptPower1` so the
  dashboard surface is consistent.
- **`zappi_battery_drain_hard_clamp_w` default = 200 W.** Tight enough that
  Fast mode never silently bleeds the battery; loose enough to absorb
  sub-second EV transients.
- **`zappi_battery_drain_target_w` routes through `KnobValue::Float`.**
  Additive within existing types — no new wire-format variant.
- **Hard clamp runs before the existing grid_export/import cap** in
  `run_setpoint`. Means a runaway raise gets clipped to `+grid_import_limit_w`,
  which is the intended belt-and-suspenders.
- **`compute_battery_balance::PreserveForZappi`** branch retained as
  approximation; SoC-chart parity with the loop is a follow-up, not
  M-ZAPPI-DRAIN scope.

---

## 1. Goal

When the Zappi is actively pulling power AND `allow_battery_to_car == false`,
the controller's job is "don't let the battery be drained for the EV". The new
feedback variable is

    compensated_drain = max(0, -battery_dc_power - heat_pump_w - cooker_w)

— battery discharge that is NOT explained by the two metered grid-side
loads the operator excluded on purpose. Stale HP / cooker readings are
treated as `0 W` (conservative — clamps tighter, never looser). The
controller raises (less negative) the proposed setpoint when
`compensated_drain > threshold_w` (default 1000) and slowly relaxes it
toward `-solar_export` when drain is below threshold. The legacy `(2..8)`
Soltaro-only branch is **folded into the unified loop** (Soltaro AC export
naturally registers in the battery power balance, so the loop handles
early-morning surplus without a special branch). The 23:55–00:00 Soltaro
protection window stays as-is. A separate **Fast-mode-only hard clamp**
runs after `evaluate_setpoint()` returns: if `zappi_mode == Fast &&
!allow_battery_to_car && observed_drain > hard_clamp_w (default 200)`,
raise the proposed setpoint by the excess drain.

Locked numerically (do not relitigate):

| Knob | Default | Type |
|---|---|---|
| `zappi_battery_drain_threshold_w` | `1000` | `u32` |
| `zappi_battery_drain_relax_step_w` | `100` | `u32` |
| `zappi_battery_drain_kp` | `1.0` | `f64` |
| `zappi_battery_drain_target_w` | `0` | `i32` (reserved for future PI extension; routes via `KnobValue::Float`) |
| `zappi_battery_drain_hard_clamp_w` | `200` | `u32` |

All five knobs are `category = "config"` (install-time tuning, not
day-to-day operator).

---

## 2. Out-of-scope

- **MPPT op-mode is observability only.** No control-loop coupling. The
  two new sensors land on the dashboard with human-readable strings ("Off",
  "V/I-limited", "MPPT-tracking") and are otherwise inert. A future PR may
  use them to colour the SoC chart or annotate the forecast view; not in
  this milestone.
- **Forecast-derived export ceilings.** No interaction with the
  weathersoc-driven export thresholds — the loop runs only when the
  Zappi is active and the existing setpoint controller's other branches
  (force_disable_export / 23:55 protection / evening discharge / daytime
  PV-multiplier / boost / extended-night) take precedence in their own
  windows.
- **`zappi_battery_drain_target_w` is exposed but inert.** The math uses
  `threshold_w` as the reference. Knob exists to let a future PI
  extension read it without another wire bump.
- **Soltaro export-during-day branch redesign.** The new soft loop
  consumes Soltaro power via the battery-balance signal, so the
  `(2..8)` carve-out is removed; but the daytime PV-multiplier
  (`(8..17)`) branch in `evaluate_setpoint` is unchanged — that runs
  outside the Zappi-active gate.
- **Knob category demotion.** All five new knobs are `"config"`; we
  don't promote any to operator-table even if field-tuning ergonomics
  would benefit. Operators reach them via `[knobs]` config or the HA
  inspector.
- **Pinned-register coverage of `/MppOperationMode`.** Out of scope —
  the field is read-only on Victron's side anyway; pinned-registers is
  for write enforcement.
- **`compute_battery_balance` projection parity** with the new loop
  dynamics. Branch retained as approximation; chart-parity is a
  follow-up, not part of this milestone.

---

## 3. PR breakdown

5 PRs, sequenced. PR-ZD-1 + PR-ZD-2 are pure plumbing (no behaviour
change); PR-ZD-3 + PR-ZD-4 ship the new control law; PR-ZD-5 is
frontend-only.

### 3.1 PR-ZD-1 — Sensors

#### Scope
Wire four new sensors through the full sensor pipeline:

- `HeatPumpPower` — zigbee2mqtt JSON push, topic `zigbee2mqtt/nodon-mtr-heat-pump`, JSON `.power` field (W). Availability `zigbee2mqtt/nodon-mtr-heat-pump/availability` payload `online`/`offline` per zigbee2mqtt convention.
- `CookerPower` — same shape, topic `zigbee2mqtt/nodon-mtr-stove`, JSON `.power`. Availability `zigbee2mqtt/nodon-mtr-stove/availability`.
- `Mppt0OperationMode` — D-Bus, `com.victronenergy.solarcharger.ttyUSB1` (DeviceInstance 289), path `/MppOperationMode`. Enum: 0=Off, 1=Voltage-or-current-limited, 2=MPPT-tracking. Aligned with existing `MpptPower0` ↔ `mppt_0` ↔ `ttyUSB1`.
- `Mppt1OperationMode` — D-Bus, `com.victronenergy.solarcharger.ttyS2` (DeviceInstance 274), path `/MppOperationMode`. Aligned with `MpptPower1` ↔ `mppt_1` ↔ `ttyS2`.

No control-loop coupling in this PR — the four sensors land in `world.sensors`,
flow through `apply_sensor_reading`, decay via `apply_tick`, and surface on the
dashboard sensor table. Op-mode sensors render on the dashboard as numeric
codes; PR-ZD-5 maps them to human-readable strings.

#### Files touched

Core (`crates/core/src/`):
- `types.rs` — add 4 variants to `SensorId` enum, add 4 entries to `SensorId::ALL`, add 4 arms to `freshness_threshold` (HP/cooker `30 s`; MPPT op-modes `30 s`), 4 arms to `regime()` (HP/cooker `SlowSignalled`; MPPT op-modes `ReseedDriven`), 4 arms to `reseed_cadence()` (15 s for all four), 4 `None` arms in `actuated_id()`.
- `world.rs` — add 4 `Actual<f64>` slots to `Sensors`, init in `Sensors::unknown`, route in `Sensors::by_id`.
- `process.rs` — `apply_sensor_reading` — 4 new arms; `apply_tick` — 4 new `tick(at, freshness_threshold())` calls.

Models (`models/`):
- `dashboard.baboon` — extend `data Sensors` with `heat_pump_power: ActualF64`, `cooker_power: ActualF64`, `mppt_0_operation_mode: ActualF64`, `mppt_1_operation_mode: ActualF64`. Per CLAUDE.md "Deployment topology": additive within an existing version. Run `scripts/regen-baboon.sh`. Fix compile errors in convert.rs.

Shell — D-Bus (`crates/shell/src/dbus/subscriber.rs`):
- `routing_table()`: add two `Route::Sensor` entries:
  ```
  add(&mut r, &s.mppt_0, "/MppOperationMode", Route::Sensor(Mppt0OperationMode));
  add(&mut r, &s.mppt_1, "/MppOperationMode", Route::Sensor(Mppt1OperationMode));
  ```
  Both share the existing `mppt_0` / `mppt_1` services, so no new `DbusServices` field, no new TimerId, no `service_set` change.

Shell — MQTT (`crates/shell/src/mqtt/`):
- `mod.rs` — extend `Subscriber` with `heat_pump_topic: Option<String>`, `cooker_topic: Option<String>` plus availability topics. Closer to the `matter_outdoor_topic` pattern than the EV-SoC two-stage discovery.
- `mod.rs` — extend the inbound dispatch loop with two new exact-match branches:
  - On the value topic: parse JSON body → extract `.power` → emit `Event::Sensor(SensorReading { id: HeatPumpPower / CookerPower, value, at: Instant::now() })`. Reject non-finite; reject sub-zero (negative power); reject > `MAX_SANITY_W` (30 000 W).
  - On the availability topic: payload `"offline"` → log only; rely on freshness-window expiry, do not push synthetic stale events.
- `mod.rs` — re-subscribe two value + two availability topics on every reconnect.
- `serialize.rs` — add `sensor_name` arms: `solar.mppt.0.mode.operation`, `solar.mppt.1.mode.operation`, `house.heat-pump.power`, `house.cooker.power`.

Shell — config (`crates/shell/src/config.rs`):
- Add to `MqttConfig` (or sibling section, matching matter/EV layout): four optional topic-name fields. Defaults: empty (bridge dormant). `validate_topic` reused.
- `config.example.toml` — add a commented-out `[mqtt]` block showing `zigbee2mqtt/nodon-mtr-heat-pump` and `zigbee2mqtt/nodon-mtr-stove`. Document JSON-`.power` parse expectation.

Shell — dashboard (`crates/shell/src/dashboard/convert.rs`):
- Add four entries to the `ModelSensors { … }` literal.
- `sensors_meta` — add four `MetaContext` rows. HP/cooker get the configured topic as `provenance` (mirror `outdoor_temperature` / `ev_soc`). MPPT op-modes leave `provenance` empty (D-Bus path is implicit).

Web (`web/src/`):
- `displayNames.ts` — extend the sensors block:
  ```
  heat_pump_power: "house.heat-pump.power",
  cooker_power: "house.cooker.power",
  mppt_0_operation_mode: "solar.mppt.0.mode.operation",
  mppt_1_operation_mode: "solar.mppt.1.mode.operation",
  ```
- `descriptions.ts` — four entries (HP/cooker = "AC power draw of the metered grid-side load X (W). Excluded from the compensated-drain feedback signal."; MPPT op-modes = "Operation mode of MPPT charger N: 0=Off, 1=Voltage-or-current-limited, 2=MPPT-tracking. Observability only.").
- The op-mode sensor's human-readable mapping waits for PR-ZD-5; for now the dashboard renders a bare number.

#### Acceptance criteria

- `cargo test --workspace` green; the staleness invariant test
  (`freshness_threshold_invariant_holds_for_every_sensor`) passes for all
  four new variants.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cd web && ./node_modules/.bin/tsc --noEmit -p .` clean.
- `cargo build --target armv7-unknown-linux-gnueabihf --release` green.
- Live test (operator-driven): with HP / cooker drawing power, dashboard sensor table shows fresh values within ~30 s of publication. Both MPPTs in MPPT-tracking → both op-mode rows show `2`.
- Stale-transition test: stop zigbee2mqtt → HP/cooker rows flip to `Stale` within `2 × 15 s = 30 s`.

#### Risks / failure modes

- **JSON-parse silent failure.** zigbee2mqtt's body shape varies per device firmware. Lock parse against a fixture; rate-limit `warn!` to once per 60 s on parse failure (mirror existing matter / EV-SoC pattern).
- **Availability topic ignored.** zigbee2mqtt's `online`/`offline` markers are *informational*; the freshness window already handles disconnects. PR does NOT use availability to flip the sensor to `Stale` immediately — that would couple two information sources where one suffices, and `offline` followed by an immediate `online` can flap. Decision documented in code.
- **MPPT op-mode service-cadence drag.** Adding `/MppOperationMode` to a service that today only carries `/Yield/Power` (5 s reseed) does NOT change the per-service `min(cadence)` (5 s already wins). Confirmed by `compute_service_cadence` inspection.
- **Sensor naming case.** MPPT power sensors use suffix `_0` / `_1`; following that convention for op-mode prevents the dashboard rendering them under different groups.

#### Test plan (≥ 5 new tests)

1. `apply_sensor_reading_heat_pump_power_writes_field` — `Event::Sensor(HeatPumpPower)` → `world.sensors.heat_pump_power.value == Some(v)`. Mirror `apply_sensor_reading_ev_soc_writes_field`.
2. `apply_sensor_reading_cooker_power_writes_field` — symmetric.
3. `apply_sensor_reading_mppt_0_operation_mode_writes_field` — same.
4. `apply_sensor_reading_mppt_1_operation_mode_writes_field` — same.
5. `parse_zigbee2mqtt_power_body_extracts_power_field` — fixture `{"power": 1234.5, "voltage": 230.0}` → `Some(1234.5)`. Negative power → reject (or clamp to 0 — choose one and lock).
6. `parse_zigbee2mqtt_power_body_rejects_non_finite` — `{"power": "Infinity"}` / `{"power": "NaN"}` → `None`.
7. `parse_zigbee2mqtt_power_body_rejects_out_of_range` — `{"power": 99999}` → `None` (above `MAX_SANITY_W`).
8. `freshness_threshold_invariant_holds_for_every_sensor` — already exists; verify passes for the four new variants.
9. `dbus_routing_table_includes_mpp_operation_mode` — extend `routing_table_default_venus_3_70` to assert both `/MppOperationMode` paths route correctly.
10. `dashboard_snapshot_surfaces_new_sensors` — extend the existing `world_to_snapshot_*` test to assert all four rows appear in `WorldSnapshot.sensors`.

---

### 3.2 PR-ZD-2 — Knobs

#### Scope

Add five new knobs through the full 11-step CLAUDE.md registration. Defaults
applied. No control-loop reads them yet — they are inert until PR-ZD-3 lands.

#### Files touched

Step 1 — Baboon model (`models/dashboard.baboon`):
- Extend `data Knobs` with five additive fields:
  ```
  zappi_battery_drain_threshold_w: i32
  zappi_battery_drain_relax_step_w: i32
  zappi_battery_drain_kp: f64
  zappi_battery_drain_target_w: i32
  zappi_battery_drain_hard_clamp_w: i32
  ```
  Run `scripts/regen-baboon.sh`.

Step 2 — Core knobs struct (`crates/core/src/knobs.rs`):
- Add five fields. `kp` is `f64`; `target_w` is `i32`; threshold/relax_step/hard_clamp are `u32`.
- `safe_defaults` — `1000`, `100`, `1.0`, `0`, `200`.
- Tests — extend `safe_defaults_match_spec_*` family with assertions for all five new defaults.

Step 3 — Core enum (`crates/core/src/types.rs`):
- Add `KnobId::ZappiBatteryDrainThresholdW`, `…RelaxStepW`, `…Kp`, `…TargetW`, `…HardClampW`.

Step 4 — Core apply_knob (`crates/core/src/process.rs`):
- Add 5 `(KnobId, KnobValue)` arms: 3 × `Uint32` (threshold, relax_step, hard_clamp); 2 × `Float` (kp, target_w). Document why `target_w` (signed) routes via `Float`: avoids adding `KnobValue::Int32` (would be a wire-format variant change).

Step 5 — Shell MQTT serialize (`crates/shell/src/mqtt/serialize.rs`), four sites:
- `knob_name` — add 5 dotted names: `zappi.battery-drain.threshold-w`, `zappi.battery-drain.relax-step-w`, `zappi.battery-drain.kp`, `zappi.battery-drain.target-w`, `zappi.battery-drain.hard-clamp-w`.
- `knob_id_from_name` — symmetric.
- `knob_range` — threshold `(0, 10000)`; relax_step `(0, 5000)`; kp `(0.0, 50.0)`; target_w `(-5000.0, 5000.0)`; hard_clamp `(0, 10000)`.
- `parse_knob_value` — route 3 to `parse_ranged_u32 → KnobValue::Uint32`; 2 to `parse_ranged_float → KnobValue::Float`.

Step 6 — Shell HA discovery (`crates/shell/src/mqtt/discovery.rs`):
- Add 5 `number_knob` entries. Steps: threshold `50 W`, relax_step `25 W`, kp `0.05`, target `25 W`, hard_clamp `25 W`. Units: `W`, `W`, none, `W`, `W`.

Step 7 — Shell config defaults (`crates/shell/src/config.rs`):
- Add 5 `Option<…>` fields to `KnobsDefaultsConfig`. Add 5 `set!(…)` lines in `apply_to`. `config.example.toml` gets 5 commented-out lines at safe defaults.

Step 8 — Shell dashboard convert (`crates/shell/src/dashboard/convert.rs`):
- `knobs_to_model` — 5 lines mapping `k.zappi_battery_drain_…` to wire fields.
- `knob_id_from_name` — 5 snake_case mappings.

Step 9 — Web display-name table (`web/src/displayNames.ts`):
- 5 entries (snake_case → dotted).

Step 10 — Web KNOB_SPEC (`web/src/knobs.ts`):
- 5 entries, all `category: "config"`, `group: "Zappi compensated drain"` (new group). `kp` is `kind: "float"`; the four `_w` are `kind: "int"` for threshold/relax/hard_clamp; `target_w` `kind: "float"` (matching its wire-form via `KnobValue::Float`).

Step 11 — Web descriptions (`web/src/descriptions.ts`):
- 5 entries explaining each knob's role in the loop.

#### Acceptance criteria

- `cargo test --workspace` green. `safe_defaults_match_spec_*` updated.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cd web && ./node_modules/.bin/tsc --noEmit -p .` clean.
- `cargo build --target armv7-unknown-linux-gnueabihf --release` green.
- HA discovery: after restart, all five new `number` entities appear under `victron-controller-knobs` device.
- `mosquitto_pub -t 'victron-controller/knob/zappi.battery-drain.threshold-w/set' -m 1500` lands on `world.knobs.zappi_battery_drain_threshold_w == 1500`.
- Out-of-range reject: `mosquitto_pub … -m 99999` → dropped with `warn!`; `world.knobs.…` unchanged.

#### Risks / failure modes

- **Silent registration failure.** CLAUDE.md §"Adding a new knob" lists 11 layers, *each* a hard requirement. Reviewer's checklist must enumerate all 11 and confirm each. The `displayNames.ts` step is the easiest to forget — without it the knob falls through to "Other" with no controls.
- **`KnobValue::Int32` absence.** `target_w` is signed but no `Int32` variant exists. Routing through `Float` is additive; controller rounds to nearest W with `.round() as i32`.
- **`safe_defaults_match_spec_*` test growth.** Adding 5 assertions makes the test verbose. Acceptable — it's contract documentation.
- **MQTT topic naming.** Settled on `zappi.battery-drain.*` to group all 5 under one HA device sub-tree. Mechanical change if user prefers `evcharger.battery-drain.*`.

#### Test plan (≥ 4 new tests)

11. `safe_defaults_match_spec_zappi_drain` — assert all 5 fields equal documented defaults.
12. `apply_knob_zappi_battery_drain_*_routes_to_field` — one test per knob (5 total) — `Event::Command(Knob{…})` → `world.knobs.…` updated.
13. `mqtt_knob_name_round_trip_zappi_drain` — for each of the 5 names, `knob_id_from_name(knob_name(id)) == Some(id)`.
14. `discovery_includes_zappi_drain_knobs` — extend `knob_schemas_round_trip` proptest to include all 5 new IDs.

---

### 3.3 PR-ZD-3 — Soft loop (the actual control law)

#### Scope

Replace lines 617–637 in `evaluate_setpoint()` with the compensated-drain
control law. Drop the early-morning Soltaro-only sub-branch (`(2..8)`) —
the unified loop handles it via the battery-power balance. The 23:55–00:00
Soltaro protection window stays as-is.

#### Files touched

Core (`crates/core/src/controllers/setpoint.rs`):
- Extend `SetpointInput` / `SetpointInputGlobals` with new fields:
  - `pub battery_dc_power: f64,` on `SetpointInput`.
  - `pub heat_pump_power: f64,` and `pub cooker_power: f64,` on `SetpointInput`. Caller passes `0.0` when stale.
  - `pub setpoint_target_prev: i32,` on `SetpointInput` — previously commanded grid setpoint, threaded from `world.grid_setpoint.target.value.unwrap_or(idle_setpoint_w)`.
  - `pub zappi_drain_threshold_w: u32,`, `pub zappi_drain_relax_step_w: u32,`, `pub zappi_drain_kp: f64,`, `pub zappi_drain_target_w: i32,` on `SetpointInputGlobals`.
- Lines 617–637: **delete** the entire `else if g.zappi_active && !g.allow_battery_to_car` branch (both the `(2..8)` Soltaro sub-branch and the daytime else). Replace with a unified branch (full code in the plan-execution prompt; mirrors the brief).
- `compute_battery_balance` (the projection helper): when `g.zappi_active && !g.allow_battery_to_car`, set `net_battery_w = 0.0`. Document the projection-vs-live mismatch.

Core (`crates/core/src/process.rs`):
- `build_setpoint_input` — add `battery_dc_power` to the required-fresh check. Pass HP/cooker as `world.sensors.heat_pump_power.value.unwrap_or(0.0)` / `cooker_power.value.unwrap_or(0.0)` (treating Stale or Unknown as `0`). Pass the four new knobs. Pass `setpoint_target_prev = world.grid_setpoint.target.value.unwrap_or(idle_setpoint_w)`.
- `missing_required_setpoint_sensors` — add `battery_dc_power` to the required list when missing.

#### Acceptance criteria

- `cargo test --workspace` green.
- The `evaluate_setpoint` snapshot tests covering the deleted Zappi branch are deleted, replaced by the new test set.
- Live test: with Zappi Fast and `allow_battery_to_car=false`, battery genuinely draining at 2 kW → dashboard shows `compensated_drain_W ≈ 2000`, decision summary "tightening", setpoint moves up by ~1000 W per tick (`kp=1.0` × `(2000-1000)`).
- Live test: Zappi Fast, battery flat (drain 0), HP at 1 kW, cooker at 500 W → `compensated_drain = max(0, 0 - 1000 - 500) = 0`; loop relaxes by 100 W per tick toward `-solar_export`.

#### Risks / failure modes

- **Sign-convention error on `battery_dc_power`.** Verified at brief: positive = charging, negative = discharging. Hence `-battery_dc_power` is positive when discharging. Test `compensated_drain_only_counts_discharge_above_excluded_loads` locks this in.
- **Stale HP/cooker → underestimated drain.** Locked spec: stale = 0, *tighter* clamp. Documented in code.
- **Setpoint windup.** With `kp=1.0` and 2 kW excess drain, loop adds `+1000 W` per tick. Grid-side clamp (`grid_import_limit_w` / `grid_export_limit_w`) bounds this in `run_setpoint`. Deadband prevents excess MQTT churn.
- **`PreserveForZappi` projection branch drift.** SoC-chart projection is a derivative of the live controller; we keep the branch but document it's an approximation.
- **`build_setpoint_input` precondition expansion.** Adding `battery_dc_power` to the safety-fallback set means a momentary battery-service hiccup falls through to `apply_setpoint_safety` (idle 10 W). Conservative; matches safety-first posture.

#### Test plan (≥ 8 new tests, in `crates/core/src/controllers/setpoint.rs::tests`)

15. `compensated_drain_zero_when_loads_explain_battery_flow` — battery=-1500, HP=1000, cooker=500 → drain=0, relax branch.
16. `compensated_drain_clamped_zero_when_battery_charging` — battery=+2000, HP=0, cooker=0 → drain=0, relax branch.
17. `tightens_setpoint_when_drain_exceeds_threshold` — battery=-2500, threshold=1000, kp=1.0, prev=-3000 → drain=2500, excess=1500, new=-1500.
18. `relaxes_setpoint_toward_minus_solar_export_when_drain_below_threshold` — battery=-500, threshold=1000, prev=-3000, solar_export=2000, relax_step=100 → new=-2000 (clamped).
19. `stale_heat_pump_treated_as_zero` — caller substitutes 0; verify arithmetic.
20. `stale_cooker_treated_as_zero` — symmetric.
21. `early_morning_zappi_handled_by_unified_loop` — clock(3,0), Zappi active, soltaro_power=2000, battery=-100, HP=0, cooker=0 → drain=100, relax branch toward -solar_export.
22. `zappi_active_decision_factors_present` — assert all 11 factors populated.
23. `zappi_branch_bypassed_when_allow_battery_to_car_true` — knob true → existing day/evening branches run.
24. `force_disable_export_takes_priority_over_zappi_branch` — order preserved.
25. `setpoint_safety_fallback_fires_when_battery_dc_power_stale` — stale battery → safety fallback (idle 10 W).
26. `bookkeeping_unchanged_for_unified_zappi_branch` — snapshot parity vs. PR-ZD-2 baseline.

---

### 3.4 PR-ZD-4 — Hard clamp (Fast-mode safety net)

#### Scope

Post-`evaluate_setpoint()` step in `run_setpoint`: if Zappi *target* is
`Fast` AND `!allow_battery_to_car` AND `compensated_drain > hard_clamp_w`,
raise the proposed setpoint by the excess drain *before* it hits
`maybe_propose_setpoint`. Decision factors note clamp engagement.
Eco / Eco+ / Off bypass.

#### Files touched

Core (`crates/core/src/process.rs`):
- After `let out = evaluate_setpoint(...)` and before the existing `grid_export_limit_w / grid_import_limit_w` clamp in `run_setpoint`, insert the hard-clamp block (full pseudocode in the plan-execution prompt; reads `world.zappi_mode.target.value`, computes `compensated_drain_w(world)`, compares to `world.knobs.zappi_battery_drain_hard_clamp_w`, raises by excess).
- New helper `compensated_drain_w(world: &World) -> f64` near the existing `effective_*` cluster. Reads `world.sensors.battery_dc_power.value` (returns 0.0 if unusable; defensive); HP/cooker with the same 0-on-stale rule.
- Decision factors append (only when clamp engaged): `hard_clamp_engaged: true`, `hard_clamp_excess_W`, `hard_clamp_threshold_W`, `hard_clamp_pre_W`, `hard_clamp_post_W`. Match PR-09a-D02 pattern of only emitting factors when the clamp altered the value.

#### Acceptance criteria

- `cargo test --workspace` green.
- Live test: Zappi Fast, `allow_battery_to_car=false`, drain 500 W, hard_clamp=200 → clamp fires, `hard_clamp_excess_W: 300`, setpoint moves up by 300 W.
- Live test: Zappi Eco, same drain → no clamp.
- Live test: Zappi Fast, `allow_battery_to_car=true` → no clamp (operator opted in).

#### Risks / failure modes

- **Reading `world.zappi_mode.target.value` (target, not actual).** Decision: read target. Rationale: clamp is *predictive* — moment the controller commits to Fast, clamp arms; we don't wait for the next myenergi poll. Document.
- **Zappi target = `Unset` (cold boot).** `target.value` is `Option<ZappiMode>`; `None` → no Fast → no clamp. Defensive.
- **Soft loop + hard clamp interaction.** With defaults (threshold=1000, hard_clamp=200), drain=2000 → soft adds `+1000`; hard clamp adds `+1800`. Total = +2800 W. User-intended belt-and-suspenders. Test 32 locks it in.
- **Decision-factor pollution.** Only emit clamp factors when clamp altered the value (mirror existing PR-09a pattern).
- **Clamp-then-grid-cap interaction.** Hard clamp runs before grid-cap; combined raise exceeding `+grid_import_limit_w` gets clipped. Tests 31 + 32 cover.

#### Test plan (≥ 4 new tests, in `crates/core/src/process.rs::tests`)

27. `hard_clamp_engages_in_fast_mode_when_drain_exceeds_threshold` — Fast + !allow + drain=500 + hard_clamp=200 → setpoint raised by 300 W vs evaluate_setpoint's output.
28. `hard_clamp_disengaged_in_eco_mode` — Eco + drain=500 → no clamp.
29. `hard_clamp_disengaged_in_off_mode` — Off → no clamp.
30. `hard_clamp_disengaged_when_allow_battery_to_car_true` — Fast + allow=true → no clamp.
31. `hard_clamp_disengaged_when_drain_below_clamp_threshold` — Fast + drain=100 + hard_clamp=200 → no clamp.
32. `hard_clamp_combines_with_soft_loop` — Fast + drain=2000 + threshold=1000 + kp=1.0 + hard_clamp=200 → soft adds +1000, hard adds +1800, total +2800 modulo grid_import_cap.
33. `hard_clamp_respects_grid_import_cap` — Fast + drain=20000 + hard_clamp=200 + grid_import_limit_w=5000 → final capped at +5000.

---

### 3.5 PR-ZD-5 — Dashboard MPPT-mode display

#### Scope

Pure frontend. The two MPPT op-mode sensors land as numeric `0`/`1`/`2`
in PR-ZD-1; surface them as human-readable strings on the dashboard:

- `0` → "Off"
- `1` → "Voltage-or-current-limited"
- `2` → "MPPT-tracking"

#### Files touched

- `web/src/render.ts` — add a per-sensor formatter intercepting `mppt_0_operation_mode` / `mppt_1_operation_mode` and rendering the enum string. Fall back to numeric for out-of-range codes.
- `web/src/descriptions.ts` — extend the MPPT-op-mode description to enumerate codes and call out that `2` is the "everything is normal" reading.

#### Acceptance criteria

- `cd web && ./node_modules/.bin/tsc --noEmit -p .` clean.
- Live test: both MPPTs in MPPT-tracking → dashboard shows "MPPT-tracking".
- Live test: cloudy conditions where one charger is voltage-limited → "Voltage-or-current-limited".

#### Risks / failure modes

- **Float-vs-int rendering mismatch.** Wire shape `ActualF64` → JS receives a number; `Math.round` to int, then look up. Out-of-range falls back to `String(value)`.

#### Test plan (≥ 1 new test)

34. `mppt_operation_mode_renders_as_string` — three sub-cases for codes 0/1/2; one out-of-range (5) → numeric.

---

## 4. Cross-cutting locked decisions

- **Compensated-drain definition (locked):** `max(0, -battery_dc_power - heat_pump - cooker)`. Stale HP/cooker → 0. Stale battery_dc_power → safety fallback (idle 10 W).
- **Sign convention (locked):** `battery_dc_power > 0` = charging. `setpoint > 0` = importing.
- **Stale HP/cooker → 0, not last value (locked):** clamps tighter on dead bridge.
- **MPPT op-mode coupling (locked):** observability only, no control-loop reads.
- **Knob category (locked):** all five new knobs are `"config"`.
- **Freshness windows for new sensors (locked):** HP/cooker `30 s` (z2m push, 15 s reseed); MPPT op-modes `30 s` (15 s reseed).
- **Hard-clamp scope (locked):** only when Zappi *target* is Fast AND `!allow_battery_to_car` AND `world.derived.zappi_active`.
- **Hard-clamp ordering (locked):** runs after `evaluate_setpoint` returns, before the grid-cap clamp in `run_setpoint`.
- **Deadband interaction:** existing `setpoint_retarget_deadband_w` filters MQTT churn for both soft-loop and hard-clamp output. No special-case bypass.
- **Branch ordering in `evaluate_setpoint`:** `force_disable_export` → `zappi_active && !allow_battery_to_car` (the new branch) → `23:55–00:00` → evening → daytime → boost → extended-night. Identical to today; only the body of the second branch changes.

---

## 5. Verification matrix

After every PR (per CLAUDE.md):

| Command | Expected |
|---|---|
| `cargo test --workspace` | `test result: ok. <N> passed; 0 failed; 0 ignored`. Milestone target: ≥ 30 new tests. |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean exit. |
| `cd web && ./node_modules/.bin/tsc --noEmit -p .` | clean exit. |
| `cargo build --target armv7-unknown-linux-gnueabihf --release` | green build. |

Per-PR live verification:

- PR-ZD-1: all 4 sensors flip Fresh ≤ 30 s; flip Stale ≤ 30 s after stop.
- PR-ZD-2: 5 knobs round-trip via `mosquitto_pub` → MQTT retain → boot replay.
- PR-ZD-3: synthetic Zappi-active scenario produces expected tighten/relax decisions.
- PR-ZD-4: Fast vs Eco gate.
- PR-ZD-5: string rendering on dashboard.

---

## 6. Rollout / safety notes

Three escape hatches, ordered by reversibility:

1. **Set `zappi_battery_drain_threshold_w` to a very large value (e.g. 10 000).** Soft tighten path effectively disabled — drain never exceeds threshold, loop always relaxes toward `-solar_export`. Recovery: set back to `1000`.
2. **Set `allow_battery_to_car = true`.** Bypasses the entire Zappi branch (both soft loop and hard clamp); controller falls through to time-of-day branches. Recovery: set back to `false`.
3. **Set `writes_enabled = false`.** Master kill switch — observer mode. Decisions still computed; no actuation.

To disable the hard clamp specifically: `zappi_battery_drain_hard_clamp_w = 10000` (effectively unreachable).

MQTT bridge dies (HP/cooker): stale = 0, controller sees all drain as un-explained, tightens harder. Worst case: setpoint pegs at `+grid_import_limit_w`; battery stops draining; some PV wasted as grid import. Self-recovers when bridge returns. Better than the alternative ("stale = last value" → permanent leniency on a dead bridge).

Solarcharger D-Bus drops: `Mppt*OperationMode` flips Stale; loop unaffected.

Setpoint windup mid-incident: with kp=1.0 and 2 kW excess, loop raises by `+1000 W/tick`. With 15 s ticks the loop reaches `+grid_import_limit_w` (default 10 W) in roughly one tick — *immediate* clamp at the import cap. To soften: lower `kp` (e.g. 0.3); to delay: raise `threshold_w`.
