# Phase B — LG ThinQ heat-pump TASS integration plan

Detail doc for milestone **M-LG-THINQ**. Pointed at from `./tasks.md`.

## 1. Scope summary

Phase B promotes the LG ThinQ heat-pump bridge from a self-contained MQTT
sidecar (`crates/shell/src/lg_thinq/`, HEAD `36e4a29`) to a first-class
TASS-integrated subsystem. It adds:

- **4 KnobIds + 4 Actuated entities + 6 SensorIds** to `core` (knobs:
  heat-pump master power, DHW power, heating-water target °C, DHW target °C;
  sensors: 4 readbacks mirroring the actuators plus 2 read-only
  current-temperature sensors `lg_dhw_actual_c`, `lg_heating_water_actual_c`).
- **1 new Owner** (`Owner::HeatPumpController`) and **1 new core**
  (`HeatPumpControlCore`) running `evaluate_heat_pump`, gated by
  outdoor-temperature freshness.
- **1 new Effect** variant `Effect::CallLgThinq(LgThinqAction)` plus a
  shell-side `lg_thinq::Writer` that consumes it and posts to LG.
- **1 new shell timer** `TimerId::LgThinqPoller` emitting `Event::Sensor`
  for the 6 sensors (replacing the sidecar's direct MQTT publishes).
- **Web/MQTT plumbing**: 4 knob plumbing sites × 4 = 16 sites; 4 actuator
  dashboard sites; 6 sensor pipeline arms; 4 displayNames + 4 KNOB_SPEC +
  4 descriptions; 4 actuated displayNames + 4 actuated descriptions +
  4 mkRow rows.

It **deletes**: the command-handler half of `crates/shell/src/lg_thinq/mod.rs`
(subscriber, `KnobCommand` enum, `handle_commands`, `parse_bool`/`parse_int`,
optimistic-echo `LastApplied`, the bridge's separate MQTT client + LWT
availability topic `<root>/availability/lg_thinq` and the call to
`discovery::publish_all` from `lg_thinq/discovery.rs`). The state-poll loop
is rewritten to emit `Event::Sensor` into the runtime channel instead of
publishing raw MQTT. The whole HA discovery file
`crates/shell/src/lg_thinq/discovery.rs` is **deleted** — discovery now
flows through the main `mqtt::discovery::publish_ha_discovery` path via the
new `KnobId` / `ActuatedId` / `SensorId` variants.

Static analysis only — no real device this session. `defects.md` empty (or
deferred-with-rationale only).

## 2. PR breakdown decision — single PR (PR-LG-THINQ-B-1)

**Argued for: one PR.** Enforced by three structural constraints:

1. **Adding a `SensorId` variant breaks compilation everywhere.**
   `crates/core/src/types.rs:303 regime()`, `:387 reseed_cadence()`,
   `:483 actuated_id()`, `:543 is_external_polled()`, `:247 ALL`, the
   per-variant test at `:1500–1731`, plus `Sensors::by_id` at
   `world.rs:73`, `apply_sensor_reading` at `process.rs:296`,
   `apply_tick` at `process.rs:1058`, and `serialize.rs::sensor_name`
   at `:536` and `discovery.rs::sensor_meta` at `:554` all use
   explicit per-variant matches (no `_ =>` arm). Same applies to
   `KnobId`, `ActuatedId`, `Owner`. Splitting "add types" from "use
   types" is impossible without inserting transient `_ =>` arms.
2. **The baboon model must move atomically.** Adding `ActuatedBool` to
   `data Actuated` requires regen, which fixes shell convert + dashboard
   simultaneously.
3. **The Phase A delete and Phase B add must land together** to avoid
   duplicate HA-discovery entities racing on the same topic root.

Mitigation: order sub-tasks so each cohort is reviewable as a logical
chunk even though the PR ships as one.

## 3. Implementation order — D-checklist

**Read `CLAUDE.md` checklists first.** This plan applies the "Adding a
new knob" checklist 4× and the "Adding a new actuator" checklist 4× —
with 3 of the 4 actuators sharing each readback sensor as their mirror.

### D01. Baboon model — extend `data Actuated`, `enum Owner`, `data Sensors`

File: `models/dashboard.baboon`.

- Above `data Actuated` at `:108`, add a new wire type:
  ```baboon
  data ActuatedBool {
    target_value: opt[bit]
    target_owner: Owner
    target_phase: TargetPhase
    target_since_epoch_ms: i64
    actual: ActualBool
  }
  data ActualBool {
    value: opt[bit]
    freshness: Freshness
    since_epoch_ms: i64
  }
  ```
  Mirror the shape of `ActuatedI32` + `ActualI32` from
  `models/dashboard-0.1.0.baboon:94,103` exactly.
- Extend `data Actuated` at `:108–116` with four new fields:
  ```baboon
  lg_heat_pump_power: ActuatedBool
  lg_dhw_power: ActuatedBool
  lg_heating_water_target_c: ActuatedI32
  lg_dhw_target_c: ActuatedI32
  ```
- Extend `enum Owner` at `:89–102` with `HeatPumpController` (at the end).
- Extend `data Sensors` at `:56–84` with two new fields (readback paths
  live on the new `Actuated*` slots — they don't need `Sensors` fields):
  ```baboon
  lg_dhw_actual_c: ActualF64
  lg_heating_water_actual_c: ActualF64
  ```
- Run `./scripts/regen-baboon.sh`. Expect ~20 new Rust files and matching
  TypeScript under `crates/dashboard-model/src/` and `web/src/model/`.

**Why first**: every downstream consumer in Rust + TS depends on these
regenerated types.

### D02. Owner — `Owner::HeatPumpController`

File: `crates/core/src/owner.rs:9–36`.

Add `HeatPumpController` after `EssStateOverrideController` at `:35`.
Doc-comment: `/// HeatPumpControl core (evaluate_heat_pump).`

### D03. Core types — `SensorId`, `ActuatedId`, `KnobId`, `Effect`, `LgThinqAction`, `TimerId`

File: `crates/core/src/types.rs`.

- `SensorId` at `:62–132`: add 6 variants at end:
  - `LgHeatPumpPowerActual` (bool readback mirror of `ActuatedId::LgHeatPumpPower`)
  - `LgDhwPowerActual`
  - `LgHeatingWaterTargetActual`
  - `LgDhwTargetActual`
  - `LgDhwCurrentTemperatureC` (plain f64)
  - `LgHeatingWaterCurrentTemperatureC` (plain f64)
- `SensorId::ALL` at `:247–290`: add all 6.
- `SensorId::freshness_threshold` at `:145–217`: all 6 → `Duration::from_secs(180)`
  (3 min — ≥ 2× the 60 s poll period plus headroom; see §7 R1).
- `SensorId::regime` at `:304–369`: 6 variants → `FreshnessRegime::ReseedDriven`.
- `SensorId::reseed_cadence` at `:387–460`: 6 variants → `Duration::from_secs(60)`.
- `SensorId::actuated_id` at `:483–534`: 4 readback variants route to
  their `ActuatedId::Lg*`; the 2 plain temperature sensors return `None`.
- `is_external_polled` at `:543–553`: **do not** add the LG sensors
  (see §7 R1).
- `ActuatedId` at `:708–720`: add 4 variants — `LgHeatPumpPower`,
  `LgDhwPower`, `LgHeatingWaterTarget`, `LgDhwTarget`.
- `KnobId` at `:724–815`: add 4 variants — `LgHeatPumpPower`,
  `LgDhwPower`, `LgHeatingWaterTargetC`, `LgDhwTargetC`.
- `KnobValue` at `:1043–1055`: no new variants. Bool via
  `KnobValue::Bool`; °C targets via `KnobValue::Uint32` (LG's
  `set_dhw_target_c` and `set_water_heat_target_c` take `i64`; cast at
  the writer boundary).
- New `LgThinqAction` enum, near `MyenergiAction` at `:1230`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum LgThinqAction {
      SetHeatPumpPower(bool),
      SetDhwPower(bool),
      SetHeatingWaterTargetC(i64),
      SetDhwTargetC(i64),
  }
  ```
- `Effect` at `:1463–1484`: add `CallLgThinq(LgThinqAction)` after
  `CallMyenergi(MyenergiAction)`.
- `TimerId` at `:881–997`: add `LgThinqPoller` variant, extend `ALL`,
  `name()` → `"timer.lg-thinq.poll"`, `description()` →
  `"LG ThinQ heat-pump cloud poller (state + temperature readbacks)"`.
- Update the per-variant invariant tests at `:1500` and `:1656` to
  enumerate the 6 new sensor variants. Explicit-match-no-wildcard, so
  they'll fail-build if missed.

### D04. Knobs struct

File: `crates/core/src/knobs.rs:91–269`.

Add 4 fields after `actuator_retry_s` at `:252`:
```rust
pub lg_heat_pump_power: bool,
pub lg_dhw_power: bool,
pub lg_heating_water_target_c: u32,
pub lg_dhw_target_c: u32,
```

Update `safe_defaults` at `:415–540`: `lg_heat_pump_power=false`,
`lg_dhw_power=false`, `lg_heating_water_target_c=42`, `lg_dhw_target_c=60`.
Extend the `safe_defaults_match_spec_7` test at `:553`.

### D05. World — `World::lg_*` + `Sensors::lg_*` + `Sensors::by_id` arms

File: `crates/core/src/world.rs`.

- `Sensors` struct around `:60`: add
  `pub lg_dhw_current_c: Actual<f64>` and
  `pub lg_heating_water_current_c: Actual<f64>`.
- `Sensors::by_id` at `:73`: arms for the 2 new f64 sensors → matching
  `self.<field>`. The 4 readback variants (`Lg*Actual`) join the
  existing actuated-mirror block at `:115` returning `Actual::unknown`
  (storage of truth is `world.lg_*.actual`).
- `Sensors::unknown` at `:131`: init both new f64 sensors to
  `Actual::unknown(now)`.
- `World` struct around `:597–610`: add 4 `Actuated<…>` fields:
  ```rust
  pub lg_heat_pump_power: Actuated<bool>,
  pub lg_dhw_power: Actuated<bool>,
  pub lg_heating_water_target_c: Actuated<i32>,
  pub lg_dhw_target_c: Actuated<i32>,
  ```
- `World::fresh_boot` at `:745`: init all 4 with `Actuated::new(now)`.

### D06. Core process — `apply_knob`, `apply_sensor_reading`, `apply_tick`

File: `crates/core/src/process.rs`.

- `apply_knob` at `:691`: add 4 arms. For the master `LgHeatPumpPower`
  knob (operator-only — controller never proposes), `apply_knob` ALSO
  calls `world.lg_heat_pump_power.propose_target(value, owner, now)`
  where `owner` is threaded from `Event::Command`. The 3 controller-
  driven knobs get the same operator-mirror treatment so a dashboard
  edit shows up immediately (controller re-proposes next tick with
  `Owner::HeatPumpController`, overriding within ≤ tick).
- `apply_sensor_reading` at `:296`: add arms for the 2 plain f64 sensors
  (write to `world.sensors.lg_*_current_c`). Add arms for the 4
  actuator-mirror variants — leave them empty like the schedule-mirror
  block at `:386`. The post-update hook block at `:406–450` must grow
  4 new `Some(ActuatedId::Lg*)` arms doing `on_reading + confirm_if`:
  - Bool actuators: `let bool_val = v != 0.0; world.lg_*.on_reading(bool_val, at)`.
    Predicate: strict `|t, a| t == a`.
  - i32 actuators: `world.lg_heating_water_target_c.on_reading(v as i32, at)`,
    `confirm_if(|t, a| t == a, at)`.
  - On confirm-success, emit
    `Effect::Publish(PublishPayload::ActuatedPhase { id: ActuatedId::Lg*, phase: ... })`.
    Mirror `EssStateTarget` at `:329–342`.
- `apply_tick` at `:1058`: extend freshness-decay block at `:1126–1140`
  with `world.lg_*.tick(at, SensorId::Lg*Actual.freshness_threshold())`
  for the 4 actuators; extend sensor decay block at `:1063–1109` with
  `world.sensors.lg_*_current_c.tick(...)` for the 2 plain sensors.

### D07. Controller — `controllers/heat_pump.rs::evaluate_heat_pump`

New file: `crates/core/src/controllers/heat_pump.rs`.

Pure controller, mirrors `controllers/ess_state_override.rs`:
- Input struct fields: `now_local: NaiveDateTime`, `outdoor_temp: Actual<f64>`.
- Output: 3 `Option<…>` decisions + a `Decision` factor list.
- Logic:
  - **DHW power** `Option<bool>`: `Some(true)` when `now_local.time()`
    ∈ [02:00, 05:00) ∪ [07:00, 08:00), else `Some(false)`. Always
    proposes.
  - **DHW target** `Option<i32>`: `Some(60)` constant. Always proposes.
  - **Heating-water target** `Option<i32>`: only `Some(_)` when
    `outdoor_temp.freshness == Fresh`. Strict boundaries: `t ≤ 2 → 48`,
    `t ≤ 5 → 46`, `t ≤ 8 → 44`, `t ≤ 10 → 43`, else `42`. Skip-on-non-
    Fresh, do not propose, do not emit.
- Register `pub mod heat_pump;` in `crates/core/src/controllers/mod.rs`.

### D08. Core DAG — `HeatPumpControlCore`

File: `crates/core/src/core_dag/cores.rs` (+ companion changes in
`crates/core/src/core_dag/mod.rs`).

- `CoreId::HeatPumpControl` in `mod.rs:27–49` enum + `:55–67` name arm.
  Name: `"heat-pump.control"`. Place after `EssStateOverride`.
- New `pub(crate) struct HeatPumpControlCore` near `EssStateOverrideCore`
  at `:656`. Pattern from `EssStateOverrideCore::run` at `:667–726`:
  - `depends_on()` returns `&[]`.
  - `run()` builds input, calls `evaluate_heat_pump`, and for each
    `Some(…)` target:
    - `world.lg_*.propose_target(value, Owner::HeatPumpController, now_mono)`.
    - Push `Effect::Publish(PublishPayload::ActuatedPhase { … })`.
    - If `writes_enabled=false`: emit
      `Effect::Log { LogLevel::Info, source: "observer", message: "…" }`.
    - If `writes_enabled=true` AND
      (`changed || world.lg_*.needs_actuation(now, retry_threshold)`):
      emit `Effect::CallLgThinq(LgThinqAction::Set<X>(value))`,
      `mark_commanded`, re-publish phase. Mirror `run_eddi_mode` at
      `process.rs:2295–2363`.
  - **NOT** proposed: `lg_heat_pump_power` (4th actuator). The slot
    receives operator writes via `apply_knob`'s mirror (D06); the
    controller does nothing for it.
- Register in production registry at `cores.rs:1037`-ish:
  `Box::new(HeatPumpControlCore)` after `Box::new(EssStateOverrideCore)`.
- Add a `DepEdge { from: CoreId::HeatPumpControl, fields: &[] }` to
  `SensorBroadcastCore::depends_on` at `:809–818`.

### D09. (Merged into D06 — `apply_knob` mirrors knob → actuated.)

### D10. Re-run baboon / verify regen compiles

Run `./scripts/regen-baboon.sh` if the model changed since D01. Confirm
`crates/dashboard-model/` compiles standalone — the post-regen `sed`
patches in `regen-baboon.sh:113–154` for `actuated_f64.rs` etc. do NOT
extend to `actuated_bool.rs` because `bit`/`bool` has no `total_cmp`
issue. Verify by inspecting the generated file after regen.

### D11. Shell writer — `lg_thinq::{Client, Writer, Poller}`

File: `crates/shell/src/lg_thinq/mod.rs`.

Refactor. **Delete**:
- `KnobCommand` enum at `:230–236`.
- `MqttBridge`, `MqttBridge::new` at `:238–288`.
- `run_subscriber` at `:290–340`.
- `handle_commands` at `:342–425`.
- `LastApplied` at `:427–436`.
- `parse_bool`/`parse_int`/`bool_text`/`format_int`/`format_float` at
  `:531–558`.
- `mod discovery;` import + the `discovery::publish_all` call at
  `:156–163`.
- The `availability_topic` publish at `:166–168` and `:222–224`.
- The `KNOB_*` / `SENSOR_*` constant block at `:40–47`.

Replace `Service::new` + `Service::run` with:
- A `Client` struct holding `ThinqApi` + `device_id` +
  `(heating_range, dhw_range)`, mirroring `myenergi::Client` at
  `myenergi/mod.rs:43–58`.
- A `Writer` struct (new) holding `Arc<Client>` + `dry_run: bool`.
  Mirror `myenergi::Writer` at `myenergi/mod.rs:540–569`. `execute(action)`:
  - `SetHeatPumpPower(b)` → `HeatPumpControl::set_heating_power(b)`,
    `api.post_device_control(&device_id, payload).await`.
  - `SetDhwPower(b)` → `HeatPumpControl::set_dhw_power(b)`.
  - `SetHeatingWaterTargetC(t)` → `validate_temperature_c(t, h.0, h.1)`,
    `HeatPumpControl::set_water_heat_target_c(t)`.
  - `SetDhwTargetC(t)` → `validate_temperature_c(t, d.0, d.1)`,
    `HeatPumpControl::set_dhw_target_c(t)`.
- A `Poller` struct (new) holding `Arc<Client>` + `poll_period: Duration`.
  Mirror `myenergi::Poller` at `myenergi/mod.rs:311–526`. `run(tx)`:
  - Every `poll_period`: `api.get_device_state(&device_id)`.
  - `HeatPumpState::from_json(...)`.
  - For each present field, emit `tx.send(Event::Sensor(SensorReading { id, value, at }))`.
    Bool readbacks → `1.0` / `0.0`.
  - Emit `Event::TimerState { id: TimerId::LgThinqPoller, … }` per cycle.

### D12. (Folded into D03 — `TimerId::LgThinqPoller`.)

### D13. Runtime — `Effect::CallLgThinq` dispatch

File: `crates/shell/src/runtime.rs`.

- `Runtime` struct at `:28–38`: add `lg_thinq: Option<LgThinqWriter>`.
- `Runtime::new` at `:41–71`: add the parameter.
- `dispatch` at `:110`: add an arm before `CallMyenergi` at `:122`:
  ```rust
  Effect::CallLgThinq(action) => {
      if let Some(lg) = &self.lg_thinq {
          let lg = lg.clone();
          tokio::spawn(async move {
              match tokio::time::timeout(Duration::from_secs(20), lg.execute(action)).await {
                  Ok(()) => {}
                  Err(_) => warn!(?action, "lg_thinq call stuck >20s; dropping"),
              }
          });
      } else {
          debug!(?action, "CallLgThinq dropped (no [lg_thinq] configured)");
      }
  }
  ```
  Mirror `CallMyenergi` at `runtime.rs:122–145` line-for-line.

### D14. Main wiring

File: `crates/shell/src/main.rs`.

- Replace the sidecar spawn at `:539–563` with:
  ```rust
  let (lg_writer, lg_poller): (Option<LgThinqWriter>, Option<tokio::task::JoinHandle<()>>) =
      if cfg.lg_thinq.is_configured() {
          let client = LgThinqClient::new(&cfg.lg_thinq)?;
          let writer = LgThinqWriter::new(client.clone(), !cfg.lg_thinq.writes_enabled);
          let tx_for_lg = tx.clone();
          let poller_handle = tokio::spawn(
              LgThinqPoller::new(client, cfg.lg_thinq.poll_period).run(tx_for_lg),
          );
          (Some(writer), Some(poller_handle))
      } else {
          info!("lg_thinq: not configured; bridge disabled");
          (None, None)
      };
  ```
- Thread `lg_writer` into `Runtime::new`.

### D15. MQTT serialize — 4 sites + actuated_name + sensor_name

File: `crates/shell/src/mqtt/serialize.rs`.

- `knob_name` at `:359`:
  - `LgHeatPumpPower → "heat-pump.power.master"`
  - `LgDhwPower → "heat-pump.dhw.power"`
  - `LgHeatingWaterTargetC → "heat-pump.heating-water.target-c"`
  - `LgDhwTargetC → "heat-pump.dhw.target-c"`
- `knob_id_from_name` at `:433`: inverse map.
- `knob_range` at `:649`: for the 2 temperature knobs, read bounds from
  a process-wide `OnceLock<LgThinqRanges>` populated at startup from
  `cfg.lg_thinq` (mirror `hardware_params()` at `:686`). Bool knobs
  fall through to the `return None` block at `:750–770`. **See §9 Q2.**
- `parse_knob_value` at `:828`: bool block + Uint32 block both grow 2
  arms each.
- `actuated_name` at `:512`:
  - `LgHeatPumpPower → "heat-pump.power.master.target"`
  - `LgDhwPower → "heat-pump.dhw.power.target"`
  - `LgHeatingWaterTarget → "heat-pump.heating-water.target.target-c"`
  - `LgDhwTarget → "heat-pump.dhw.target.target-c"`
- `sensor_name` at `:536`:
  - `LgDhwCurrentTemperatureC → "heat-pump.dhw.current-c"`
  - `LgHeatingWaterCurrentTemperatureC → "heat-pump.heating-water.current-c"`
  - The 4 `Lg*Actual` mirror variants join the existing `unreachable!`
    block at `:574–589`.

### D16. MQTT discovery — knob schemas + phases + sensor_meta

File: `crates/shell/src/mqtt/discovery.rs`.

- `knob_schemas` at `:663`:
  ```rust
  (KnobId::LgHeatPumpPower, "switch", json!({"payload_on": "true", "payload_off": "false"})),
  (KnobId::LgDhwPower,      "switch", json!({"payload_on": "true", "payload_off": "false"})),
  number_knob(KnobId::LgHeatingWaterTargetC, 1.0, Some("°C")),
  number_knob(KnobId::LgDhwTargetC,          1.0, Some("°C")),
  ```
- `publish_phases::ids` at `:194–201`: add the 4 new `ActuatedId::Lg*`.
- `sensor_meta` at `:554` (scan the file): add arms for the 2 new f64
  temperature sensors → `unit: Some("°C")`, `device_class: Some("temperature")`,
  `state_class: "measurement"`. Mirror existing temperature-sensor rows.
- The 4 readback variants (`Lg*Actual`) auto-skip discovery via the
  `id.actuated_id().is_some()` filter at `:233` — no extra arm needed.

### D17. Shell config — `KnobsDefaultsConfig` extension

File: `crates/shell/src/config.rs`.

- `LgThinqConfig` at `:234–306`: **keep all existing fields**. The
  writer + poller consume them; the `heating_target_*_c` and
  `dhw_target_*_c` ranges feed `knob_range` via the OnceLock from D15.
- `KnobsDefaultsConfig` at `:991–1069`: add 4 `Option<…>` fields.
- `apply_to` at `:1074–1151`: 4 new `set!` lines.
- `config.example.toml`: 4 commented lines under `[knobs]`.

### D18. Dashboard convert

File: `crates/shell/src/dashboard/convert.rs`.

- `owner` at `:171–187`: add `Owner::HeatPumpController → ModelOwner::HeatPumpController`.
- New `actuated_bool` helper near `actuated_i32` at `:259–271`.
- `WorldActuatedRefs` at `:1260–1268`: 4 new fields.
- `world_actuated` at `:1248–1258`: 4 new mappings.
- `knob_id_from_name` at `:1395`: 4 snake_case arms.
- `knobs_to_model` at `:1148`: 4 field assignments.
- Sensor block (search `ModelSensors`): include `lg_dhw_actual_c` +
  `lg_heating_water_actual_c`. Snapshot's `actuated:` literal must grow
  4 fields.

### D19. Delete the sidecar's discovery file

`crates/shell/src/lg_thinq/discovery.rs` — **delete entirely**.

### D20. Web — `displayNames.ts`

Per `web/src/displayNames.ts:131–148`, actuated entries can share
snake_case keys with knobs via `displayNameOfTyped(canonical, "actuated")`.
Add:
- Sensors: `lg_dhw_actual_c`, `lg_heating_water_actual_c`.
- Knobs: `lg_heat_pump_power`, `lg_dhw_power`,
  `lg_heating_water_target_c`, `lg_dhw_target_c`.
- Actuated (type-disambiguated): same snake_case keys, `.target` suffix
  on the dotted form per the existing convention.

### D21. Web — `knobs.ts` `KNOB_SPEC`

Add to a new group `"Heat pump"`:
```ts
"heat-pump.power.master": { kind: "bool", default: false, category: "operator", group: "Heat pump" },
"heat-pump.dhw.power":    { kind: "bool", default: false, category: "operator", group: "Heat pump" },
"heat-pump.heating-water.target-c": { kind: "int", min: 25, max: 55, step: 1, default: 42, category: "operator", group: "Heat pump" },
"heat-pump.dhw.target-c":           { kind: "int", min: 30, max: 65, step: 1, default: 60, category: "operator", group: "Heat pump" },
```
Add `"Heat pump"` to `OPERATOR_GROUPS` at `:37–41`.

### D22. Web — `descriptions.ts`

4 knob + 4 actuated + 2 sensor descriptions. ~1-2 sentences each.

### D23. Web — `render.ts` `renderActuated`

File: `web/src/render.ts:571–671`. Add 4 `mkRow` entries — 2 i32 (mirror
`ess_state_target` at `:659–667`) and 2 bool (new shape — read the
generated `ActuatedBool` shape after baboon regen).

### D24. Web — sensor table

`web/src/render.ts` sensor block: add 2 rows for `lg_dhw_actual_c` +
`lg_heating_water_actual_c` (if not already auto-generated; verify).

## 4. Critical-look-at-first list

Read these before writing any code:

1. `crates/core/src/controllers/ess_state_override.rs:76–177` — canonical
   pure-controller shape with a `Decision` factor list; closest match for
   `evaluate_heat_pump`.
2. `crates/core/src/controllers/eddi_mode.rs:112–` — the dwell/freshness
   gate pattern.
3. `crates/core/src/process.rs:2295–2363` `run_eddi_mode` — canonical
   `mark_commanded` + retry + `writes_enabled` gate + `ActuatedPhase`
   publish. Mirror in `HeatPumpControlCore::run`.
4. `crates/core/src/process.rs:1735–1800` (`set_grid_setpoint` /
   `apply_setpoint_safety`) — surrounding scaffolding.
5. `crates/shell/src/myenergi/mod.rs:540–569` `Writer` + `:311–526`
   `Poller` — direct templates for the LG Writer + Poller.
6. `crates/shell/src/mqtt/serialize.rs:359/433/512/536/649/828` — the
   4 knob sites + actuated_name + sensor_name + knob_range.
7. `crates/shell/src/mqtt/discovery.rs:663/190/554` — HA discovery
   shape.
8. `crates/shell/src/dashboard/convert.rs:1248/1260/1395/1148/171` —
   five surfaces per CLAUDE.md actuator step 8.
9. `crates/core/src/types.rs:62/303/387/145/483/543` — staleness
   invariant scaffolding.
10. `crates/core/src/world.rs:73 Sensors::by_id` (`:104–126`) —
    actuated-mirror vs plain-sensor storage rule.
11. `crates/core/src/process.rs:296–451 apply_sensor_reading` —
    post-hook `actuated_id()` routing.
12. `models/dashboard.baboon:56/89/108` — `data Sensors` / `enum Owner` /
    `data Actuated` blocks to extend.
13. `web/src/displayNames.ts:131–148 Actuated section` — actuated/knob
    snake_case-collision disambiguation pattern.
14. `web/src/knobs.ts:59 KNOB_SPEC` + `web/src/render.ts:571 renderActuated`.
15. `crates/core/src/tass/actuated.rs:36 Actuated<V>` + `:154 impl<V: PartialEq>`
    — `Actuated<bool>` composes cleanly.

## 5. Test plan

### 5.1 `evaluate_heat_pump` purity tests

New `#[cfg(test)] mod tests` in `crates/core/src/controllers/heat_pump.rs`:

- 5 outdoor-temp buckets × at least 1 DHW-window case each:
  - `t=1.0` → heating=48
  - `t=4.0` → heating=46
  - `t=6.0` → heating=44
  - `t=9.0` → heating=43
  - `t=15.0` → heating=42
- Boundary tests for `t=2.0, 5.0, 8.0, 10.0` (`≤` is inclusive).
- 3 DHW-window cases:
  - 02:30 → DHW power=true
  - 07:30 → DHW power=true
  - 12:00 → DHW power=false
- Boundary tests for the window edges: 02:00 in, 05:00 out, 07:00 in,
  08:00 out.
- `stale_outdoor_temperature_skips_heating_target`: `freshness=Stale` →
  heating field is `None`; DHW unaffected.
- `unknown_outdoor_temperature_skips_heating_target`: same for `Unknown`.
- `dhw_target_constant_60`: every test asserts `dhw_target_c == Some(60)`.

### 5.2 Core `apply_knob` round-trip tests

Extend `crates/core/src/process.rs` test section. Four tests, one per
knob. Mirror `apply_knob_zappi_battery_drain_threshold_w_routes_to_field`
at `:5809`. For the 3 controller-driven knobs, also assert the operator
write propagates into `world.lg_*.target.value`.

### 5.3 Core `apply_sensor_reading` tests

Six tests, one per new `SensorId`. Pattern from
`apply_sensor_reading_heat_pump_power_writes_field` at `process.rs:5660`.
For the 4 actuator-mirror tests, also assert `world.lg_*.actual.value`
is set AND that a confirm-success `ActuatedPhase` effect is emitted.

### 5.4 `LgThinqAction → JSON payload` tests

Four tests in `crates/shell/src/lg_thinq/mod.rs::tests`. Assert
`Writer::execute(LgThinqAction::Set<X>(value))` produces the exact JSON
envelope from `HeatPumpControl::set_<X>` (per existing tests at
`heat_pump.rs:347–392`):
- `set_heating_power(true)` → `{"operation":{"boilerOperationMode":"POWER_ON"}}`
- `set_dhw_power(true)` → `{"operation":{"hotWaterMode":"ON"}}`
- `set_dhw_target_c(60)` → `{"hotWaterTemperatureInUnits":{"targetTemperature":60,"unit":"C"}}`
- `set_water_heat_target_c(48)` → `{"roomTemperatureInUnits":{"waterHeatTargetTemperature":48,"unit":"C"}}`

Use a recording fake of `ThinqApi` so the test doesn't hit the network
(dual-tests pattern).

### 5.5 Existing invariant tests

`SensorId::ALL` membership + `freshness_threshold_invariant` +
`check_staleness_invariant` (startup) — all auto-pinned via the
explicit per-variant matches in `types.rs:1500/1656`. Just add the 6
new variants to the test bodies; the missing-variant build-error rule
does the rest.

### 5.6 `safe_defaults_match_spec_7` extension

`crates/core/src/knobs.rs:553`. 4 new assertions.

### 5.7 MQTT serialize round-trip

`knob_name(KnobId::Lg*) → knob_id_from_name(...) → KnobId::Lg*` for
each of the 4 knobs.

### 5.8 Dashboard convert tests

`world_actuated(&world).lg_heat_pump_power` round-trips a proposed
target value.

## 6. Risk register

### R1. Staleness invariant for the 6 new sensors

- LG poll cadence = 60 s (`default_lg_thinq_poll`).
- **Decision**: strict 2× rule → `freshness_threshold = 180 s` (3 min
  with headroom). Do NOT mark `is_external_polled`.
- Justification: the LG poll is owned by our poller (bounded, retried),
  not external publisher-paced. Strict rule applies, like
  `Mppt0/1OperationMode`.

### R2. `Actuated<bool>` composition

`Actuated<V>` is generic; `propose_target` only requires `V: PartialEq`.
`bool` is `PartialEq`. No workaround.

### R3. Baboon `ActuatedBool` missing today

Confirmed via D01-pre exploration: `models/dashboard.baboon:108–116` has
only `ActuatedI32`, `ActuatedF64`, `ActuatedEnumName`, `ActuatedSchedule`.
**Action**: D01 adds `ActuatedBool` + `ActualBool`. No `total_cmp` patch
needed in `regen-baboon.sh` (bool has native cmp).

### R4. Local time bucketing for DHW windows

`clock.naive()` reads via `topology.tz_handle`. The TZ is operator-
configurable through Victron settings (`Event::Timezone`). Use the same
pattern as `evaluate_schedules`. **Action**: `evaluate_heat_pump`
consumes `now_local: NaiveDateTime` passed in from `clock.naive()` and
compares `now_local.time()` against `chrono::NaiveTime::from_hms_opt`.

### R5. `LgThinqAction` payload-shape correctness

Verified via `crates/lg-thinq-client/src/heat_pump.rs`:
- `set_heating_power(b: bool) → Value` at `:189`
- `set_dhw_power(b: bool) → Value` at `:199`
- `set_dhw_target_c(t: i64) → Value` at `:209`
- `set_water_heat_target_c(t: i64) → Value` at `:224`

All 4 exist. Temperature actuators are `Actuated<i32>` → cast to `i64`
at the writer boundary.

### R6. Heat-pump master power asymmetry

Slot 4 (`lg_heat_pump_power`) is not proposed by the controller.
**Action**: `apply_knob`'s `LgHeatPumpPower` arm does double-duty: write
`world.knobs.lg_heat_pump_power` AND
`world.lg_heat_pump_power.propose_target(value, owner_from_command, now)`.
`HeatPumpControlCore` skips this slot. Phase + readback still work; the
operator's `Owner::Dashboard` (or `HaMqtt`) target persists until the
operator changes it.

### R7. HA discovery duplicate-entity risk during deploy

Old sidecar entities (snake_case `lg_*` prefix) and new core-emitted
entities (dotted `heat-pump.*`) won't collide on `unique_id` (different
shapes), but HA will keep the old retained discovery as zombies.
**Action**: the executor publishes one-shot empty payloads to the 6 old
discovery topics at the next boot — implementable as a small `cleanup`
helper in the new `lg_thinq` module, gated by a `cleaned_v1_discovery:
Option<bool>` field on `Bookkeeping`. (See §9 Q4 — alternative: document
the cleanup in the PR description and let the operator purge manually.)

### R8. The post-update `on_reading` for bool actuators

`SensorReading.value` is `f64`. Bool readbacks arrive as `0.0` / `1.0`.
**Action**: in the post-hook at `:406–450`,
`let bool_val = v != 0.0; world.lg_*.on_reading(bool_val, at)`. Predicate
`|t: &bool, a: &bool| t == a` strictly confirms.

## 7. Definition of done

- `cargo test --workspace` — green.
- `cargo clippy --workspace --all-targets -- -D warnings` — green.
- `cd web && ./node_modules/.bin/tsc --noEmit -p .` — green.
- Phase A sidecar command-handler removed; state-publishing replaced by
  `SensorReading` events.
- Static analysis only; no real device this session.
- `defects.md` empty (or only deferred-with-rationale entries).

## 8. Open questions — orchestrator decisions

**Q1**. Operator-knob → actuated mirror for the master power slot.
*Decision*: yes — `apply_knob` does double-duty. Mirrors in `apply_knob`
also apply to the 3 controller-driven knobs so operator writes show up
in the dashboard immediately (controller re-proposes next tick).

**Q2**. Source of HA-slider ranges for the 2 temperature knobs.
*Decision*: thread `cfg.lg_thinq.heating_target_*_c` / `dhw_target_*_c`
through a process-wide `OnceLock<LgThinqRanges>` (mirror
`hardware_params()` at `serialize.rs:686`). Initialised once at startup
from the loaded config.

**Q3**. HA discovery for the 2 controller-driven temperature knobs.
*Decision*: yes — operator override matters; ESS-state-override
precedent applies.

**Q4**. Phase A `<root>/availability/lg_thinq` topic.
*Decision*: delete. Stale `Actuated` phases + stale sensor readings
surface failures via the existing infrastructure.

**Q5**. The `cache_dir` field on `LgThinqConfig`.
*Decision*: keep (no migration). Phase C will use it for MQTT push.
Add a `TODO(phase-c)` comment in the config struct.
