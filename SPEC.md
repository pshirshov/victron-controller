# victron-controller — Specification

Single source of truth for the on-device Rust service that replaces the Node-RED flows on the user's MultiPlus-II GX.

Sections marked **FILLME** need user input. Everything else is a frozen decision.

---

## 1. Goal

Port the current Node-RED control stack (set-point, current-limit, charging schedules, weather-SoC planner, Zappi/Eddi control) onto the Victron GX as a single Rust daemon. Home Assistant stops being a hot-path dependency; it becomes one of several UI surfaces over MQTT. All persistent state moves to an external MQTT broker.

Drivers:

- Node-RED flows are too complex to maintain correctness; corner cases remain.
- HA is on a separate host — outages currently degrade control.
- Moving to a typed, tested, pure-core design reduces the space of failure modes.

Two net-new features on top of the port:

1. **`allow_battery_to_car`** toggle — optionally permit discharging the DC battery into the EV during Zappi-active windows.
2. **Local Eddi on/off** — replace the HA automation with on-device SoC-based control.

---

## 2. Hardware topology

- **MultiPlus-II GX** (integrated ARMv7 Venus OS GX). All code runs on this device.
- **DC battery pack** (Pylontech) on `com.victronenergy.battery/512`.
- **2× MPPT solar chargers** (`solarcharger/274`, `solarcharger/289`) covering 8 sub-arrays with different tilts/azimuths.
- **AC-coupled Soltaro battery** (no MPPT, used purely as an AC-coupled battery); its meter is `com.victronenergy.pvinverter/33` ("ET112 Soltaro"). This remains the Soltaro meter — not the EV branch.
- **Myenergi Zappi** EV charger and **Myenergi Eddi** diverter, both on the same myenergi hub.
- **Hoymiles microinverters** AC-coupled, wired **through the Zappi**. The EV-branch ET112 is on D-Bus as `com.victronenergy.evcharger/35`; its `/Ac/Power` and `/Ac/Current` are **signed** — positive = net import (car), negative = net export (Hoymiles).
- **Grid meter** `com.victronenergy.grid/34`, **vebus** `com.victronenergy.vebus/275`, **system** `com.victronenergy.system/0`.
- **Home Assistant** on a separate host; after cutover, consumed only via MQTT.

Service instance IDs above are copied from the current NR deployment; M1 discovery will confirm them on the running system.

---

## 3. Current Node-RED flows — reverse-engineered

The NR project has seven tabs; Maintenance and MongoDB writes are out of scope. The six in-scope tabs:

| Tab | Trigger | Output side-effect |
|---|---|---|
| Set-Point | any D-Bus input change (joined) | `com.victronenergy.settings /Settings/CGwacs/AcPowerSetPoint` |
| Current limit | inject every 5 s | `com.victronenergy.vebus/275 /Ac/In/1/CurrentLimit` |
| Charging schedules | manual start + when SoC == 100 | 10 `BatteryLife/Schedule/Charge/{0,1}/*` paths |
| Weather SoC Target | cron `55 01 * * *` | POSTs HA `input_select` options |
| Zappi | cron every 15 s + 08:00 + cron 02:00–07:59 | myenergi `setZappiChargeMode` |
| HA | cron every 5 s | GETs ~30 HA entities → globals, POSTs 2 `input_text`s |

### 3.1. Set-Point tab

D-Bus inputs joined on `payload`: `power_consumption`, `battery_soc`, `soh`, `capacity`, `battery_power` (unused), `mppt_power_0`, `mppt_power_1`, `soltaro_power`. A `Set-Point Target` function invokes `tools.full_setpoint_flow(global, msg, node)` → `compute_payload` in `legacy/setpoint-node-red-ts/src/index.ts`. Resulting `payload.setpoint_target` is written to `/Settings/CGwacs/AcPowerSetPoint`. Wrapped in a `semaphore-plus` lock with 250 ms release.

Algorithm summary (full 1:1 port target):

- **Baseline**: setpoint defaults to 10 W (slight forced import to prevent zero-crossing discharge into Soltaro).
- **Globals**: `force_disable_export`, `export_soc_threshold`, `discharge_soc_target`, `full_charge_*`, `zappi_active`, `discharge_time` (`23:00` | `02:00`), `debug_full_charge` (`forbid` | `force` | `none`), `pessimism_multiplier_modifier`, `next_full_charge`.
- **Full-charge schedule**: `next_full_charge` tracks next Sunday 17:00; when `now ≥ next_full_charge` or `debug_full_charge=force`, `charge_to_full_required = true`, raising `soc_end_of_day_target` to `max(full_charge_discharge_soc_target, discharge_soc_target)` and `export_soc_threshold` to `full_charge_export_soc_threshold`.
- **Capacity model**: `total_capacity = capacity × soh/100 × 48 V`; `current_capacity = total × soc/100`; `end_of_day_target = total × soc_end_of_day_target/100`.
- **Branches**:
  1. `force_disable_export` → setpoint = 10.
  2. `zappi_active` → 02:00–08:00 setpoint = `10 − soltaro_export`; otherwise setpoint = `−solar_export` (dump PV, don't discharge battery).
  3. Last 5 min before midnight → setpoint = 10 (avoid feeding Soltaro during the 23:59–00:00 quirk).
  4. Evening 17:00–02:00 → discharge-toward-target controller using `pessimism_multiplier`, `to_be_consumed`, `exportable_capacity`, `preserve_battery`.
  5. Day 08:00–17:00 → PV-multiplier controller with piecewise `pv_multiplier` ladders depending on `export_soc_threshold ≤ 67` vs. `> 67`. Bad weather (`solar_export ≤ 1100 W`) clamps export.
  6. 02:00–05:00 (Boost) and 05:00–08:00 (NightExtended) → setpoint = 10.
- **Post-processing** (`_prepare_setpoint`): clamp to `max_discharge = max(-5000, -(4020 + solar_export))`, floor, round to 50 W, promote any non-negative value to 10 W.

### 3.2. Current-limit tab

Joined every 5 s from D-Bus (`grid_power`, `grid_voltage`, `grid_current`, `consumption_power`, `offgrid_power`, `offgrid_current`, `system_input_current`, `system_output_current`, `battery_soc`, `battery_power`, `mppt_power_*`, `soltaro_power`, `soltaro_current`, `zappi_current` from `evcharger/35`, `ess_state`) and globals (`zappi_current_target`, `zappi_state`, `charge_battery_extended`, `charge_car_extended`, `disable_night_grid_discharge`, `zappi_emergency_margin`, `battery_selected_soc_target`, `force_disable_export`, `prev_ess_state`).

`compute limit` function:

- Derives `zappi_active` from `zappi_mode`, plug/status, time-in-state, and `zappi_amps > 1`. Writes global.
- `available_pv_power = clamp0(mppt + soltaro − offgrid)` → grid-side amps via `grid_voltage`.
- `soltaro_inflow_power = -soltaro_power` when negative.
- `gridside_consumption_current = (consumption − offgrid + soltaro_inflow) / voltage`.
- `fit_current()` accounts for Zappi: subtracts `zappi_current_target` + `zappi_emergency_margin` when Zappi is still ramping.
- Branches on tariff band (`Boost` 02–05 / `NightExtended` 05–08 gated on `extendedChargeRequired` / Day / Evening):
  - Boost or enabled-extended: if battery charging → fitted current; elif Zappi active → `offgrid_current` (no battery drain); elif `disable_night_grid_discharge` → `offgrid_current`; else 65 A.
  - Otherwise: if Zappi active → `available_pv_power_as_gridside_amps`; elif `disable_night_grid_discharge` ∧ `NightExtended` → `offgrid_current`; else 65 A.
- `input_current_limit = clamp(0, target, 65)` → `/Ac/In/1/CurrentLimit`.

Constants: `min_system_current = 10`, `max_grid_current = 65`.

### 3.3. Charging schedules tab

Inputs: `battery_soc`; globals `charge_battery_extended`, `charge_car_extended`, `charge_to_full_required`, `disable_night_grid_discharge`, `zappi_active`, `above_soc_date`, `battery_soc_target`. Writes 10 `Schedule/Charge/{0,1}/{Start,Duration,Soc,Day,AllowDischarge}` settings.

- `battery_selected_soc_target = charge_to_full_required ? 100 : battery_soc_target`.
- Schedule 0 = Boost 02:00–05:00, fixed `days=7`, `discharge=0`, `soc=battery_soc_target`.
- Schedule 1 = NightExtended 05:00–08:00, conditional on `charge_battery_extended` / `charge_car_extended` / `zappi_active` / `above_soc_date`.

### 3.4. Weather SoC Target tab

Fires at 01:55. Reads `weather_forecast_daily[0].temperature` and `solcast_today.attributes.estimate`, plus the five `weathersoc_*` thresholds. Outputs five decisions:

- `export_threshold` ∈ {35, 50, 67, 80, 100}
- `discharge_target` ∈ {20, 30}
- `charge_target` ∈ {90, 100}
- `disable_night_grid_discharge: bool`
- `charge_battery_extended: bool`

Cascading ladder (order matters, last-write-wins):

- `today_energy > too_much` ⇒ `export_more` (threshold=50).
- `today_temp > winter ∧ today_energy > 1.5·too_much` ⇒ `export_max` (35).
- `today_temp ≤ winter` ⇒ `preserve_evening_battery` (80, discharge=30).
- `today_energy ≤ high` ⇒ `disable_export` (100); if cold ⇒ `extend_charge` (target=90) + `preserve_morning_battery`.
- `today_energy ≤ ok` ⇒ `extend_charge` + `preserve_morning_battery`.
- `today_energy ≤ low` ⇒ `charge_to_full_extended` (100).
- `charge_to_full_required ∧ ¬(today_energy ≥ high)` ⇒ `charge_to_full_extended`.

### 3.5. Zappi tab

15 s polling of myenergi. `zappi_state` function normalises to `{zappi_mode, zappi_status, zappi_plug_state, zappi_state_signature, zappi_last_change_*, amps, power, voltage, last_update}`.

At any `TariffBandKind.Night`, if `zappi_limit ≤ 65` ∧ `charged ≥ limit`, set Zappi to Off. Cron at 02:00–04:59 switches mode based on `charge_car_boost`; cron at 05:00–07:59 based on `charge_car_extended`.

### 3.6. HA tab

Pure I/O marshalling — no control logic. 5 s polling of ~30 HA entities via REST; responses routed through `parse` functions with per-knob `*_fallback` flow variables. Also POSTs `input_text.full_charge_state` and `input_text.next_full_charge` for display.

Fallback defaults encoded in the legacy parsers (preserved as **legacy baselines**; the new policy defaults in §7 supersede them):

| Knob | Legacy fallback | Range |
|---|---|---|
| `zappi_limit` | 100 | 1..100 |
| `zappi_current_target` | 9.5 | 7.5..32.5 |
| `force_disable_export` | `true` | bool |
| `battery_soc_target` | 100 | 50..100 |
| `discharge_soc_target` | 100 | 0..100 |
| `export_soc_threshold` | 100 | 2..100 |
| `discharge_time` | `'02:00'` | `02:00`/`23:00` |
| `disable_night_grid_discharge` | `false` | bool |
| `zappi_emergency_margin` | 5.0 | 0..10 |
| `debug_full_charge` | `'none'` | `forbid`/`force`/`none` |
| `pessimism_multiplier_modifier` | 1.0 | any |
| `full_charge_discharge_soc_target` | 57 | 0..100 |
| `full_charge_export_soc_threshold` | 100 | 0..100 |
| `charge_car_{extended,boost}` | `false` | bool |
| `weathersoc_winter_temperature_threshold` | 12 | 0..100 °C |
| `weathersoc_{low,ok}_energy_threshold` | 12 / 20 | 0..1000 kWh |
| `weathersoc_{high,too_much}_energy_threshold` | 80 / 80 | 0..1000 kWh |

### 3.7. Global state inventory

Cross-tab shared globals that the port must preserve:

- Knobs from HA (all rows in §3.6).
- Weather inputs: `weather_forecast_daily`, `solcast_today`.
- Computed: `charge_to_full_required`, `next_full_charge`, `soc_end_of_day_target`, `battery_selected_soc_target`, `charge_battery_extended`, `above_soc_date`, `zappi_active`, `zappi_state`, `prev_ess_state`.

---

## 4. Design principles

1. **TASS** — every controllable quantity is a quadruple `(TargetValue, TargetPhase, ActualValue, ActualFreshness)`. Business logic is a pure function `process(event, world, clock, topology) → (effects, world')`. Shell handles all I/O.
2. **No local persistence** — all persistent state lives on an external MQTT broker as retained messages. The Victron filesystem holds only the binary and a read-only config file.
3. **HA is MQTT-only** — no REST in either direction. HA reads our MQTT state; HA may publish to knob `/set` topics.
4. **Immediate actuation** — the service starts actuating with hard-coded safe defaults on cold start; MQTT connects in parallel; retained knob values overwrite defaults as they arrive.
5. **Dashboard wins vs HA** — a dashboard write suppresses HA commands arriving within the next 1 s (γ rule).
6. **No secrets in code** — credentials and site IDs live in `/data/etc/victron-controller/config.toml`, never in the source tree.
7. **Fail fast internally, be conservative externally** — assertions in the pure core; at every D-Bus / MQTT / HTTP boundary, stale/unknown values degrade to safe fallbacks, never crash the service.

---

## 5. Target architecture

### 5.1. Deployment

- MultiPlus-II GX, Venus OS (ARMv7). Cross-compile from x86-64.
- Install fully under `/data/` with a `/data/rcS.local` hook to survive Venus firmware upgrades.
- Config at `/data/etc/victron-controller/config.toml`.
- Dashboard on port **8910**, LAN-only, no auth.

### 5.2. TASS entity catalogue

**Actuated** (full quadruple):

| Entity | Target type | Command effect | Actual source |
|---|---|---|---|
| `GridSetpoint` | `i32` W | `WriteDbus(settings, /Settings/CGwacs/AcPowerSetPoint)` | D-Bus readback |
| `InputCurrentLimit` | `f32` A | `WriteDbus(vebus, /Ac/In/1/CurrentLimit)` | D-Bus readback |
| `BatteryLifeSchedule0` | `(start_s, duration_s, soc, days, allow_discharge)` | 5× `WriteDbus` on `Schedule/Charge/0/*` | D-Bus readback |
| `BatteryLifeSchedule1` | same | 5× `WriteDbus` on `Schedule/Charge/1/*` | D-Bus readback |
| `ZappiMode` | `Fast` / `Eco` / `EcoPlus` / `Off` | `CallMyenergi(setZappiChargeMode)` | myenergi `zmo` via poll |
| `EddiMode` | `Normal` / `Stopped` | `CallMyenergi(setEddiMode)` | myenergi poll |

**Knobs** (persisted as retained MQTT; target = desired; actual = last published). Full list in §7.

**Sensors** (actual only, freshness machine only):

- Battery: `soc`, `soh`, `installed_capacity`, `dc_power`.
- PV: `mppt_power_0`, `mppt_power_1`, `soltaro_power`.
- Grid: `grid_power`, `grid_voltage`, `grid_current`, `consumption_power`, `consumption_current`.
- Vebus: `offgrid_power`, `offgrid_current`, `input_current`, `output_current`, `current_limit` (readback), `ac_power_setpoint` (readback).
- EV branch (net): `evcharger_35_ac_power`, `evcharger_35_ac_current` (both signed).
- Zappi / Eddi raw state (from myenergi).
- Per-provider forecast: `today_kwh`, `tomorrow_kwh`, `intraday_shape`, `fetched_at`.
- Outdoor temperature (from MQTT).

**Derived views** (pure functions of World):

- `solar_export_w = max(0, mppt_0) + max(0, mppt_1) + max(0, soltaro) + max(0, −evcharger_35.ac_power)`.
- `zappi_active = (zappi_mode ≠ Off ∧ plug ∈ active-set ∧ status ≠ Complete ∧ ¬wait_timeout) ∨ evcharger_35.ac_power > 500 W`. The `> 500 W` form replaces the legacy `zappi_amps > 1` test, which could false-trigger on Hoymiles exports (≈ 12 A at 2.8 kW).
- `tariff_band(now)`, `charge_to_full_required`, `next_full_charge`.

### 5.3. Target phase and freshness

**Target phase**: `Unset → Pending → Commanded → Confirmed`. Collapsed `Pending`/`Commanded` for D-Bus writes. Kept separate for myenergi (HTTP can take seconds).

**Confirmation tolerances** (configurable):

| Entity | Confirm tolerance | Re-target dead-band |
|---|---|---|
| `GridSetpoint` | 50 W | 25 W |
| `InputCurrentLimit` | 0.5 A | 0.5 A |
| Schedules (per field) | exact | exact |

**Actual freshness**: `Unknown → Fresh → Stale`; `Fresh/Stale → Deprecated` on target change; `Deprecated → Fresh` on next reading.

Thresholds (configurable):

| Entity | Threshold |
|---|---|
| Local D-Bus values (battery, MPPT, grid, vebus, EV branch) | 5 s |
| Myenergi Zappi / Eddi state | 5 min |
| Forecast per provider | 6 h / 6 h / 4 h |
| Outdoor temperature (MQTT) | 5 min |

### 5.4. Target owner

Every target carries an owner. The full set:

`Unset`, `System` (safety fallback / hard-coded default / kill switch), `Dashboard`, `HaMqtt`, `WeatherSocPlanner`, `SetpointController`, `CurrentLimitController`, `ScheduleController`, `ZappiController`, `EddiController`, `FullChargeScheduler`.

**Conflict rule (γ)**: a write by `Dashboard` installs a 1 s hold against `HaMqtt` writes on the same knob. A `HaMqtt` write landing during the hold is dropped (with a log line).

### 5.5. Events and Effects

```rust
enum Event {
    DbusPropertyChanged { service, path, value, at },
    MyenergiPolled { device, state, at },
    ForecastRefreshed { provider, data, at },
    MqttMessage { topic, payload, owner, at },
    HttpApiCommand { entity, value, owner, at },
    Tick { at },                  // 1 Hz heartbeat for freshness decay
    TimerFired { timer_id, at },
}

enum Effect {
    WriteDbus { service, path, value },
    CallMyenergi { device, action },
    PublishMqttState { topic, payload, retain },
    PublishMqttDiscovery { entity, schema },
    BroadcastWsState { entities },
    ScheduleTimer { id, fire_at },
    CancelTimer { id },
    LogLine { level, module, fields },
}
```

`process` is pure: same `(event, world, clock, topology)` → same `(effects, world')`. No I/O, no wall-clock reads, no RNG.

### 5.6. Controllers (one per NR tab)

```
fn evaluate_setpoint(&World, &Topology, &dyn Clock) -> Option<(i32, Owner, SetpointDebug)>;
fn evaluate_current_limit(...)
fn evaluate_schedules(...)
fn evaluate_zappi_mode(...)
fn evaluate_eddi_mode(...)
fn evaluate_weather_soc(...)       // returns proposed knob changes owned by WeatherSocPlanner
fn evaluate_full_charge_rollover(...)
```

All re-run on every `Event` — all cheap pure code. Each returns `None` when input data is too stale / missing to decide; the phase machine stays as-is.

### 5.7. Forecasting

Three providers, all on free tiers. Sub-array geometry is configured on each provider's own dashboard, not locally — the service consumes provider-level daily totals.

- **Solcast**: free tier supports 2 rooftop sites; user approximates the 8 planes by grouping. Refresh every 2 h.
- **Forecast.Solar**: free tier is key-less, rate-limited ~12 req/h/IP; user supplies a list of representative planes. Refresh every 1 h.
- **Open-Meteo**: no key; user supplies lat/lon + representative planes. Refresh every 15 min.

All three daily totals and hourly shapes are **published to MQTT** under `victron-controller/forecast/<provider>/*` so the user can monitor each provider independently and pick a fusion strategy empirically.

Fusion strategy is a runtime knob: `max` / `mean` / `min` / `solcast_if_available_else_mean` / `weighted{…}`. Default `solcast_if_available_else_mean`.

### 5.8. EV-branch accounting

No split into Zappi-vs-Hoymiles. The net reading is authoritative:

- `evcharger/35 /Ac/Power` — signed W; `> 0` = net import (car), `< 0` = net export (Hoymiles), `≈ 0` = balanced/idle.
- `evcharger/35 /Ac/Current` — signed A.

Controller impact:

- `solar_export_w` gains a `max(0, −evcharger_35.ac_power)` term (Hoymiles through EV branch counts as PV).
- `zappi_active` fallback uses `ac_power > 500 W` instead of `ac_current > 1 A`.

No Hoymiles DTU integration — the ET112 is the low-latency source of truth.

### 5.9. Battery → Car export toggle (new)

- Knob `allow_battery_to_car` (bool). **Always boots `false`** regardless of retained MQTT value (safety reset across power cycles).
- When `true` and `zappi_active`, the Zappi-specific branch in `evaluate_setpoint` (currently `setpoint = −solar_export`) is bypassed; the usual time-of-day branch runs instead, allowing the evening controller to discharge battery into the car.
- Hard cap on battery discharge rate stays at `battery_discharge_limit = 4020 W`.
- Exposed as MQTT switch + dashboard toggle. When Zappi is Off, the flag is a no-op.

### 5.10. Eddi controller (new)

- Knobs `eddi_enable_soc` (default 96), `eddi_disable_soc` (default 94), `eddi_dwell_s` (default 60).
- **Safety direction**: default target is `Stopped`. `Normal` is only issued when battery SoC freshness is `Fresh` AND `soc ≥ eddi_enable_soc`. Unknown/stale SoC → `Stopped`.
- Hysteresis: once `Normal`, stay until `soc ≤ eddi_disable_soc` (or stale).
- Minimum dwell `eddi_dwell_s` before re-evaluating after a transition to avoid flapping.
- No time-of-day gating — pure SoC + freshness.
- Writes via `CallMyenergi(SetEddiMode, …)`; reads via `MyenergiPolled` every 5 min.

### 5.11. Grid export hard cap (new)

- Knob `grid_export_limit_w` (default 4900).
- Applied in `_prepare_setpoint` post-processing: `setpoint = max(-grid_export_limit_w, setpoint)` in addition to the existing battery-side `max_discharge = max(-5000, -(4020 + solar_export))`. The more restrictive cap wins.

### 5.12. Dashboard + MQTT bridge

- `axum` + embedded SPA (plain HTML + htmx, no build tooling on the GX).
- Port 8910, LAN-only, no auth.
- Shows: live power flows, today's forecast vs. realized, Zappi + Eddi state, every controller's debug output (same `SetpointOutputDebug` fields the NR node emits), and the full TASS world (target/actual/phase/freshness per entity).
- Every knob is writable from the dashboard (owner = `Dashboard`).
- `writes_enabled` toggle is prominent.
- MQTT bridge uses HA discovery protocol (`homeassistant/<component>/victron_controller/<id>/config` retained). Each knob becomes an HA `number`/`select`/`switch` entity; each derived state becomes a `sensor`.

### 5.13. Failure modes & safeguards

Uniform handling via freshness:

| Trigger | Rule |
|---|---|
| `battery.soc.freshness ≠ Fresh` | `evaluate_setpoint` returns `None`; `GridSetpoint` target forced to `10 W` by `System` owner. |
| `mppt_*.freshness ≠ Fresh` or `soltaro_power.freshness ≠ Fresh` | Treat that source as 0 W in `solar_export_w`. |
| All forecast providers `Stale > 12 h` | `evaluate_weather_soc` returns `None`; knobs stay. |
| All forecast providers `Stale > 48 h` | `evaluate_weather_soc` returns conservative preset, owner `System`. |
| Myenergi `Stale > 5 min` | `ZappiMode` / `EddiMode` phase frozen; no `CallMyenergi` effects. Zappi-active falls back to `ac_power > 500 W`. |
| MQTT disconnected | Shell reconnects; `process` unaffected. Outbound state publishes buffered in bounded ring, replayed on reconnect. HA goes offline; dashboard and D-Bus actuation keep working. |
| D-Bus disconnected | Shell reconnects; actuated entities transition `Fresh → Deprecated` on next target change. No retry on dead connection. |
| `writes_enabled = false` | All actuated targets forced to `Unset`; no `WriteDbus` / `CallMyenergi` emitted. |

Cold start: subscribe to D-Bus; **controllers start evaluating immediately**; `System`-owned hard-coded defaults (§7) are already installed at boot. MQTT connects in parallel; retained knobs overwrite defaults as they arrive.

---

## 6. Module layout

Two-crate workspace: pure core + async shell.

```
crates/
  core/                               // no tokio, no zbus, no reqwest
    src/
      lib.rs
      types.rs                        // Event, Effect, EntityId, Owner, TimerId
      tass/
        actuated.rs                   // Actuated<V>
        actual.rs                     // Actual<V>
        phase.rs                      // TargetPhase
        freshness.rs                  // ActualFreshness
        timestamped.rs
      world.rs                        // World struct
      topology.rs                     // Topology struct + TOML parsing
      process.rs                      // process(event, world, clock, topo)
      controllers/
        setpoint.rs                   // evaluate_setpoint (1:1 port of compute_payload)
        current_limit.rs
        schedules.rs
        zappi_mode.rs
        eddi_mode.rs
        weather_soc.rs
        next_full_charge.rs
        tariff_band.rs
      util/
        clamp.rs roundf.rs
    tests/
      setpoint_golden.rs              // Jest cases translated into Rust
      property_phase.rs
      property_freshness.rs

  shell/                              // async; all I/O lives here
    src/
      main.rs
      config.rs
      clock.rs
      bus_dbus/
        client.rs                     // zbus → DbusPropertyChanged
        writer.rs                     // Effect::WriteDbus
        discovery.rs                  // M1 service enumeration
      bus_myenergi/
        poller.rs
        writer.rs
      bus_forecast/
        open_meteo.rs forecast_solar.rs solcast.rs scheduler.rs
      bus_mqtt/
        subscriber.rs publisher.rs discovery.rs
      bus_http/
        api.rs dash.rs
      telemetry/
        log.rs metrics.rs
```

---

## 7. Hard-coded safe defaults (cold-start baseline)

These values apply on cold start before any retained MQTT knobs arrive. They are chosen to match the user's conservative 80 % policy: keep battery around 80, export above 80, schedule-only grid charging, cap grid export at 4900 W, don't discharge battery.

| Knob | Default | Meaning |
|---|---|---|
| `force_disable_export` | `false` | Allow export (gated by SoC) |
| `export_soc_threshold` | `80` | Export only when battery SoC ≥ 80 % |
| `discharge_soc_target` | `80` | Evening controller won't dip below 80 % |
| `battery_soc_target` | `80` | Nightly charge target |
| `full_charge_discharge_soc_target` | `57` | Unchanged from NR |
| `full_charge_export_soc_threshold` | `100` | Unchanged from NR |
| `discharge_time` | `'02:00'` | Unchanged |
| `debug_full_charge` | `'none'` | Weekly Sunday-17:00 rollover active |
| `pessimism_multiplier_modifier` | `1.0` | No bias |
| `disable_night_grid_discharge` | `false` | Schedules may stop charging on target-reached |
| `charge_car_boost` | `false` | |
| `charge_car_extended` | `false` | |
| `zappi_current_target` | `9.5` A | Unchanged |
| `zappi_limit` | `100` | Unchanged |
| `zappi_emergency_margin` | `5.0` A | Unchanged |
| `grid_export_limit_w` | `4900` | **NEW** grid-meter-side hard cap |
| `allow_battery_to_car` | `false` | **NEW**. Boot-resets regardless of retained value |
| `eddi_enable_soc` | `96` | **NEW** |
| `eddi_disable_soc` | `94` | **NEW** |
| `eddi_dwell_s` | `60` | **NEW** |
| `weathersoc_winter_temperature_threshold` | `12` °C | |
| `weathersoc_low_energy_threshold` | `12` kWh | |
| `weathersoc_ok_energy_threshold` | `20` kWh | |
| `weathersoc_high_energy_threshold` | `80` kWh | |
| `weathersoc_too_much_energy_threshold` | `80` kWh | |
| `writes_enabled` | `true` | |
| `forecast_disagreement_strategy` | `solcast_if_available_else_mean` | |

---

## 8. Testing strategy

- **Pure unit tests**: `(seed World, sequence of Events, expected (effects, World'))` triples. No async, no mocks, injected `Clock`. Port every Jest test from `legacy/setpoint-node-red-ts/src/__tests__/*.ts`.
- **Property tests**: QuickCheck-style invariants on `process`:
  - Target phase never skips steps.
  - Freshness never spontaneously upgrades.
  - `writes_enabled = false` ⇒ no `WriteDbus` / `CallMyenergi` effects.
  - Idempotence on "no new information" events.
- **Golden replay**: capture 24 h of live D-Bus events from the running GX into a JSONL file; replay through `process`; diff proposed `WriteDbus` effects against what NR actually wrote.
- **Integration (shell)**: test zbus bus + fake myenergi HTTP + fake MQTT broker; full shell end-to-end with a sped-up clock.
- **Observer shadow run** before cutover: `writes_enabled = false` for a week in parallel with NR. Divergence hunted via the service's own observability surface.

---

## 9. Milestones

1. **M0 — Scaffolding** (1–2 days): Nix dev shell, `core` + `shell` workspace, CI cross-compile for `armv7-unknown-linux-gnueabihf`, config + logging.
2. **M1 — TASS core skeleton + D-Bus discovery** (2–3 days): the TASS types, empty `process`, read-only D-Bus shell task, and standalone discovery scripts (`dbus-send` + shell) the user runs on the Victron to enumerate services and dump a `topology.toml` snippet.
3. **M2 — Setpoint controller port** (2–3 days): `evaluate_setpoint`, golden replay vs. NR. `writes_enabled = false`.
4. **M3 — Current-limit controller port** (2 days).
5. **M4 — Schedule controller port + Zappi night logic** (2 days).
6. **M5 — Forecast adapters + weather_soc port** (3–4 days).
7. **M6 — Myenergi poller + EV-branch accounting** (2 days).
8. **M7 — Eddi controller** (1 day).
9. **M8 — `allow_battery_to_car` toggle** (1 day).
10. **M9 — MQTT bridge with HA discovery** (2 days).
11. **M10 — Dashboard SPA** (3 days).
12. **M11 — Observer shadow run** (1 week wall clock).
13. **M12 — Cutover** (0.5 day): flip `writes_enabled = true`, disable NR tabs one by one.

---

## 10. Inventory — endpoints, credentials, external dependencies

### 10.1. MQTT broker

| Field | Value |
|---|---|
| Hostname | `mqtt.example.invalid` |
| Port | 1883 |
| TLS | NO |
| Username | mqtt |
| Password | **FILLME** |
| ClientId prefix | `victron-controller` |
| Keepalive | 30 s |
| Clean session on reconnect | false |
| Broker retention configured (`persistence true`) | yes |

Topic root: `victron-controller/`.

| Purpose | Topic pattern | Retained | Direction |
|---|---|---|---|
| Knob state | `victron-controller/knob/<name>/state` | yes | svc → all |
| Knob command | `victron-controller/knob/<name>/set` | no | all → svc |
| Entity state snapshot | `victron-controller/entity/<id>/state` | yes | svc → all |
| Derived metrics | `victron-controller/metric/<name>/state` | yes | svc → all |
| Per-provider forecast | `victron-controller/forecast/<provider>/{today_kwh,tomorrow_kwh,shape}` | yes | svc → all |
| Bookkeeping | `victron-controller/bookkeeping/{next_full_charge,above_soc_date}/state` | yes | svc → all + svc subscribes on boot |
| Kill switch | `victron-controller/writes_enabled/{state,set}` | yes | svc + all |
| Logs | `victron-controller/log/<level>/<module>` | no | svc → archiver |
| HA discovery | `homeassistant/<component>/victron_controller/<id>/config` | yes | svc → HA |

### 10.2. External MQTT inputs

| Input | Topic | Payload | Publisher |
|---|---|---|---|
| Outdoor temperature | **FILLME** | **FILLME** (number or `{temperature: N}`) | **FILLME** |
| Other values worth pulling from MQTT instead of HA? | **FILLME** | | |

### 10.3. myenergi

| Field | Value |
|---|---|
| Hub serial | 10000002 |
| API key / password | **FILLME** |
| Zappi serial | 10000001 |
| Eddi serial | 10000002 |
| Director region URL | **FILLME** (usually `https://s18.myenergi.net`) |
| Poll cadence | 15 s (default, configurable) |
| Freshness threshold | 5 min |

Write operations the service owns exclusively: `setZappiChargeMode` (Fast/Eco/EcoPlus/Off) and `setEddiMode` (Normal/Stopped).

### 10.4. Forecast providers

**Solcast**:

| Field | Value |
|---|---|
| API key | **FILLME** |
| Rooftop site IDs (≤ 2 on free tier) | **FILLME** |
| Refresh cadence | 2 h |

**Forecast.Solar**:

| Field | Value |
|---|---|
| API key (optional) | **FILLME** (leave blank for free tier) |
| Representative planes (list of `{tilt, azimuth, kwp}`) | **FILLME** |
| Refresh cadence | 1 h |

**Open-Meteo**:

| Field | Value |
|---|---|
| Latitude | **FILLME** |
| Longitude | **FILLME** |
| Representative planes | **FILLME** |
| Refresh cadence | 15 min |

### 10.5. Victron D-Bus (on-device, no credentials)

Confirmed via `scripts/discover-victron.sh` on 2026-04-21 (Venus OS v3.70, ARMv7, kernel 6.12.23-venus-8). All instance IDs match the legacy NR flow.

| Bus name | `DeviceInstance` | `ProductName` | `Mgmt.Connection` | `Position` |
|---|---|---|---|---|
| `com.victronenergy.system` | 0 | (system aggregator) | — | — |
| `com.victronenergy.battery.socketcan_can0` | 512 | Pylontech battery | CAN-bus | — |
| `com.victronenergy.solarcharger.ttyS2` | 274 | SmartSolar MPPT RS 450/200 | VE.Direct | — |
| `com.victronenergy.solarcharger.ttyUSB1` | 289 | SmartSolar MPPT RS 450/200 | USB | — |
| `com.victronenergy.pvinverter.cgwacs_ttyUSB2_mb1` (Soltaro) | 33 | PV inverter on input 1 | /dev/ttyUSB2 | **0 = AC-Input-1** |
| `com.victronenergy.grid.cgwacs_ttyUSB0_mb1` | 34 | Grid meter | /dev/ttyUSB0 | — |
| `com.victronenergy.vebus.ttyS3` | 275 | MultiPlus-II 48/5000/70-50 | VE.Bus | — |
| `com.victronenergy.evcharger.cgwacs_ttyUSB0_mb2` (EV-branch ET112) | 35 | EVSE | /dev/ttyUSB0 | **1 = AC-Output** |
| `com.victronenergy.settings` | — | — | — | — |

**Topology note** (verified against `victronenergy/dbus-modbus-client` source):

The `/Position` enum is **role-dependent and deliberately reversed** between service types. From `dbus-modbus-client/victron_em.py` (with an explicit comment in the source):

| Role | Position=0 | Position=1 | Position=2 |
|---|---|---|---|
| `pvinverter` | AC Input 1 | AC Output | AC Input 2 |
| `evcharger` / `heatpump` / `acload` | AC Output | AC Input | (treated as AC Input) |
| `grid` | (no `/Position` — grid has no position to pick) | | |

On this install both meters are on the **AC Input** side (confirmed by the user's GX console):

- Soltaro `pvinverter.cgwacs_ttyUSB2_mb1` — Position=0 ⇒ AC Input 1.
- EV-branch `evcharger.cgwacs_ttyUSB0_mb2` — Position=1 ⇒ AC Input (the evcharger-role reversal).

Consequences for the controller:

- Both Soltaro and the Zappi/Hoymiles branch are upstream of the inverter, on the grid side. Grid-side power accounting in `compute_limit` (`gridside_consumption_power = consumption - offgrid + soltaro_inflow`) is consistent with this wiring — Soltaro and Zappi contribute to the same grid-side balance.
- The legacy clamp `zappi_active && !battery_charging → offgrid_current` restricts the MultiPlus's AC-input current budget so the inverter doesn't consume grid capacity the Zappi is trying to use directly; it's about grid-side contention, not off-grid battery protection.
- `allow_battery_to_car` (SPEC §5.9) works at the **setpoint** layer: when on, it bypasses the Zappi-active clamp (which today pins the setpoint to `-solar_export`, capping export at PV output only) and lets the evening-discharge controller set a more-negative setpoint, pushing battery energy out to the grid-side bus where the Zappi naturally consumes it.

Paths we subscribe to (by role):

| Role | Bus name | Paths |
|---|---|---|
| System aggregates | `…system` | `/Ac/Consumption/L1/Power`, `/Ac/Grid/L1/Power`, `/Ac/Consumption/L1/Current` |
| Battery | `…battery.socketcan_can0` | `/Soc`, `/Soh`, `/InstalledCapacity`, `/Dc/0/Power` |
| MPPTs | `…solarcharger.ttyS2`, `…solarcharger.ttyUSB1` | `/Yield/Power` |
| Soltaro meter | `…pvinverter.cgwacs_ttyUSB2_mb1` | `/Ac/Power`, `/Ac/L1/Current` |
| Grid meter | `…grid.cgwacs_ttyUSB0_mb1` | `/Ac/L1/Power`, `/Ac/L1/Voltage`, `/Ac/L1/Current` |
| Inverter (vebus) | `…vebus.ttyS3` | `/Ac/ActiveIn/L1/I`, `/Ac/In/1/CurrentLimit`, `/Ac/Out/L1/I`, `/Ac/Out/L1/P` |
| EV-branch ET112 | `…evcharger.cgwacs_ttyUSB0_mb2` | `/Ac/Current` (signed), `/Ac/Power` (signed) |
| Settings | `…settings` | `/Settings/CGwacs/AcPowerSetPoint`, `/Settings/CGwacs/BatteryLife/State`, `/Settings/CGwacs/BatteryLife/Schedule/Charge/{0,1}/{Start,Duration,Soc,Day,AllowDischarge}` |

**Other `com.victronenergy.*` services present** (not consumed by the controller but good to have on record):

- `com.victronenergy.adc` — GX analog inputs.
- `com.victronenergy.fronius` — Fronius autodiscovery (unused — no Fronius).
- `com.victronenergy.hub4` — ESS hub (DeviceInstance=0, ProductName=ESS). May become interesting if we want to override the ESS state directly.
- `com.victronenergy.logger` — VRM portal logger.
- `com.victronenergy.modbustcp` / `com.victronenergy.modbusclient.tcp` — modbus bridges.
- `com.victronenergy.platform` — GX device info (DeviceInstance=0, ProductName=GX Device).
- `com.victronenergy.shelly` — Shelly autodiscovery.

### 10.6. Home Assistant

We consume **nothing** from HA directly. HA reads our MQTT state; HA may publish to knob `/set` topics.

| Field | Value |
|---|---|
| HA MQTT bridged to same broker (`mqtt.example.invalid`)? | yes |

### 10.7. Logging sink

Current plan: the service publishes JSON log lines to `victron-controller/log/<level>/<module>` (not retained). A small NixOS-side archiver subscribes and writes rotated files.

| Field | Value |
|---|---|
| NixOS host that runs the archiver | **FILLME** |
| Output directory | `/var/log/victron-controller/` (default) |
| Retention | 30 days rotating (default) |

### 10.8. Network egress from the GX

Outbound ports that must be reachable from the Victron:

| Target | Port | Why |
|---|---|---|
| MQTT broker | 1883 / 8883 | state + commands + logs |
| `s18.myenergi.net` (or region) | 443 | Zappi + Eddi |
| `api.solcast.com.au` | 443 | Solcast |
| `api.forecast.solar` | 443 | Forecast.Solar |
| `api.open-meteo.com` | 443 | Open-Meteo |
| NTP (ntp.org or local) | 123 | clock sync |

Firewall / VLAN constraints: none

---

## 11. Deferred decisions

- **Logging transport details** — confirmed MQTT-based, but archiver host and exact format deferred until post-M0.
- **Alerting** — none configured for M0–M12; revisit after shadow run.
- **`pessimism_multiplier_modifier`** — kept as manual knob for now. Long-term goal: derive from forecast confidence and retire the knob.
- **Metrics storage** — starts as HA MQTT sensors. If something heavier is needed later, pick at that point (VictoriaMetrics on a NixOS host is the likely path).
- **Eddi time-of-day gating** — none for now; pure SoC. Revisit if flapping observed near midnight tariff transitions.

---

## 12. Glossary of NR → new-service term mappings

- `global.X` → `World` field (if cross-controller) or `Knob.X` (if user-controllable).
- `msg.payload.X` → field on the typed `Event` variant.
- `flow.get("X_fallback")` → hard-coded default in §7.
- `semaphore-plus` lock → removed; `process` is single-threaded over the Event stream.
- `rbe` (report-by-exception) node → replaced by the TASS re-target dead-band (§5.3).
- NR `change` node chains that feed one `victron-output-*` → `Effect::WriteDbus`.
- NR HA `http request` nodes → gone. Replaced by `PublishMqttState` and `MqttMessage` events.
- MongoDB writes → gone (user explicitly excluded).
