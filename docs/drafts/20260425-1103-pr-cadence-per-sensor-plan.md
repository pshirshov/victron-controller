# PR-cadence-per-sensor — implementation plan

Drafted 2026-04-25 11:03 local. Supersedes the `FreshnessRegime::Fast` exemption introduced in PR-staleness-floor (commit `8785d44`).

## §1 Goal

User feedback on the M-UX-1 staleness-floor PR (verbatim): *"sensors (e.g. `battery_dc_power`) which have reseed interval 1m but staleness threshold is 5s. I thought we agreed that 1) staleness threshold must be always higher than reseed interval 2) reseed intervals for frequently updating values should be lower for frequently-updating values."*

The design call I made was wrong: I introduced a `FreshnessRegime::Fast` carve-out that exempts fast sensors from `staleness ≥ 2 × reseed_cadence`, leaving them at `staleness ≥ 5 s` while reseed stayed at 60 s. That defeats the whole point of the invariant — if signals stop AND the next reseed is 55 s away, the staleness window doesn't bound the worst-case data-staleness; it only bounds the time-to-Stale, and meanwhile we have nothing to fall back on. Tightening the reseed cadence (so the safety net actually catches the fast cases) is the right fix; the invariant becomes universal.

This PR (a) extends the cadence matrix doc with an empirically-rooted "expected organic update frequency" column, (b) replaces the two flat per-service constants with a per-sensor `reseed_cadence()` whose value drives a per-service `min(...)` lookup at scheduler-build time, (c) drops the `Fast` regime exemption, and (d) recalibrates each sensor's `freshness_threshold` and `reseed_cadence` so every sensor satisfies `staleness ≥ 2 × reseed_cadence` (1× would satisfy the user's literal "higher than", but 2× tolerates a single missed reseed — same headroom logic the slow-signalled and reseed-driven regimes already use, so making the rule uniform is simpler than introducing a new fudge factor).

## §2 Per-sensor reseed cadence — Option A vs B

**Option A — per-path D-Bus calls.** Each `(service, path)` gets its own scheduler entry; the subscriber issues `Get` (single-path) at the per-sensor cadence. Highest fidelity, but Victron's batch API is `GetItems` (returns ALL paths for a service in one round trip). Per-path means many more round trips (~20 paths vs 9 services), and a fast sensor on a service with many slow paths multiplies broker load. The current per-service `BinaryHeap<Reverse<ServiceSchedule>>` and `seed_service` would need rewriting to use `Get` rather than `GetItems`.

**Option B — per-service `GetItems` keyed by `min(reseed_cadence)` over its sensors.** Keep `seed_service` (which calls `GetItems` once and routes every returned path) intact. The scheduler entry's `interval` becomes `min(SensorId::reseed_cadence())` over all sensors that route to that service. A fast sensor's needs drive its whole service's cadence. Higher reseed traffic on services that contain a fast sensor (we'd reseed `battery` at 5–10 s instead of 60 s), but `GetItems` is one round trip regardless of path count, so the per-call cost is constant.

**Recommendation: Option B.** Justification: (1) per-call cost dominates per-path cost on this bus (`GetItems` is `<50 ms` for a healthy service, regardless of path count); (2) the `dbus-flashmq` 3-republish-per-sec ceiling and the t≈15 s eviction we already mitigated are about *call rate*, not bytes, so amortising the fast sensor's cadence across the service's other paths is free observability; (3) it's a surgical change — only `reseed_interval_for` (`subscriber.rs:376-382`) needs new logic, plus a way for the shell to look up `min(reseed_cadence)` per service. Option A would require a refactor of `seed_service`, the `ServiceSchedule` struct, and the routing table — out of scope for a "fix the invariant" PR.

**Cost quantified.** Today's worst-case is `8 services × 1/60 s + 1 service × 1/300 s ≈ 0.137 GetItems/s`. Under Option B with the proposed fast-sensor cadences below, the `battery`, `system`, `grid`, `vebus`, `pvinverter_soltaro`, `evcharger` services drop from 60 s to ~5 s; `mppt_0`/`mppt_1` drop to ~15 s; settings stays 300 s. New worst case: `6 × 1/5 + 2 × 1/15 + 1 × 1/300 ≈ 1.34 GetItems/s`. About 10× the old reseed load, still 30× gentler than the original pre-cadence-matrix 18/s broadcast — well below the 3 republish/s ceiling.

## §3 Audit table

Authoritative numbers. "Organic" sources cited: VW = Victron wiki, I789 = `venus/issues/789`, U = user field observation. Empirical / inferred entries flagged "(empirical)".

| SensorId | Service | Organic update freq | Cur reseed | Cur staleness | Proposed reseed | Proposed staleness | Action |
|---|---|---|---|---|---|---|---|
| `BatteryDcPower` | battery | ~1 Hz (U) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `BatterySoc` | battery | ~1 Hz changing / minutes idle (VW) | 60 s | 120 s | 60 s | 120 s | no change |
| `BatterySoh` | battery | minutes–hours (VW) | 60 s | 900 s | 60 s | 900 s | no change |
| `BatteryInstalledCapacity` | battery | static (VW) | 60 s | 3600 s | 60 s | 3600 s | no change |
| `PowerConsumption` | system | ≤1 Hz (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `ConsumptionCurrent` | system | ≤1 Hz (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `GridPower` | system | ≤1 Hz (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `GridVoltage` | grid | ~1 Hz, slow-moving | 60 s | 10 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `GridCurrent` | grid | sub-second when loaded (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `OffgridPower` | vebus | sub-second when inverting (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `OffgridCurrent` | vebus | sub-second (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `VebusInputCurrent` | vebus | sub-second (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `SoltaroPower` | pvinverter_soltaro | ~1–9 Hz when flowing (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `EvchargerAcPower` | evcharger | ~1–9 Hz when flowing (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `EvchargerAcCurrent` | evcharger | ~1–9 Hz (I789) | 60 s | 5 s | **5 s** | **15 s** | lower reseed, raise staleness |
| `MpptPower0` | solarcharger.ttyS2 | sub-second sunny / silent night | 60 s | 30 s | **15 s** | **30 s** | lower reseed; staleness 30 s satisfies 2×15 |
| `MpptPower1` | solarcharger.ttyUSB1 | same | 60 s | 30 s | **15 s** | **30 s** | same |
| `EssState` | settings | rare, on user/GUI action (VW) | 300 s | 900 s | 300 s | 900 s | no change |
| `OutdoorTemperature` | (Open-Meteo) | 30 min poll (external) | 30 min | 40 min | 30 min | 40 min | no change (external-polled grace model) |
| `SessionKwh` | (myenergi cloud) | 5 min poll (external) | 300 s | 600 s | 300 s | 600 s | no change |

Per-service `min(reseed_cadence)` (Option B): `battery=5s`, `system=5s`, `grid=5s`, `vebus=5s`, `pvinverter_soltaro=5s`, `evcharger=5s`, `mppt_0=15s`, `mppt_1=15s`, `settings=300s`.

The 5 s fast-service cadence is well above `dbus-flashmq`'s 3 republish/s ceiling per service, but `GetItems` is a method call not a republish — distinct rate-limit. Initial deploy should be monitored for the t≈15 s eviction signature; if it returns we widen to 10 s and accept tighter ping-pong tolerance via slightly larger staleness (30 s).

## §4 Subscriber changes

1. **`crates/shell/src/dbus/subscriber.rs`**:
   - Drop `pub const SEED_INTERVAL_DEFAULT` and `pub const SEED_INTERVAL_SETTINGS`. Replace with a per-service map computed from `routes` at `Subscriber::new`.
   - Replace `reseed_interval_for(&self, service: &str) -> Duration` with a lookup into that map. Fall back to a sane default (60 s) only if a service is in `service_set` but no `SensorId`/actuated readback routes to it (defensive).
   - The `routing_table` includes routes for actuated readbacks (`GridSetpointReadback`, `CurrentLimitReadback`, `ScheduleField{...}`). Their reseed cadence comes from `ActuatedId::freshness_threshold / 2` — the helper that computes per-service min should iterate `routing_table` and inspect each `Route` variant: for `Route::Sensor(id)` use `id.reseed_cadence()`; for actuated readbacks use a small static table embedded next to `routing_table`. **Avoid** adding a parallel `ActuatedId::reseed_cadence()` API.
   - The startup info log currently logs `default_reseed_s` and `settings_reseed_s`. Replace with one line per service showing computed cadence — handy for field-debug parity.

2. **`crates/shell/src/dashboard/convert.rs`**:
   - Drop the `SEED_INTERVAL_DEFAULT` / `SEED_INTERVAL_SETTINGS` imports. Look up cadence per `SensorId` via `id.reseed_cadence()`.

3. **`crates/core/src/types.rs`**:
   - `SensorId::reseed_cadence` arms updated per the audit table.
   - `SensorId::freshness_threshold` arms updated per the audit table.
   - DELETE `FreshnessRegime::Fast` and `FAST_REGIME_STALENESS_FLOOR`. Single universal rule.
   - `is_external_polled` retained for `OutdoorTemperature` and `SessionKwh`.

## §5 Test changes

1. **`freshness_threshold_invariant_holds_for_every_sensor`** (`types.rs`):
   - Drop the `(FreshnessRegime, Duration)` parallel match table — call `regime()` and `reseed_cadence()` directly.
   - Drop the `FreshnessRegime::Fast` branch. Universal rule: `staleness >= 2 × reseed_cadence` for non-external-polled; `staleness >= cadence + 1 s` for external-polled.
   - Cross-check `id.reseed_cadence()` values against the matrix doc cadences explicitly via per-variant assertions.

2. **`check_staleness_invariant`** (runtime startup): unchanged on call-site; the helper itself drops its `Fast` arm.

3. **New regression test** in `subscriber.rs` tests module: assert that for every `(service)` in the routing table, the computed per-service min cadence equals the `min` over sensors+actuated routing to it.

4. **New regression test** in `types.rs`: assert that for every fast-organic sensor (5 s reseed), `freshness_threshold ≥ 2 × reseed_cadence`. Property-test form of the universal rule.

5. **Snapshot-test parity**: run `cargo insta review` (or equivalent) after the cadence changes — expected diffs are mechanical (cadence_ms numbers).

## §6 Risks

- **D-Bus traffic increase.** Worst-case `GetItems/s` rises from `~0.137` to `~1.34` — about 10×. Below the original 18/s and below the documented 3-republish/s broker ceiling, but monitor for the t≈15 s wedge signature. Mitigation: stage deploy; if eviction returns, widen all 5 s services to 10 s (staleness becomes 25 s) — one-line change.
- **Reseed-driven jitter on fast paths.** The signal stream and reseed will co-drive; duplicate `SensorReading` events with different `at` timestamps for the same value. The core's HA-publish dedup is `f64::to_bits`-based, but `process.rs::apply_tick`'s freshness clock just resets — should be fine. Verify during impl.
- **MPPT silent-at-night.** 15 s reseed when PV=0 is cheap (service alive, no organic signal); confirms staleness model rather than thrashing it.
- **`FreshnessRegime::Fast` removal.** Grep for callers before deleting; only `types.rs` and the test reference it today.
- **Honesty / TASS / wire format unaffected** per the constraints.

## §7 Open questions

(Pre-decided by parent agent before launching executor:)
- **Q1 (decided)**: 5 s reseed floor for fast-organic services. We can widen to 10 s if field eviction returns.
- **Q2 (decided)**: Delete `FreshnessRegime::Fast` entirely. Single rule.

## Critical files

- `crates/core/src/types.rs`
- `crates/shell/src/dbus/subscriber.rs`
- `crates/shell/src/runtime.rs`
- `crates/shell/src/dashboard/convert.rs`
- `docs/drafts/20260424-1959-victron-dbus-cadence-matrix.md`
