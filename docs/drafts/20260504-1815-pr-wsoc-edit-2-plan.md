# PR-WSOC-EDIT-2 — Plan

## 1. Goal

Rewrite the Weather-SoC widget UX from PR-WSOC-EDIT-1's 3-column
grouped layout into a fully flat 9-column × 6-row grid where every
field cell, every kWh boundary number embedded in a row label, and
the shared "12 °C" temperature header are individually clickable —
each opening a uniform single-knob-edit modal that edits one knob at
a time. The 6 inline boundary inputs strip and the multi-field
cell-edit modal disappear; their function is absorbed into the new
layout. Infrastructure consolidates around a single new
`entity-type="single-knob-edit"` arm, replacing the per-PR
`weathersoc-cell` arm and reusing the KNOB_SPEC + dirty-input save
discipline from PR-WSOC-EDIT-1. Rust side, MQTT plumbing, KNOB_SPEC
contents, the bat=100 invariant, and the drift-guard fixture are all
unchanged — pure web-UX refactor.

## Resolved open questions (orchestrator decisions)

1. **Live values in row labels (D07)** — yes; row-label boundary
   numbers come from `snap.knobs.*`, not KNOB_SPEC defaults. Operator
   edits reflect immediately on the next snapshot.
2. **Module-level dispatcher renamed** `weatherSocSendCommand` →
   `singleKnobSendCommand` (accurate to new scope).
3. **CSS** — minimal `.single-knob-edit-grid` rule mirroring
   `.cell-edit-grid`; remove orphaned `.weathersoc-boundaries*` rules
   if present.
4. **Per-cell descriptions** — fallback via column-header surrogate
   (`weathersoc.table.<field>`) is acceptable for v1; do NOT author 48
   per-cell prose entries.
5. **Sub-header surrogate ids** — `entity-type="knob"` description-only
   popups stay; existing PR-WSOC-EDIT-1 behaviour.

## 2. Sub-task breakdown

Order matters. HTML markup first (so renderer selectors match), then
renderer rewrite, then modal infrastructure rewrite, then EntityType
rename, then descriptions, then tests.

### D01 — Rewrite `<section id="weather-soc-table">` HTML markup

- File: `crates/shell/static/index.html`.
- Delete `<div class="weathersoc-boundaries"></div>` and its inner
  `<div class="weathersoc-boundaries-row">`.
- Delete the current `<thead>` (rowspan-2 Bucket / colspan-4 group
  cells / 2 inline `wsoc-col-headers` rows of 4 anchors each).
- Insert new `<thead>` matching the locked design — two rows:
  - **Row 1 (group bands)**: `<th rowspan="2">Bucket</th>` + two
    `<th colspan="4">` group cells. Warm cell text =
    `Warm (>` + clickable `12` anchor + ` °C)`; Cold cell text =
    `Cold (≤` + clickable `12` anchor + ` °C)`. Both anchors carry
    `data-entity-id="weathersoc.threshold.winter-temperature"`,
    `data-entity-type="single-knob-edit"`. The numeric `12` is a
    DISPLAY-only hint — the modal reads the live value at click
    time. Do NOT inline the live value into static HTML.
  - **Row 2 (sub-headers)**: 8 `<th>` cells — `exp/bat/dis/ext`
    warm, then `exp/bat/dis/ext` cold. Each remains an
    `entity-link mono` anchor with `data-entity-type="knob"`
    pointing at the existing dotted ids
    (`weathersoc.table.export-soc-threshold` etc.).
- Keep `<tbody></tbody>` empty. Renderer fills it.
- Section wrapper / table id (`#weather-soc-table-table`) unchanged.
- **Done when:** static markup loads with no boundaries strip; thead
  has 9 columns in row 2; each `12` anchor is discoverable by
  `[data-entity-type="single-knob-edit"]`; sub-header anchors by
  `[data-entity-type="knob"]`.

### D02 — Rewrite `buildWeatherSocTableRows` for 9-column layout

- File: `web/src/render.ts`.
- Replace the existing `buildWeatherSocTableRows` (3-cell rows: bucket
  label / warm group / cold group) with a 9-cell shape:
  - **Cell 0**: bucket-label `<td>` rendering the bucket name +
    bracketed numeric range, with each kWh number wrapped as an
    `entity-link` anchor for the matching boundary knob:
    - `VerySunny (>67.5)` — anchor on `67.5` →
      `weathersoc.threshold.energy.very-sunny`.
    - `Sunny (45–67.5)` — `45` → `…too-much`; `67.5` → `…very-sunny`.
    - `Mid (30–45)` — `30` → `…high`; `45` → `…too-much`.
    - `Low (15–30)` — `15` → `…ok`; `30` → `…high`.
    - `Dim (8–15)` — `8` → `…low`; `15` → `…ok`.
    - `VeryDim (≤8)` — `8` → `…low`.
  - `>` and `≤` symbols passed through `esc(...)`.
  - **Cells 1..4 (warm exp/bat/dis/ext)**: each its OWN `<td>`,
    wrapped as `entity-link mono` with
    `data-entity-type="single-knob-edit"`,
    `data-entity-id="weathersoc.table.<bucket-kebab>.warm.<field-kebab>"`.
    Field kebabs: `export-soc-threshold`, `battery-soc-target`,
    `discharge-soc-target`, `extended`.
  - **Cells 5..8 (cold)**: same shape, `cold` segment.
- Factor `fmtCellField(value, isBool) → string` for individual cell
  content (number with existing rounding rule; bool → `✓` or `—`).
- Drop the `wrap(kebab, temp, inline)` helper.
- Drop `WEATHER_SOC_CELL_FIELDS` if its remaining call-sites are all
  in modal-handler code being deleted in D04.
- **Done when:** 6 rows × 9 cells; every cell 1..8 contains
  `data-entity-type="single-knob-edit"`; `low` row's cell[0] contains
  anchors for both `…energy.ok` and `…energy.high`.

### D03 — Delete `renderBoundaryInputs` and the inline-input dispatcher

- File: `web/src/render.ts`.
- Delete `renderBoundaryInputs(snap)`, its module-level
  `WEATHER_SOC_BOUNDARY_INPUTS` constant, and any boundary-input click
  handler installation.
- Remove the call in `renderWeatherSocTable`.
- `weatherSocSendCommand` reference stays (renamed in D04 to
  `singleKnobSendCommand`).
- Verify `cssEscape(s)` helper — keep if still referenced by D04's
  selectors; delete if unused.
- File: `crates/shell/static/style.css` — remove orphaned
  `.weathersoc-boundaries`, `.weathersoc-boundaries-row`,
  `.weathersoc-boundary-input` rules if present.
- **Done when:** no occurrence of `weathersoc-boundary` (any case) or
  `wsoc-boundary` in `web/src/render.ts` or `crates/shell/static/`.

### D04 — Replace cell-edit modal with single-knob-edit modal

- File: `web/src/render.ts`.
- **Delete:**
  - `WEATHER_SOC_CELL_FIELDS` (4-field metadata array).
  - `wsocCellSnakeKey(entityId)`, `readWeatherSocCell(entityId, snap)`.
  - `renderWeatherSocCellModalBody(entityId, snap, bodyEl)` and the
    `cell-edit-grid` table markup.
  - `installWeatherSocCellModalHandlers(bodyEl)` and dataset
    attributes `wsocCellId`, `wsocModalHandlersInstalled`,
    `wsocLastSnap`, `wsoc-field`, `wsoc-revert`, `wsoc-default`.
  - `stampWeatherSocCellLastSnap(bodyEl)`.
  - `clearWeatherSocCellModal()`.
  - `saveWeatherSocCellEdits(entityId, bodyEl)`.
- **Add:**
  - `renderSingleKnobEditModalBody(dotted: string, snap, bodyEl):
    void` — invoked from `renderEntityModal` when
    `type === "single-knob-edit"`.
    - Look up `KNOB_SPEC[dotted]`. Missing → "Unknown knob" body.
    - Resolve current value via
      `currentValueFor(dotted, snap): number | boolean | undefined`:
      - `weathersoc.table.<bucket>.<temp>.<field>` →
        `snap.knobs.weather_soc_table.<bucket>_<temp>.<field>`
      - else → `snap.knobs[snake]` where `snake` is the canonical
        snake form (use the existing `displayNameOfTyped` /
        canonical-mapping discipline).
    - Pull description via helper `descriptionForCellKnob(dotted)`
      that strips `<bucket>.<temp>.` from cell-knob names and looks
      up the column-header surrogate; otherwise reads
      `entityDescriptions[dotted]` directly.
    - Body:
      ```html
      <section><p>Description: …</p></section>
      <table class="single-knob-edit-grid">
        <tr>
          <td>{label}</td>
          <td><input data-singleknob-field …></td>
          <td><span class="dim">[default: X]</span></td>
          <td><button data-singleknob-revert>↺</button></td>
        </tr>
      </table>
      <footer>
        <button id="single-knob-cancel">Cancel</button>
        <button id="single-knob-save">Save</button>
      </footer>
      ```
      Float/int → `<input type="number" min/max/step value>`. Bool →
      `<input type="checkbox" checked?>`.
    - First-render branch (`bodyEl.dataset.singleknobKnob !== dotted`):
      replace innerHTML, set `dataset.singleknobKnob = dotted`,
      stamp `input.dataset.singleknobLastSnap`.
    - Live-refresh branch: dirty-input rule mirrors PR-WSOC-EDIT-1 —
      skip if input has focus; for value match against
      `dataset.singleknobLastSnap`, only overwrite if user hasn't
      typed; always update stamp.
  - `installSingleKnobEditHandlers(bodyEl): void` — bind once per
    `bodyEl`, latched by
    `bodyEl.dataset.singleknobHandlersInstalled === "1"`. Click
    handler dispatches:
    - `[data-singleknob-revert]` → reset input to KNOB_SPEC default.
    - `#single-knob-cancel` → click `entity-modal-close`.
    - `#single-knob-save` → call `saveSingleKnobEdit(bodyEl)`, then
      click `entity-modal-close`.
  - `saveSingleKnobEdit(bodyEl): void` — read `dataset.singleknobKnob`,
    locate the input, dispatch ONE `SetFloatKnob` (numeric) or
    `SetBoolKnob` (checkbox) IF value differs from
    `dataset.singleknobLastSnap`. `SetFloatKnob` covers `int` knobs
    too (existing convention).
  - `clearSingleKnobEditModal(): void` exported. Mirrors
    `clearWeatherSocCellModal`'s shape — clears
    `bodyEl.dataset.singleknobKnob` and zeros `bodyEl.innerHTML` if
    a single-knob modal was open.
- Module-level `weatherSocSendCommand` renamed to
  `singleKnobSendCommand` (the modal handler is no longer wsoc-specific).
- **Done when:** no symbol with `wsoc` or `WeatherSocCell` in the
  modal-handling section of `render.ts`; the `single-knob-edit` arm
  in `renderEntityModal` covers all 56 click-targets (48 cell fields
  + 6 boundary kWh + 1 winter-temp).

### D05 — Update `renderEntityModal` dispatch + `EntityType` union

- File: `web/src/render.ts`.
- `EntityType` union: replace `"weathersoc-cell"` with
  `"single-knob-edit"`.
- `renderEntityModal` early-return arm:
  ```ts
  if (type === "single-knob-edit") {
    renderSingleKnobEditModalBody(entityId, snap, bodyEl);
    return;
  }
  ```
- **Done when:** the union has the new variant; the dispatcher
  branches on it; tsc clean.

### D06 — Wire single-knob-edit through `index.ts`

- File: `web/src/index.ts`.
- Replace `clearWeatherSocCellModal` import with
  `clearSingleKnobEditModal`; update the call in
  `closeEntityInspector`.
- `VALID_TYPES` set: replace `"weathersoc-cell"` with
  `"single-knob-edit"`.
- The 6-boundary `wsocBoundariesChanged` re-render check stays —
  the row-label kWh numbers track live boundary values, so any
  change must re-render.
- **Done when:** `clearWeatherSocCellModal` no longer referenced
  anywhere in `web/src/`; `index.ts` compiles.

### D07 — Plumb live boundary values into row labels

- File: `web/src/render.ts`.
- Signature change:
  `buildWeatherSocTableRows(table: WeatherSocTableLike, boundaries:
  WeatherSocBoundariesLike): KeyedRow[]` where
  `WeatherSocBoundariesLike = { low: number; ok: number; high:
  number; tooMuch: number; verySunny: number }`.
- Renderer `renderWeatherSocTable(snap, sendCommand)` extracts
  boundaries from `snap.knobs.*` (snake names: `weathersoc_low_…`,
  `weathersoc_ok_…`, `weathersoc_high_…`, `weathersoc_too_much_…`,
  `weathersoc_very_sunny_threshold`). KNOB_SPEC defaults are
  fallback-only.
- Numeric formatting: integer if round-trips cleanly, else
  `toFixed(1)` (existing pattern).
- **Done when:** changing `weathersoc_high_energy_threshold` via the
  modal updates "Mid (30–45)" → "Mid (30–46)" without page reload
  (operator-validated; tests cover the default values).

### D08 — Description fallback for cell knobs

- File: `web/src/render.ts` (the helper, not `descriptions.ts` —
  no edits there).
- Add helper:
  ```ts
  function descriptionForCellKnob(dotted: string): string {
    const m = dotted.match(/^weathersoc\.table\.[a-z-]+\.(?:warm|cold)\.(.+)$/);
    if (m) return entityDescriptions[`weathersoc.table.${m[1]}`] ?? "";
    return entityDescriptions[dotted] ?? "";
  }
  ```
- Used inside `renderSingleKnobEditModalBody` for the description
  section.
- **Done when:** clicking any cell `<td>` shows non-empty description
  prose; clicking a boundary kWh number / 12 °C header also shows
  non-empty text.

### D09 — `render.test.ts` updates

- File: `web/src/render.test.ts`.
- Update existing assertions:
  - `wsoc rows[i] cell count` 3 → 9.
  - `wsoc very_sunny warm cell carries entity-link wrapper` — assert
    `cells[1..4]` each include
    `data-entity-type="single-knob-edit"` and the matching dotted
    entity-id.
  - `wsoc very_sunny warm cell renders 35/100/20/—` — split into 4
    individual assertions: cell[1]="35", cell[2]="100", cell[3]="20",
    cell[4]="—".
  - Same updates for `low` and `very_dim`.
- Add new assertions:
  - **Row label boundary anchors**: for `low` row, cell[0] HTML
    contains both `…energy.ok` and `…energy.high`. Same for other
    rows per the mapping in D02.
  - **48-cell coverage**: loop the 48 (bucket × temp × field)
    triples, assert each dotted-id substring appears in the
    appropriate row's HTML.
  - **`single-knob-edit` count**: substring count = 48 across cells
    1..8 in all 6 rows.
  - **`EntityType` compile-time check**: `const _: EntityType =
    "single-knob-edit"` (and confirm `"weathersoc-cell"` is no
    longer assignable — implicit via tsc errors if the symbol is
    used).
- **Drift-guard test stays unchanged.**
- **Done when:** `tsc --noEmit -p .` passes with new assertions.

### D10 — Style.css cleanup

- File: `crates/shell/static/style.css`.
- Remove orphaned `.weathersoc-boundaries`, `.weathersoc-boundaries-row`,
  `.weathersoc-boundary-input`, `.cell-edit-grid`,
  `.wsoc-col-headers` rules (verify each is unused).
- Add minimal `.single-knob-edit-grid` block mirroring the deleted
  `.cell-edit-grid` so the modal table renders consistently.
- **Done when:** no orphaned rules; new grid renders with same
  visual weight.

## 3. Acceptance criteria

1. 9-column × 6-row layout renders the bucket / temp / field grid.
2. 48 cells + boundary kWh numbers + "12" temperature value each
   open the single-knob-edit modal pointed at the correct dotted
   knob.
3. Modal: 1 input + default hint + revert button + Save/Cancel.
4. Save dispatches one `SetFloatKnob` or `SetBoolKnob` IFF value
   differs from snapshot.
5. Boundaries strip is gone from the rendered DOM.
6. `tsc --noEmit` clean; Rust tests + clippy clean; drift guard
   passes.
7. Boundary edits reflect in row labels next snapshot, no reload.
8. Sub-headers continue to open description-only `entity-type="knob"`
   popups.
9. Memory file updated to reflect v3 UX.

## 4. Risks / unknowns

a. **Shared knob editing path for boundaries.** The 6 boundary knobs
   live on `snap.knobs.*` (snake) AND are in KNOB_SPEC. The modal
   reads via `currentValueFor(dotted, snap)` which maps dotted →
   snake. Dispatch shape (`SetFloatKnob { knob_name: dotted, … }`)
   matches what the boundary-input "set" button used to send, so
   retained MQTT topic is the same.

b. **HTML escaping in row labels.** `>` and `≤` glyphs are passed
   through `esc(...)` (existing helper); UTF-8 safe. Numbers are
   formatted by `fmtBoundary(v)` and not user input.

c. **Sub-header surrogate ids.** Anchors like
   `weathersoc.table.export-soc-threshold` have NO entry on
   `snap.knobs` — they're description-only. Existing
   `renderKnobBody` renders `value: —` for missing snapshot fields.
   Unchanged from PR-WSOC-EDIT-1.

d. **`KNOB_SPEC` lookup for surrogates.** The 4 surrogate keys are
   in KNOB_SPEC (registered at `knobs.ts:286+`). Sub-headers stay
   on `entity-type="knob"`, NOT `single-knob-edit`, so they route
   through the read-only path.

e. **`wsocBoundariesChanged` re-render.** Existing dirty-check at
   `index.ts:163-176` covers the 5 energy thresholds + winter
   temperature. Keep it intact.

## 5. Test matrix (`render.test.ts`, type-checked only)

(i) **9-column shape**: 6 rows × `cells.length === 9`.

(ii) **Row labels carry boundary anchors**:
- `rows[0]` (very_sunny) cell[0] → `…energy.very-sunny`.
- `rows[1]` (sunny) cell[0] → both `…too-much` and `…very-sunny`.
- `rows[2]` (mid) cell[0] → both `…high` and `…too-much`.
- `rows[3]` (low) cell[0] → both `…ok` and `…high`.
- `rows[4]` (dim) cell[0] → both `…low` and `…ok`.
- `rows[5]` (very_dim) cell[0] → `…low` + `≤` glyph.

(iii) **Per-field cells**: spot-check `very_sunny` warm cells (1..4)
contain dotted ids like
`weathersoc.table.very-sunny.warm.export-soc-threshold` etc., values
35/100/20/—. Same for cold cells (5..8). Spot-check `low.cold` ext =
✓; `very_dim` warm + cold ext both = ✓.

(iv) **48-cell coverage**: cartesian-product loop counts dotted-id
substring occurrences = 48.

(v) **`single-knob-edit` marker count**: 48 occurrences across
cells 1..8 of all 6 rows.

(vi) **Drift-guard**: existing tests stay green.

(vii) **Description fallback**: `descriptionForCellKnob(
"weathersoc.table.low.cold.battery-soc-target")` returns prose for
`weathersoc.table.battery-soc-target` (non-empty).

(viii) **`EntityType` union**: `const _: EntityType =
"single-knob-edit"` compiles; `"weathersoc-cell"` does not.

## 6. Verification commands

```sh
cd web && ./node_modules/.bin/tsc --noEmit -p .
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
scripts/build-web.sh
```

Manual:
- Reload dashboard → Control tab.
- Boundaries strip is gone.
- Click 67.5 in "VerySunny (>67.5)" → modal edits very-sunny knob.
- Click 12 in "Warm (>12 °C)" → modal edits winter-temp.
- Click each of 48 cells → modal opens with the right field name +
  current value, Save dispatches one knob.
- Click any sub-header (`exp/bat/dis/ext`) → description-only popup.
