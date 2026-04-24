# Victron D-Bus Path Cadence Matrix

Generated: 2026-04-24
Research-only deliverable — no source-file changes.

## Current state (pre-research)

- Uniform `DBUS_POLL_PERIOD = 500 ms` against **every** configured service, running `GetItems` on all 9 services every tick (≈ 18 `GetItems` calls/sec). See
  [`crates/shell/src/dbus/subscriber.rs:34`](../../crates/shell/src/dbus/subscriber.rs).
- Uniform `freshness_local_dbus = 2 s` applied to every sensor that comes off the local D-Bus. See
  [`crates/core/src/topology.rs:21`](../../crates/core/src/topology.rs) (`ControllerParams::default`).
- Myenergi freshness is separate (`freshness_myenergi = 300 s`); outdoor temperature (MQTT) is 40 min.
- `freshness_outdoor_temperature` is `40 * 60 s`.
- Field observation: after deploy, the D-Bus session goes silent at ~t = 15 s; reconnect loop picks up, but the cycle repeats.

## The authoritative answers

Source-cited, distilled from Venus OS repos and wiki:

1. **Victron's own client libraries never periodically poll `GetItems`.**
   [`velib_python/dbusmonitor.py`](https://github.com/victronenergy/velib_python/blob/master/dbusmonitor.py) calls `GetItems` exactly twice per service lifetime:
   (a) during initial `_scan_dbus()`, and (b) asynchronously when a *new* service appears via `NameOwnerChanged`. Thereafter it listens to `PropertiesChanged` / `ItemsChanged` and is purely signal-driven. No re-scan timer exists.
2. **`node-red-contrib-victron` explicitly disables periodic re-polling by default.**
   [`dbus-listener.js`](https://github.com/victronenergy/node-red-contrib-victron/blob/master/src/services/dbus-listener.js):
   `this.pollInterval = 5` seconds, but the code comments `"Polling is disabled. This is the recommended configuration."` Subscriptions with `callbackPeriodically=true` only re-fire at 5 s out of an in-process **cache**, never re-hitting the bus.
3. **`dbus-mqtt` is signal-driven and batches at 1 s.**
   [`dbus_mqtt.py`](https://github.com/victronenergy/dbus-mqtt/blob/master/dbus_mqtt.py) uses `GLib.timeout_add(1000, …)` purely to coalesce outgoing MQTT publishes; the D-Bus side is pure `PropertiesChanged`.
4. **`dbus-systemcalc-py` recomputes aggregates at most once per second.**
   [`dbus_systemcalc.py`](https://github.com/victronenergy/dbus-systemcalc-py/blob/master/dbus_systemcalc.py): `_handletimertick()` is "Called on a one second timer", sets `_changed = True` on upstream signals, and calls `_updatevalues()` only when the flag is set. So **`/Ac/Consumption/*`, `/Ac/Grid/*`, `/Dc/Battery/Power` aggregates emit at ≤ 1 Hz by design** — our 500 ms poll over-samples the upstream source by 2×.
5. **FlashMQ (Venus OS MQTT broker) rate-limits itself to 3 full republishes/sec.**
   [`dbus-flashmq`](https://github.com/victronenergy/dbus-flashmq): `"dbus-flashmq will only do the full republish at most three times per second"`. Implies the overall system is *designed* around a few-Hz ceiling on whole-tree refreshes, not tens of Hz.
6. **Underlying producer rates** ([`venus/issues/789`](https://github.com/victronenergy/venus/issues/789)):
   - Solar charger: "multiple PropertiesChanged per second, one per path".
   - CGWACS / dbus-fronius: "9 signals per second per PV inverter".
   - CCGX total ceiling: "~30 signals per second" — the Cerbo/MP-II GX is similar class, driving the `ItemsChanged` redesign.
7. **`ItemsChanged`** ([`dbus-api` wiki](https://github.com/victronenergy/venus/wiki/dbus-api)) was introduced in 2021 explicitly to reduce per-signal CPU: one dict-of-paths signal per service per tick instead of N `PropertiesChanged`. Our subscriber already consumes `ItemsChanged` correctly; that part of the design is idiomatic.
8. **Write-side cadences (out of scope for this matrix but worth recording)**:
   - `/Link/ChargeCurrent`, `/Link/ChargeVoltage`, `/Ac/PowerLimit`, `/SetCurrent` all have a 60-s write-timeout fallback. That constrains the *write* cadence for those paths — not relevant for any read path we subscribe to.

**Inference:** our 500 ms broadcast-`GetItems`-to-all-9-services is unprecedented against Victron's own reference clients. The most likely cause of the t≈15 s eviction is that the D-Bus broker (or one of the data providers, which respond to every `GetItems` synchronously) is backing off an unusually chatty client. The fix isn't slower uniform polling; the fix is **stop re-polling `GetItems` entirely** and only seed (once per session, already done) + rely on `ItemsChanged` — matching what every other consumer does. A *much* longer, per-service safety-net re-seed (30 s–5 min) can handle the "stable value never changes" case.

## Per-path matrix

**The Summary table is authoritative. Detailed sections describe the source's emission cadence; staleness/reseed numbers in those sections are retained for rationale but the Summary values are what the code implements.**

Legend:
- **Update cadence (Victron)** = how often the source service *emits* an `ItemsChanged` for that path, on average, based on the data producer's documented behavior.
- **Event-driven?** = does Venus push this on change via `ItemsChanged`? (yes for essentially all `com.victronenergy.*` live measurements; settings also push on change.)
- **Criticality** = our use in the control loop — determines the tolerable max-staleness.
- **Poll cadence** = recommended *safety-net* `GetItems` interval; 0 (or "none") means "rely on `ItemsChanged` alone after initial seed".
- **Staleness window** = recommended `freshness_*` threshold for this sensor.

### Relationship between staleness window and reseed cadence

**Invariant:** `staleness > max(organic-signal-gap, safety-net-reseed-cadence)` under *healthy* operation — otherwise freshness decays spuriously while everything's fine.

Two regimes apply, driven by which mechanism is the *primary* freshness source for a given path:

**Fast metrics** (e.g. `/Ac/Grid/L1/Power`, ~1 Hz organic signals):
Signals drive freshness. The reseed is pure belt-and-suspenders — it only catches "signals were transiently down". Here `staleness ≪ reseed` is correct: if signals stop we want to FAIL FAST (e.g. 5 s) so controllers bail on stale data, even though the next safety-net reseed won't fire for another 55 s. The reseed still resets freshness whenever it lands a value, so the sensor un-Stales as soon as the signal stream recovers or the reseed succeeds — whichever comes first.

**Slow / static metrics** (e.g. `/Soh`, `/InstalledCapacity`):
Organic signals essentially never fire. The reseed IS the freshness source. Here `staleness > reseed` is mandatory: if staleness < reseed, the sensor ping-pongs Stale → Fresh → Stale on every reseed cycle and controllers bail half the time. Pick `staleness ≈ 2× reseed` for comfortable headroom.

Every entry in the matrix below should satisfy the invariant under whichever regime applies. If a future entry violates it, either the staleness is wrong (too tight for a slow path) or the reseed is wrong (too sparse for the freshness budget).

### Summary table (every subscribed path, one row)

One row per subscribed `(service, path)` in `routing_table`. Classifications:

- **Fast**: organic `ItemsChanged` signals arrive at ≥ 1 Hz when the value is moving. Signal stream drives freshness; reseed is belt-and-suspenders. Fail-fast on signal loss.
- **Slow-signalled**: organic signals arrive when the value changes, but changes are spaced seconds–minutes apart even under normal operation. Staleness must tolerate inter-change gaps.
- **Reseed-driven**: organic signals almost never fire (static or user-set values). Reseed IS the freshness source → staleness > reseed.
- **Readback**: changes only when we (or another client) write. Same as reseed-driven for freshness purposes.

| Sensor / readback | Service | Path | Classification | Reseed | Staleness |
|---|---|---|---|---|---|
| `PowerConsumption` | system | `/Ac/Consumption/L1/Power` | Fast | 60 s | 5 s |
| `ConsumptionCurrent` | system | `/Ac/Consumption/L1/Current` | Fast | 60 s | 5 s |
| `GridPower` | system | `/Ac/Grid/L1/Power` | Fast | 60 s | 5 s |
| `BatterySoc` | battery | `/Soc` | Slow-signalled (1 Hz changing, minutes idle) | 60 s | 15 s |
| `BatterySoh` | battery | `/Soh` | Reseed-driven (minutes–hours) | 300 s | 900 s |
| `BatteryInstalledCapacity` | battery | `/InstalledCapacity` | Reseed-driven (static) | 600 s | 3600 s |
| `BatteryDcPower` | battery | `/Dc/0/Power` | **Fast** (user: "updates once/sec") | 60 s | 5 s |
| `MpptPower0` | solarcharger.ttyS2 | `/Yield/Power` | Slow-signalled (sub-second sunny; idle at night) | 60 s | 30 s |
| `MpptPower1` | solarcharger.ttyUSB1 | `/Yield/Power` | Slow-signalled (same) | 60 s | 30 s |
| `SoltaroPower` | pvinverter_soltaro | `/Ac/Power` | Fast | 60 s | 5 s |
| `GridVoltage` | grid | `/Ac/L1/Voltage` | Fast (slow-moving value but regular emissions) | 60 s | 10 s |
| `GridCurrent` | grid | `/Ac/L1/Current` | Fast | 60 s | 5 s |
| `OffgridPower` | vebus | `/Ac/Out/L1/P` | Fast | 60 s | 5 s |
| `OffgridCurrent` | vebus | `/Ac/Out/L1/I` | Fast | 60 s | 5 s |
| `VebusInputCurrent` | vebus | `/Ac/ActiveIn/L1/I` | Fast | 60 s | 5 s |
| `EvchargerAcPower` | evcharger | `/Ac/Power` | Fast | 60 s | 5 s |
| `EvchargerAcCurrent` | evcharger | `/Ac/Current` | Fast | 60 s | 5 s |
| `EssState` | settings | `/Settings/CGwacs/BatteryLife/State` | Reseed-driven (user/GUI changes) | 300 s | 900 s |
| **Readback** `CurrentLimit` | vebus | `/Ac/In/1/CurrentLimit` | Readback | 120 s | 600 s |
| **Readback** `GridSetpoint` | settings | `/Settings/CGwacs/AcPowerSetPoint` | Readback | 300 s | 900 s |
| **Readback** `Schedule0.Start` | settings | `…/Schedule/Charge/0/Start` | Readback (one of 5 fields) | 300 s | 900 s |
| **Readback** `Schedule0.Duration` | settings | `…/Schedule/Charge/0/Duration` | Readback | 300 s | 900 s |
| **Readback** `Schedule0.Soc` | settings | `…/Schedule/Charge/0/Soc` | Readback | 300 s | 900 s |
| **Readback** `Schedule0.Day` | settings | `…/Schedule/Charge/0/Day` | Readback | 300 s | 900 s |
| **Readback** `Schedule0.AllowDischarge` | settings | `…/Schedule/Charge/0/AllowDischarge` | Readback | 300 s | 900 s |
| **Readback** `Schedule1.*` (5 fields) | settings | `…/Schedule/Charge/1/*` | Readback (mirror of above) | 300 s | 900 s |

Worst-case reseed load under this schedule: 9 services × ~1 call / 60 s = **0.15 GetItems/s** across all services — about 120× gentler than the current 500 ms broadcast (18/s), well below anything Victron's reference clients do, and low enough that dbus-flashmq's documented 3-republish/s ceiling isn't approached. The alternative single-path reseed (more surgical than whole-service `GetItems`) would drop load further but is harder to implement; see per-service sections below for discussion.

Every row satisfies the `staleness > max(organic-gap, reseed)` invariant:
- Fast paths: `staleness ≪ reseed` because the signal stream's ~1 Hz cadence is the freshness driver. The reseed is a safety net for signal outages — but on signal loss we want to fail fast, so staleness is tight.
- Reseed-driven and readback rows: `staleness ≈ 2–3× reseed` so the next reseed un-Stales the sensor before the current one expires.

### system — `com.victronenergy.system`

Produced by `dbus-systemcalc-py`; all values update at ≤1 Hz on the timer tick (source §4).

| Path | Update cadence (Victron) | Event-driven? | Our use (criticality) | Poll cadence | Staleness window | Source |
|---|---|---|---|---|---|---|
| `/Ac/Consumption/L1/Power` | ≤1 Hz, on-change coalesced | yes | high — setpoint input | none; re-seed 60 s | 5 s | dbus-systemcalc-py `_handletimertick` (§4) |
| `/Ac/Consumption/L1/Current` | ≤1 Hz | yes | high — current-limit input | none; re-seed 60 s | 5 s | same |
| `/Ac/Grid/L1/Power` | ≤1 Hz | yes | high — setpoint / current-limit | none; re-seed 60 s | 5 s | same |

### battery — `com.victronenergy.battery.socketcan_can0` (Pylontech via CAN)

CAN frames from Pylontech arrive at the SoC update rate of the battery BMS. Pylontech US/UP series emits SoC at roughly 1 Hz on CAN; the driver forwards on change.

| Path | Update cadence (Victron) | Event-driven? | Our use (criticality) | Poll cadence | Staleness window | Source |
|---|---|---|---|---|---|---|
| `/Soc` | ~1 Hz while changing; seconds–minutes when idle | yes | high — setpoint + Eddi | none; re-seed 60 s | **15 s** (was 2 s) | dbus wiki `com.victronenergy.battery` [wiki](https://github.com/victronenergy/venus/wiki/dbus) |
| `/Soh` | rarely — minutes to hours | yes (on change only) | low — slow aging metric | none; re-seed 300 s | **900 s** | same |
| `/InstalledCapacity` | basically static | yes (rarely) | low — constant | none; re-seed 600 s | **3600 s** | same |
| `/Dc/0/Power` | ~1 Hz (user-observed: "updates once/sec") | yes | high — matches MPPT / consumption rate | none; re-seed 60 s | **5 s** | same (user field observation) |

### solarcharger — `com.victronenergy.solarcharger.ttyS2` / `.ttyUSB1`

VE.Direct-class MPPT: emits multiple `PropertiesChanged` per second (see issue #789). `/Yield/Power` is aggregate across trackers.

| Path | Update cadence (Victron) | Event-driven? | Our use (criticality) | Poll cadence | Staleness window | Source |
|---|---|---|---|---|---|---|
| `/Yield/Power` (both instances) | sub-second while sun up; minutes at night / idle | yes | medium — solar_export term | none; re-seed 60 s | **30 s** (covers night-idle when PV=0 and no signal is emitted) | dbus wiki `com.victronenergy.solarcharger`; issue #789 |

**Note:** when PV power is zero the MPPT stops emitting `ItemsChanged` (value unchanged). Our current 2 s window flags this as Stale within 2 s and the setpoint controller will treat `/Yield/Power` as 0 via the `solar_export_w` rule in SPEC §5.13 — which is semantically correct since the value *is* zero. A 30 s window plus a 60 s safety-net re-seed prevents thrashing between Fresh(0 W) and Stale(last-known-0 W) states while still catching a truly dead service.

### pvinverter (Soltaro) — `com.victronenergy.pvinverter.cgwacs_ttyUSB2_mb1`

CGWACS (Carlo Gavazzi WACS) ET112 meter. Issue #789 quotes ~9 signals/s per CGWACS meter; that's the total, spread across multiple paths.

| Path | Update cadence (Victron) | Event-driven? | Our use (criticality) | Poll cadence | Staleness window | Source |
|---|---|---|---|---|---|---|
| `/Ac/Power` | ~1–9 Hz while flowing; seconds when idle | yes | high — solar_export, setpoint | none; re-seed 60 s | 5 s | dbus wiki `com.victronenergy.pvinverter`; issue #789 |
| `/Ac/L1/Current` | ~1–9 Hz | yes | medium — current_limit | none; re-seed 60 s | 5 s | same |

### grid — `com.victronenergy.grid.cgwacs_ttyUSB0_mb1`

CGWACS grid meter (ET112 or similar). Same producer class as the Soltaro meter.

| Path | Update cadence (Victron) | Event-driven? | Our use (criticality) | Poll cadence | Staleness window | Source |
|---|---|---|---|---|---|---|
| `/Ac/L1/Voltage` | ~1 Hz; very slow-moving | yes | low-medium — used for A↔W conversion | none; re-seed 60 s | 10 s | dbus wiki `com.victronenergy.grid`; issue #789 |
| `/Ac/L1/Current` | sub-second when loaded | yes | high — current-limit input | none; re-seed 60 s | 5 s | same |

### vebus — `com.victronenergy.vebus.ttyS3` (MultiPlus-II 48/5000)

VE.Bus inverter: produced by `mk2-dbus`/VE.Bus driver, historically chatty. Issue #789 notes VE.Bus can emit "9 signals per second".

| Path | Update cadence (Victron) | Event-driven? | Our use (criticality) | Poll cadence | Staleness window | Source |
|---|---|---|---|---|---|---|
| `/Ac/Out/L1/P` (`OffgridPower`) | sub-second when inverting | yes | high — current-limit input | none; re-seed 60 s | 5 s | dbus wiki `com.victronenergy.vebus`; issue #789 |
| `/Ac/Out/L1/I` (`OffgridCurrent`) | sub-second | yes | high | none; re-seed 60 s | 5 s | same |
| `/Ac/ActiveIn/L1/I` (`VebusInputCurrent`) | sub-second | yes | medium — diagnostic | none; re-seed 60 s | 5 s | same |
| `/Ac/In/1/CurrentLimit` (readback) | on write only (ESS writes ≤ 5 s); sparse | yes | **high — readback for TASS Confirmation** | **none; re-seed 120 s** | **600 s** (readback, not live) | dbus wiki (`/Ac/In/1/CurrentLimit` r/w) |

**Readback-path note:** `CurrentLimitReadback` is a *TASS readback*, not a live sensor. It only changes when somebody writes it. A 2 s staleness window means that if neither we nor any other consumer writes within 2 s, the readback flips to Stale — which the TASS phase machine interprets incorrectly. This is a bug in its own right: readback freshness should track "is the bus alive and reporting my last write?" with a much wider window (≥ 30 s).

### evcharger — `com.victronenergy.evcharger.cgwacs_ttyUSB0_mb2` (EV-branch ET112)

Same CGWACS meter driver as the Soltaro and grid meters.

| Path | Update cadence (Victron) | Event-driven? | Our use (criticality) | Poll cadence | Staleness window | Source |
|---|---|---|---|---|---|---|
| `/Ac/Power` (signed) | ~1–9 Hz when flowing | yes | high — zappi_active, solar_export | none; re-seed 60 s | 5 s | dbus wiki `com.victronenergy.evcharger`; SPEC §5.8 |
| `/Ac/Current` (signed) | ~1–9 Hz | yes | medium — current-limit | none; re-seed 60 s | 5 s | same |

### settings — `com.victronenergy.settings`

These paths change **only** when something writes them (us, Node-RED, GX console). They are persisted to Venus's `localsettings.py` and emit `ItemsChanged` on every write.

| Path | Update cadence (Victron) | Event-driven? | Our use (criticality) | Poll cadence | Staleness window | Source |
|---|---|---|---|---|---|---|
| `/Settings/CGwacs/AcPowerSetPoint` (readback) | on write only | yes | **high — GridSetpoint readback** | none; re-seed 300 s | **900 s** | dbus wiki; legacy NR flow writes via this path |
| `/Settings/CGwacs/BatteryLife/State` (`EssState`) | user/GUI action + rare auto-transitions | yes | medium — ESS state gate | none; re-seed 300 s | **900 s** | dbus wiki `BatteryLife/State` |
| `/Settings/CGwacs/BatteryLife/Schedule/Charge/{0,1}/{Start,Duration,Soc,Day,AllowDischarge}` (10 paths, readbacks) | on write only, very rare (≤ once/day) | yes | **high — Schedule readback** | none; re-seed 300 s | **900 s** | dbus wiki; legacy flow writes once per schedule-evaluation cycle |

**Readback-path note applies again:** schedule fields are essentially static. A 2 s freshness window means the readback is Stale ~forever after we write the schedules, which defeats the Confirmation step in the TASS phase machine. These paths *need* a staleness window measured in minutes.

## Design proposal

The evidence points to three distinct changes; they are independent and can be tackled separately.

### D1. Stop broadcasting `GetItems` every 500 ms

**Problem**: the current 500 ms `GetItems` sweep across 9 services burns ~18 calls/sec. No Victron reference client does this. Our most plausible 15 s eviction hypothesis is a broker/producer rate-limit or memory-pressure cutoff triggered by this pattern.

**Proposal**: replace the single `DBUS_POLL_PERIOD` with:
- **Per-service safety-net `GetItems`** at cadences picked from the matrix above (most services at 60 s; settings at 300 s). Round-robin across services so at most one `GetItems` call is in flight at any one time.
- Initial seed on connect remains as-is.
- Primary liveness stays on `ItemsChanged`, as today.

The worst-case `GetItems` load drops from 18/s to ~0.15/s (9 services × 1 call per 60 s averaged). This matches what `dbus-systemcalc-py` and `dbus-mqtt` do in practice.

Implementation sketch (not applied — research-only):

```rust
struct ServicePollPolicy {
    service: String,
    interval: Duration, // 60 s default, 300 s for settings
    next_due: Instant,
}
```

Then the poll arm pops the earliest-due entry, calls `GetItems`, reschedules, and loops.

### D2. Replace scalar `freshness_local_dbus` with a per-sensor table

**Superseded by the implementation** — see `SensorId::freshness_threshold` and
`ActuatedId::freshness_threshold` in `crates/core/src/types.rs`. The Summary
table above is authoritative for the values now in code.

### D3. Disentangle readback freshness from sensor freshness

**Problem**: the current `freshness_local_dbus` is used for both sensors and readbacks. Readbacks are fundamentally different: their "freshness" isn't about data age but about "has the bus told us the value since our last write?" A readback that has been `/Ac/In/1/CurrentLimit = 65` for 10 minutes is not Stale — nothing has changed.

**Proposal**: split the freshness concept for readbacks from sensors. A readback is Fresh if a readback event has arrived since the last target change (already how `Deprecated` works in SPEC §5.3); time-based Stale for readbacks should use wide windows (≥ 30 s, up to 600 s for schedules) or be removed entirely in favor of the `Deprecated`-only flow.

### D4. Drop the tight `GET_ITEMS_TIMEOUT = 2 s` — or keep it, but with fewer calls

If D1 lands, each service is hit at most once per minute, and a 2 s per-call timeout is fine (well, generous). If D1 does *not* land and we keep 500 ms broadcast, the 2 s timeout risks starving the `select!` loop as currently documented. Post-D1, no change needed.

## Rate-limit detection & response

Even with gentler per-path cadences, we can't rule out that the Venus broker (or an intermediate producer) rate-limits us for some reason — broker restart, signal-rate spike from another client, a producer process becoming briefly unresponsive. The controller must notice and respond, not silently stall.

### Signals we can observe

| Signal | Available today? | Meaning |
|---|---|---|
| `last_signal_at` age > N seconds | **yes** (PR-URGENT-20) | Every service usually emits `ItemsChanged` within tens of seconds. Total silence across all 9 for > 60 s is strong evidence of rate-limit or broker-side drop. |
| `GetItems` timeouts on ≥ 2 services within one reseed iteration | **yes** (PR-URGENT-19 + PR-URGENT-22) | Broker is refusing / dropping method calls. |
| `session_age_s` at reconnect | **yes** (PR-URGENT-20 logs) | Pattern: repeated short sessions (< 60 s) signals rate-limit. Sessions > 1 hr that suddenly drop are more likely network / socket issues. |
| zbus error variant on stream or method-call | partial | `zbus::Error` distinguishes `NameTaken`, `NoReply`, `InvalidReply`, `Failure`, `MethodError` (`org.freedesktop.DBus.Error.AccessDenied`, `LimitsExceeded`, `NoReply`). Worth inspecting the specific variant when a method call or stream fails — some of these are definitive rate-limit signals. |
| `org.freedesktop.DBus.NameOwnerChanged` for our own unique name | no | If the broker evicts our connection, it emits `NameOwnerChanged` for our bus name going away. We could listen for this and immediately trigger reconnect. |
| D-Bus broker side logs (`busctl`, `journalctl -u dbus`) | external | Cannot observe from within the controller; operator diagnostic only. |

### Classification heuristic (for logs only, not for behavior yet)

On each reconnect attempt, log a classification line based on the previous session's metrics:

```
D-Bus session ended:
  session_age_s=X
  last_signal_age_at_failure_s=Y
  poll_failures_this_session=Z
  likely_cause={"rate_limit" | "broker_restart" | "network" | "client_defect" | "unknown"}
  recommended_action={"backoff_more" | "subscribe_less" | "investigate_broker" | "reconnect_normal"}
```

Heuristic:
- `session_age < 30 s AND poll_failures > 0 AND last_signal_age > 10 s`: **rate_limit**. We hit some producer/broker limit during initial activity burst.
- `session_age > 60 s AND last_signal_age > 60 s AND poll_failures on all services`: **broker_restart** or **client_defect**. Check `dbus-daemon` logs externally.
- `session_age > 3600 s`: **network** (long-healthy connection suddenly dropping is more likely TCP/socket).
- Other: **unknown**.

This is diagnostic-only; the outer loop still just reconnects with exponential backoff. If we see `rate_limit` repeatedly in logs, that's the cue to widen cadences further.

### Response: progressive degradation (proposal)

If `likely_cause=rate_limit` triggers twice in five minutes, the next reconnect should degrade:

| Level | Change |
|---|---|
| 0 (default) | Full matrix cadences (60 s / 300 s / 600 s). |
| 1 | Drop non-essential reseeds: keep only `system`, `battery`, `vebus`. MPPT/grid/evcharger still get signal-driven updates but no reseed. |
| 2 | Narrow the match rule: only `ItemsChanged` from `system` + `battery`. Other services' updates are discarded. Controller degrades to "limp-home" with only `battery_soc + grid_power + consumption`. |
| 3 | Last resort: tear down subscription, seed once, go fully passive (no reseed, no signals). Controller runs on last-known data with long staleness windows until manually reset. |

Each level is a **monotonic** reduction in broker load. Reset to level 0 after a healthy session ≥ 10 minutes.

Implementation notes:
- Level 0 → 1 is the easiest and probably sufficient. Start there.
- Level 2 requires rewriting the match rule with a sender filter, which means rebuilding the subscription.
- Level 3 is a last-ditch fallback; we'd log alarmingly but keep the service running to dashboard clients.
- Do NOT implement this until we have evidence from the classification logs that rate_limit is the real cause. If it's client-side (zbus connection dying), degradation won't help and we'd be solving the wrong problem.

### Recommended implementation order

1. **First:** land the per-path cadence matrix (D1 + D2 + D3 from "Design proposal" below). This alone drops load 120× and almost certainly resolves the wedge.
2. **Second:** land the classification logging at each reconnect (observation only, no behavior change). Redeploy, observe for 24 h. See what `likely_cause` values appear.
3. **Third (only if needed):** land the progressive degradation machinery if logs show repeated `rate_limit` classifications.

## Open questions for the user

1. **What does the broker actually log at t=15 s?** Is it a named-owner change, a disconnect from the client side, or a server-side kick? We need `busctl monitor` or `dbus-monitor --system --profile` captured during a wedge to tell "broker evicted us" from "our client zbus connection died". If it's the latter, D1/D2/D3 won't fix it.
2. **Is the eviction deterministic at ~15 s, or only under load?** If deterministic even with `writes_enabled=false`, the cause is purely read-side traffic; if only when writes are flowing, there's a second failure mode hiding.
3. **Are 9 services × 500 ms the actual load, or does zbus batch?** Worth checking whether our `seed_service` fires the 9 calls sequentially with `await` (in which case in-flight concurrency is 1) or concurrently (in which case peak burst is 9). The code reads sequentially — confirm.
4. **Does Venus have a documented "max pending D-Bus method calls per client" limit?** Not found in the wiki or issue tracker. Would be worth filing a Victron community question with the profile capture from (1).
5. **For schedule readbacks specifically, do we actually need freshness at all?** SPEC §5.3 says `Fresh/Stale → Deprecated` on target change, which already covers "was this readback observed after our last write". A readback that's still `Fresh` from our last seed is as usable as one refreshed seconds ago — neither reflects any intervening change.
6. **`/Settings/CGwacs/BatteryLife/State` — is this the correct path for SPEC's "VbusState" reference?** The current routing (`Route::Sensor(EssState)`) treats it as a raw numeric state per wiki values 1/9/10/etc. Confirm we interpret `2..7` correctly as "actual BatteryLife states" vs. `10..12` as "Optimized without BatteryLife", since the setpoint branch doesn't currently distinguish these.
7. **Is there interest in exposing per-service/per-path poll cadences as runtime knobs (config.toml)?** Freezing them as constants matches the SPEC §4 "explicit over implicit" policy and makes property tests easier, but config flexibility aids field debug.

## Source reference index

- Victron Venus OS wiki, D-Bus reference: <https://github.com/victronenergy/venus/wiki/dbus>
- Victron Venus OS wiki, D-Bus API: <https://github.com/victronenergy/venus/wiki/dbus-api>
- `velib_python` / `dbusmonitor.py`: <https://github.com/victronenergy/velib_python/blob/master/dbusmonitor.py>
- `node-red-contrib-victron` / `dbus-listener.js`: <https://github.com/victronenergy/node-red-contrib-victron/blob/master/src/services/dbus-listener.js>
- `node-red-contrib-victron` / `victron-client.js`: <https://github.com/victronenergy/node-red-contrib-victron/blob/master/src/services/victron-client.js>
- `dbus-mqtt` / `dbus_mqtt.py`: <https://github.com/victronenergy/dbus-mqtt/blob/master/dbus_mqtt.py>
- `dbus-systemcalc-py` / `dbus_systemcalc.py`: <https://github.com/victronenergy/dbus-systemcalc-py/blob/master/dbus_systemcalc.py>
- `dbus-flashmq`: <https://github.com/victronenergy/dbus-flashmq>
- `venus/issues/789` (ItemsChanged rationale + signal-rate numbers): <https://github.com/victronenergy/venus/issues/789>
- Local repo evidence: [`crates/shell/src/dbus/subscriber.rs`](../../crates/shell/src/dbus/subscriber.rs),
  [`crates/core/src/topology.rs`](../../crates/core/src/topology.rs), [`SPEC.md`](../../SPEC.md) §5.3, §10.5;
  [`legacy/debug/20260421-120500-injects-crons.txt`](../../legacy/debug/20260421-120500-injects-crons.txt) (Node-RED inject cadences: HA 5 s, Zappi 15 s, Weather 01:55 daily — no 500 ms loops anywhere).
