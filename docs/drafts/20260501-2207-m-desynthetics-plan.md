# M-DESYNTHETICS — audit and remove dashboard synthetics

## Origin

User instruction following PR-2 scope discussion: "we should avoid
synthetics as much as we can". The dashboard sensors table currently
synthesizes three rows from non-Actual data plus hardcodes meta
literals at multiple call sites.

## Phase A — audit (synchronisation point with orchestrator)

Before any code changes, the executor produces an audit document
listing every:

- Synthetic row constructed inline in `web/src/render.ts` (not
  sourced from `snap.sensors` / `snap.typed_sensors` / `snap.actuated`
  / `snap.bookkeeping` / `snap.decisions`).
- Hardcoded freshness / cadence / staleness / origin literal in
  `web/src/render.ts` or `crates/shell/src/dashboard/convert.rs`.
- Cells rendering `—` whose underlying value is computable from
  existing state.

The list is written to a phase-A audit appendix on this plan doc, and
the executor stops before phase B for orchestrator review.

### Known synthetics (seed list — phase A may surface more)

- `system.timezone` — `web/src/render.ts:405-417` (line numbers from
  pre-PR-EDDI-SENSORS-1 grep; phase A will re-locate). Hardcoded
  freshness=Fresh, cadence=60_000ms, staleness=120_000ms, origin
  string `"D-Bus settings"`. Real source: `snap.timezone` bare string.
- `solar.sunrise` — same file, ~424. Hardcoded freshness based on
  null-check, cadence=1h, staleness=3h, origin `"baseline forecast"`.
  Real source: `snap.sunrise_local_iso` opt-string.
- `solar.sunset` — same file, just below sunrise. Same shape.
- `Actual::unknown(now)` boot-time stamp — D03 deferred from
  PR-EDDI-SENSORS-1. Every never-observed f64 sensor reads as
  "X seconds ago" instead of "—".

## Phase B — conversions (after orchestrator unblocks)

For each synthetic in the audit:

1. Add the real source to the wire model if missing (typed sensor or
   field on existing block).
2. Wire conversion in `world_to_snapshot` with proper cadence /
   staleness / origin / identifier.
3. Replace synthetic row in `render.ts` with real-data path; drop
   hardcoded literals.

### Specific conversions

- **`system.timezone`** → typed sensor on `WorldSnapshot.typed_sensors`
  with `Actual<String>` semantics (or a new `TypedSensorString` shape
  if multiple string-valued typed sensors land). cadence: D-Bus
  settings reseed cadence (verify the actual reseed period — was 300s
  pre-PR-AS-C, now 5s per the M-AS completion notes; pick whichever
  matches reality). staleness: same as other D-Bus actuator
  freshness threshold. origin: `"D-Bus settings"`. identifier:
  `"com.victronenergy.settings:/Settings/System/TimeZone"` (verify
  exact path in the existing settings reader).
- **`solar.sunrise` / `solar.sunset`** → typed sensors. cadence: 1h
  matches the baseline forecast cadence, but verify against
  `crates/core/src/forecast/baseline.rs` (or wherever sunrise/sunset
  is computed). staleness: existing `SUNRISE_SUNSET_FRESHNESS` constant
  in `core::world` per render.ts comment. origin: `"baseline
  forecast"`. identifier: probably "(computed)" or a baseline-forecast
  identifier — verify what makes sense.
- **`Actual::unknown` boot-stamp**: render-side fix only in this PR
  (the type-side change has wider blast radius — separate PR if
  surfaced as needed). When `freshness === "Unknown"`, render the
  time portion as `"—"` across all sensor rows (typed-sensor rows
  already do this from PR-EDDI-SENSORS-1). Document the rejection of
  the type-side fix in the Completed entry.

## Decisions (lock before phase B)

- **`Actual::unknown` fix layer**: render-side. Rationale: type-side
  change touches every Actual<T> call-site (`since: Option<Instant>`)
  and ripples into `tass/actual.rs::tick`, the freshness decay logic,
  and the freshness display everywhere. Out of scope for the
  desynthetics sweep; can revisit if render-side hides a real bug.
- **Single TypedSensorString vs per-string typed-sensor types**: if
  this PR adds one (timezone), use a single `TypedSensorString` shape
  parallel to `TypedSensorEnum`. If sunrise/sunset land on this PR
  too and look the same, share the type. If sunrise/sunset want a
  different shape (date-only? local-iso?), keep them separate.

## Risks

- **Audit completeness**: the reviewer specifically scrutinises that
  the audit found every synthetic. Miss one and we ship a partial
  desynthetic that's hard to clean up later.
- **`snap.timezone` consumers**: if anything else on the dashboard
  reads `snap.timezone` directly (e.g. local-time formatters), the
  typed-sensor migration has to either keep the field for compat or
  migrate the consumers in this PR. Phase A should grep for
  `snap.timezone` consumers before phase B touches the wire model.
- **Per-row copy icon**: f64 rows have a copy icon for the
  identifier; ensure the new typed-sensor-string rows do too.

## Acceptance criteria

After phase B:
- `web/src/render.ts::renderSensors` has zero `cls: "dim", html: "—"`
  cells in cadence/staleness/origin columns.
- No inline cadence/staleness/origin literal strings in
  `renderSensors`.
- `Actual::unknown(now)` boot-stamp issue resolved across all f64
  rows (matching typed-sensor behaviour from PR-EDDI-SENSORS-1) via
  the render-side approach.
- cargo test --workspace, cargo clippy, tsc --noEmit all green.
- Manual reload reports clean — note in completion entry.
