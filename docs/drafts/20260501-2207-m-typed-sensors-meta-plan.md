# M-TYPED-SENSORS-META — cadence/staleness/origin on typed sensors

## Origin

PR-EDDI-SENSORS-1 added two synthetic typed-sensor rows (`eddi.mode`
and `zappi`) to the dashboard sensors table. The implementation
hardcoded `—` for the cadence / staleness-after / origin columns
because `sensors_meta` is keyed by `SensorId` and typed sensors have
no `SensorId` entry. The user has now asked for the same metadata
they get on f64 sensors.

## Acceptance criteria

1. Wire model: `TypedSensorEnum` and `TypedSensorZappi` in
   `models/dashboard.baboon` extended with `cadence_ms: i64`,
   `staleness_ms: i64`, `origin: str`, `identifier: str` (matching the
   shape `sensors_meta` exposes).
2. A public freshness-threshold constant in core
   (`MYENERGI_TYPED_FRESHNESS` — Duration), wherever the existing
   `tick(at, …)` call sites already reference. If two distinct
   thresholds exist (one for eddi vs one for zappi), preserve the
   distinction.
3. `world_to_snapshot` populates the four new fields on the typed
   sensors:
   - `cadence_ms = cfg.myenergi.poll_period.as_millis() as i64`
   - `staleness_ms = MYENERGI_TYPED_FRESHNESS.as_millis() as i64` (or
     the per-sensor variant)
   - `origin = "myenergi cloud"`
   - `identifier = "cgi-jstatus-E<serial>"` / `"cgi-jstatus-Z<serial>"`
     using `cfg.myenergi.eddi_serial` / `zappi_serial`. When serial is
     `None`, fall back to the bare path string.
4. Render: `web/src/render.ts::renderSensors` synthetic typed-sensor
   rows pull from the new wire fields via `fmtDurationMs`, replacing
   the hardcoded `—`.
5. Verification:
   - cargo test --workspace
   - cargo clippy --workspace --all-targets -- -D warnings
   - tsc --noEmit -p .

## Plan layers

1. **Baboon model** — extend `TypedSensorEnum` and `TypedSensorZappi`.
   Run regen.
2. **Core constant** — `crates/core/src/world.rs` (or wherever the
   existing tick threshold lives): export `pub const
   MYENERGI_TYPED_FRESHNESS: Duration`. Update the call sites at
   `crates/core/src/process.rs` lines 1030 and 1051 to reference the
   constant if they don't already.
3. **Shell convert** — `crates/shell/src/dashboard/convert.rs`:
   thread `cfg` (or a `MetaContext` struct following the existing
   pattern) into `typed_sensors_to_model`. Populate the four fields.
4. **Web render** — `web/src/render.ts`: drop hardcoded `—` cells in
   the two typed-sensor rows; read `cadence_ms`, `staleness_ms`,
   `origin`, `identifier` from `snap.typed_sensors.<x>`. Use the
   existing `fmtDurationMs` helper. The identifier should render with
   a copy-icon if f64 rows do (look at `entityLink`/`copyIcon`).

## Risks

- **Config threading**: PR-EDDI-SENSORS-1's notes mention `MetaContext`
  for `sensors_meta`. The convert function may need its signature
  extended. If MetaContext already exists, plumb the myenergi config
  bits through it. If not, accept a small `&MyenergiConfig` parameter
  on `world_to_snapshot` callers — better than scattering ad-hoc
  references.
- **Constant alignment**: the staleness_ms reported on the wire MUST
  equal the threshold the runtime decay actually uses (`tick(at,
  threshold)` in `process.rs:1030,1051`). Mismatch advertises a freshness
  window that doesn't match runtime reality.
- **Identifier with no serial**: the myenergi config makes
  zappi/eddi_serial optional. When None, the identifier should be
  `"cgi-jstatus-E"` / `"cgi-jstatus-Z"` without trailing serial — not
  empty string and not panic.
