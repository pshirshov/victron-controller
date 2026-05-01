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

## Phase A audit (2026-05-01)

### Category 1 — Synthetic rows

#### system.timezone
- **Row construction**: `web/src/render.ts:407-417` (lines 407-417, within `renderSensors`)
- **Underlying source wire field**: `snap.timezone` (string), sourced from `world.timezone.clone()` at `crates/shell/src/dashboard/convert.rs:485`
- **Source location**: `crates/shell/src/dashboard/convert.rs:485` via `timezone: world.timezone.clone()`
- **Hardcoded freshness**: "Fresh" (line 412)
- **Hardcoded cadence**: `60_000` ms (line 413)
- **Hardcoded staleness**: `120_000` ms (line 414)
- **Hardcoded origin**: `"D-Bus settings"` (line 415)
- **Comment rationale** (lines 397-404): Wire field is a plain string outside `snap.sensors`/`snap.sensors_meta`; freshness is always "Fresh" at capture time because the controller updates timezone every D-Bus reseed (~60s on the settings service per the comment); staleness/cadence pinned at static informational values since we lack a per-row timestamp.
- **Conversion plan**: Replace with typed sensor `TypedSensorString` on `WorldSnapshot.typed_sensors` with proper cadence/staleness/origin/identifier; identifier should be the D-Bus settings path (verify exact path in existing settings reader).

#### solar.sunrise
- **Row construction**: `web/src/render.ts:428-447` (lines 428-447, loop body for sunrise/sunset pair, within `renderSensors`)
- **Underlying source wire field**: `snap.sunrise_local_iso` (string | null), sourced from `fresh_sunrise_sunset(world).0` at `crates/shell/src/dashboard/convert.rs:493`
- **Source location**: `crates/shell/src/dashboard/convert.rs:593-616` (`fresh_sunrise_sunset` and `fresh_sunrise_sunset_impl`)
- **Hardcoded freshness**: Computed from null-check — "Fresh" if `value !== null`, "Stale" otherwise (line 439-440)
- **Hardcoded cadence**: `60 * 60 * 1000` ms = 1h (line 442)
- **Hardcoded staleness**: `3 * 60 * 60 * 1000` ms = 3h (line 443)
- **Hardcoded origin**: `"baseline forecast"` (line 444)
- **Comment rationale** (lines 419-423): Wire fields are `opt[str]`; null means "Stale or never observed"; freshness window (3h) is enforced server-side via `core::world::SUNRISE_SUNSET_FRESHNESS`; client reflects what it received.
- **Conversion plan**: Replace with typed sensor on `WorldSnapshot.typed_sensors` with same cadence/staleness/origin; verify the freshness window constant in `crates/core/src/world.rs` and confirm 1h cadence matches baseline forecast computation (see `crates/core/src/forecast/baseline.rs`).

#### solar.sunset
- **Row construction**: Same loop body `web/src/render.ts:428-447` (lines 428-447, within `renderSensors`)
- **Underlying source wire field**: `snap.sunset_local_iso` (string | null), sourced from `fresh_sunrise_sunset(world).1` at `crates/shell/src/dashboard/convert.rs:494`
- **Source location**: Same as sunrise (`crates/shell/src/dashboard/convert.rs:593-616`)
- **Hardcoded freshness**: Same as sunrise — computed from null-check (line 439-440)
- **Hardcoded cadence**: Same as sunrise — `60 * 60 * 1000` ms (line 442)
- **Hardcoded staleness**: Same as sunrise — `3 * 60 * 60 * 1000` ms (line 443)
- **Hardcoded origin**: Same as sunrise — `"baseline forecast"` (line 444)
- **Conversion plan**: Same as sunrise — single typed sensor or unified pair depending on type design decision (see decision lock in plan section).

### Category 2 — Hardcoded literals not in synthetic rows

#### In renderSensors (f64 sensor rows):
- `web/src/render.ts:373`: Hardcoded `<span class="dim">—</span>` for origin when `meta[name]` missing
- `web/src/render.ts:374`: Hardcoded `<span class="dim">—</span>` for cadence when `meta[name]` missing
- `web/src/render.ts:375`: Hardcoded `<span class="dim">—</span>` for staleness when `meta[name]` missing

**Note**: These are NOT problematic hardcoded literals — they are correctly conditional placeholders indicating missing metadata, not fake data values. They should remain.

#### In renderActuated:
No hardcoded freshness/cadence/staleness/origin literals. All values come from structured wire sources (ActuatedI32.actual.freshness, ActuatedF64.actual.freshness, etc.).

#### In renderDecisions, renderBookkeeping, renderCoresState, renderTimers, renderSchedule:
No hardcoded freshness/cadence/staleness/origin literals.

#### In typed-sensor rows (renderSensors, lines 477-526):
- **Cadence, staleness, origin are NOT hardcoded** — they come from structured `ts.eddi_mode.cadence_ms`, `ts.eddi_mode.staleness_ms`, `ts.eddi_mode.origin` and same for `ts.zappi` (lines 495-497, 521-523).
- **Freshness values are computed** from `ts.eddi_mode.freshness`, `ts.zappi.freshness` (lines 492, 518).

**Verdict**: No additional hardcoded literals beyond the three synthetic row sections.

### Category 3 — Other "—" placeholders with computable values

#### In renderSensors (f64 rows):
- Lines 373-375: Hardcoded `<span class="dim">—</span>` for origin/cadence/staleness when sensor lacks `sensors_meta` entry.
  - **Status**: These are legitimately missing upstream metadata (sensors that have no origin/cadence/staleness configured). Not computable; correctly rendered as "—".

#### In typed-sensor rows (renderSensors):
- Line 490: `ev.value == null ? "—" : esc(ev.value)` — correctly conditional on actual null.
- Line 505, 516: `zVal = zParts.length === 0 ? "—" : ...` — correctly conditional on missing fields.
- Line 437: `fresh ? esc(value as string) : "—"` for sunrise/sunset — correctly conditional on null.

#### In inspector popup (renderSensorBody):
- Lines 1064, 1090, 1093, 1094: Typed-sensor value cells render "—" when value is null/missing — correct.
- Lines 1067, 1088: Freshness "Unknown" renders age as "—" instead of fmtEpoch (typed-sensor rows) — **PR-TS-META-1 already landed this fix** (lines 480-482, 506-508 in renderSensors table rows).

#### In all other tables:
- renderActuated, renderDecisions, renderBookkeeping, renderCoresState, renderTimers, renderSchedule: "—" placeholders are all legitimately conditional (null values, zero durations, missing data).

**Verdict**: No computab "—" placeholders found beyond the ones already addressed in Category 4.

### Category 4 — Actual::unknown(now) boot-stamp issue

**Current state per PR-EDDI-SENSORS-1 and PR-TS-META-1**:

#### Typed-sensor rows (renderSensors):
- **Line 480-482** (eddi.mode): `const evSinceText = ev.freshness === "Unknown" ? "—" : fmtEpoch(ev.since_epoch_ms as unknown as number);`
- **Line 506-508** (zappi): `const zSinceText = z.freshness === "Unknown" ? "—" : fmtEpoch(z.since_epoch_ms as unknown as number);`
- **Line 1067** (inspector popup, eddi.mode): `const ageText = ev.freshness === "Unknown" ? "—" : esc(fmtEpoch(since));`
- **Line 1088** (inspector popup, zappi): `const ageText = z.freshness === "Unknown" ? "—" : esc(fmtEpoch(since));`

**Fix status**: PR-TS-META-1 ALREADY LANDED for typed-sensor rows (both table and inspector). These are done.

#### f64 sensor rows (renderSensors):
- **Line 384-389** (table row freshness cell):
  ```
  {
    cls: `freshness-${act.freshness}`,
    html: `${act.freshness} <span class="dim">(${fmtEpoch(
      act.since_epoch_ms as unknown as number,
    )})</span>`,
  }
  ```
  **Issue**: When `act.freshness === "Unknown"`, this still calls `fmtEpoch()` on the boot-stamp Instant, rendering "X seconds ago" instead of "—".
  **Fix needed**: Change line 386-388 to conditionally render "—" when freshness is "Unknown".

- **Line 1123-1125** (inspector popup freshness cell):
  ```
  {
    cls: `freshness-${esc(String(a.freshness))}">${esc(String(a.freshness))}</td></tr>` +
    `<tr><th>age</th><td>${esc(fmtEpoch(since))}</td></tr>` +
  ```
  **Issue**: Same boot-stamp bug — renders epoch time even when freshness is "Unknown".
  **Fix needed**: Change line 1124 to conditionally render "—" when freshness is "Unknown".

**Suggested fix shape (f64 row, table)**:
```typescript
const sinceTxt = act.freshness === "Unknown" ? "—" : fmtEpoch(act.since_epoch_ms as unknown as number);
// Then in the cell:
html: `${act.freshness} <span class="dim">(${sinceTxt})</span>`,
```

**Suggested fix shape (f64 row, inspector popup)**:
```typescript
const ageText = a.freshness === "Unknown" ? "—" : esc(fmtEpoch(since));
// Then in the section:
`<tr><th>age</th><td>${ageText}</td></tr>` +
```

**Conclusion**: The typed-sensor fix is already in place (PR-TS-META-1). The f64 rows need the same fix in two places (lines ~384-389 table, ~1124 inspector).

### Category 5 — Downstream consumers of `snap.timezone`, `snap.sunrise_local_iso`, `snap.sunset_local_iso`

#### snap.timezone consumers:
- `web/src/render.ts:405`: `const tz = (snap as unknown as { timezone?: string }).timezone ?? "Etc/UTC";`
  - **Usage**: Rendered as the value cell in the synthetic "system.timezone" row.
  - **Migration impact**: If timezone migrates to typed-sensor shape, this line must be replaced with a read from `snap.typed_sensors.timezone` or similar.

**Grep for other references**:
- Searched `web/src/*.ts` for `.timezone` and `snap.timezone` — only render.ts:405 found.
- No other consumers in the web/src tree.

#### snap.sunrise_local_iso consumers:
- `web/src/render.ts:424-425`: `const sunriseStr = (snap as unknown as { sunrise_local_iso?: string | null }).sunrise_local_iso ?? null;`
  - **Usage**: Rendered as the value cell in the synthetic "solar.sunrise" row.
  - **Migration impact**: If sunrise_local_iso migrates to typed-sensor shape, this line must be replaced with a read from `snap.typed_sensors.sunrise` or similar.

**Grep for other references**:
- Only render.ts:424-425 found.
- No other consumers in the web/src tree.

#### snap.sunset_local_iso consumers:
- `web/src/render.ts:426-427`: `const sunsetStr = (snap as unknown as { sunset_local_iso?: string | null }).sunset_local_iso ?? null;`
  - **Usage**: Rendered as the value cell in the synthetic "solar.sunset" row.
  - **Migration impact**: If sunset_local_iso migrates to typed-sensor shape, this line must be replaced with a read from `snap.typed_sensors.sunset` or similar.

**Grep for other references**:
- Only render.ts:426-427 found.
- No other consumers in the web/src tree.

**Verdict**: Highly localized consumers — only the three synthetic row construction sites in `renderSensors`. No formatters, no local-time handlers, no downstream dependencies to migrate.

### Category 5b — Inspector modal handling for synthetic sensors

**Current behavior** (renderSensorBody, lines 1023-1132):

- When inspector opens for `entityId = "system.timezone"`:
  - Skips typed-sensor branches (lines 1062-1107 — only handles "eddi.mode" and "zappi").
  - Reaches the general f64 path (line 1109: `const a = sensors[entityId]`).
  - `sensors["system.timezone"]` is undefined (it's not in snap.sensors).
  - Returns error message at line 1113: `<section><p>no sensor "system.timezone" in snapshot</p></section>`

- Same behavior for "solar.sunrise" and "solar.sunset" — both will render "no sensor" error.

**Migration impact**: After converting these to typed sensors, add explicit branches in renderSensorBody (similar to lines 1062-1084 for eddi.mode) to handle "system.timezone", "solar.sunrise", "solar.sunset" reads from `snap.typed_sensors` (or add them to a generic typed-sensor lookup if they share a common type).

### Summary of findings

| Category | Count | Status |
|----------|-------|--------|
| **Synthetic rows** | 3 | system.timezone, solar.sunrise, solar.sunset — all confirmed at expected file:line locations |
| **Hardcoded literals in synthetic rows** | 9 | 3 freshness, 3 cadence, 3 staleness (all in 6 lines) — see lines 412-415, 439-444 |
| **Hardcoded origin literals** | 2 | "D-Bus settings", "baseline forecast" — see lines 415, 444 |
| **Hardcoded cadence literals** | 2 | 60_000 ms, 3600*1000 ms — see lines 413, 442 |
| **Hardcoded staleness literals** | 2 | 120_000 ms, 10800*1000 ms — see lines 414, 443 |
| **Other "—" placeholders** | 0 | All are correctly conditional on null/missing data |
| **Actual::unknown boot-stamp in f64 rows** | 2 | Needs fix at render.ts:~384-389 (table) and ~1123-1125 (inspector) |
| **snap.timezone consumers** | 1 | render.ts:405 (synthetic row only) |
| **snap.sunrise_local_iso consumers** | 1 | render.ts:424-425 (synthetic row only) |
| **snap.sunset_local_iso consumers** | 1 | render.ts:426-427 (synthetic row only) |

### Phase B implementation plan (concrete)

1. **Add wire-model types** (if not already present):
   - Decide: single `TypedSensorString` type vs per-string typed sensors.
   - Add to `crates/shell/src/dashboard/convert.rs::typed_sensors_to_model()` (or extend if already exists).
   - Verify D-Bus settings path for timezone identifier (exact path in existing settings reader).
   - Verify baseline forecast computation period in `crates/core/src/forecast/baseline.rs`.

2. **Wire conversion in `world_to_snapshot`** (crates/shell/src/dashboard/convert.rs):
   - Extend `typed_sensors_to_model()` to include timezone, sunrise, sunset.
   - Read `world.timezone` for timezone value; wrap as Actual<String> or TypedSensorString.
   - Reuse `fresh_sunrise_sunset()` calls for sunrise/sunset (lines 493-494); wrap as typed sensors.
   - Populate cadence_ms, staleness_ms, origin, identifier per the plan-section decisions.
   - Remove or nullify the bare `timezone`, `sunrise_local_iso`, `sunset_local_iso` fields on WorldSnapshot if they're no longer needed (TBD — check if any other downstream consumers exist outside web/src).

3. **Render-side synthetic row replacement** (web/src/render.ts::renderSensors, lines 405-447):
   - Replace the inline timezone row construction (lines 407-417) with a read from `snap.typed_sensors.timezone`.
   - Replace the sunrise/sunset loop (lines 424-447) with typed-sensor reads.
   - Reuse the existing typed-sensor row template (lines 483-526) for uniform rendering.
   - Remove the inline hardcoded freshness/cadence/staleness/origin literals.

4. **Inspector modal updates** (web/src/render.ts::renderSensorBody, lines 1023-1132):
   - Add explicit branches for "system.timezone", "solar.sunrise", "solar.sunset" (or generalize the typed-sensor lookup).
   - Route to same originSection() + rawResponseSection() pattern as eddi.mode/zappi.
   - Ensure copy icon appears for identifier cells (already exists in originSection).

5. **Fix Actual::unknown boot-stamp in f64 rows** (web/src/render.ts):
   - Line ~384-389 (renderSensors table): Condition `fmtEpoch()` call on `act.freshness !== "Unknown"`.
   - Line ~1123-1125 (renderSensorBody inspector): Condition `fmtEpoch()` call on `a.freshness !== "Unknown"`.
   - Use same pattern as typed-sensor fix (lines 480-482, 1067).

6. **Test and verify**:
   - cargo test --workspace (ensure convert.rs tests pass).
   - tsc --noEmit (ensure TypeScript compiles).
   - Manual reload: system.timezone, solar.sunrise, solar.sunset rows appear with proper metadata.
   - Click inspector on each: verify modal renders origin/cadence/staleness correctly.
   - Click f64 sensor with freshness=Unknown: verify age shows "—", not epoch time.

### Open questions for orchestrator review

1. **TypedSensorString type shape**: Should timezone, sunrise, sunset all use a common `TypedSensorString` type, or should they be separate types? (Plan decision: share if they look the same.)

2. **D-Bus timezone path**: What is the exact D-Bus settings path for timezone? Should the identifier be `"com.victronenergy.settings:/Settings/System/TimeZone"` (as suggested in the plan) or something else? Please confirm before wiring.

3. **Baseline forecast cadence**: Is 1h cadence correct for sunrise/sunset? Should it match `core::world::SUNRISE_SUNSET_FRESHNESS` (3h), or is 1h the actual poll period? Verify against `crates/core/src/forecast/baseline.rs`.

4. **Wire-model backwards compat**: After converting timezone/sunrise/sunset to typed sensors, should the bare `timezone`, `sunrise_local_iso`, `sunset_local_iso` fields be removed from WorldSnapshot, or retained for compat with older dashboards? (No evidence of other consumers outside web/src, but confirm deployment strategy.)

5. **Category 5b — Inspector for synthetic sensors**: Should the three new typed sensors (timezone, sunrise, sunset) get explicit branches in `renderSensorBody`, or can they reuse a generic typed-sensor lookup if they share a common type?

6. **Per-row copy icon**: Plan doc mentions "ensure the new typed-sensor-string rows do too" — this is already handled by `originSection()` (line 1138-1157), which renders copy icon for any non-empty identifier. Confirm no action needed here.

