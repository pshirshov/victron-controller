# PR-WSOC-EDIT-1 — Plan

## 1. Goal

Promote the read-only Weather-SoC table widget from PR-WSOC-TABLE-1
into a fully editable control surface and relocate it to the Control
tab. Each of the 12 cells × 4 fields = 48 fields becomes an
individually addressable flat knob (MQTT topic, HA entity, KNOB_SPEC
entry, dashboard popup editor) carried by ONE associated-data
`KnobId::WeathersocTableCell { bucket, temp, field }` variant — every
plumbing layer gets a single programmatic match arm rather than 48
hand-rolled lines. Default `bat` is normalised to 100 in three cells
(Low.cold, Dim.warm, Dim.cold), severing the previous coupling between
`extended=true` and `bat=90`. The 6 already-flat boundary knobs
(`weathersoc.threshold.energy.{low,ok,high,too-much,very-sunny}`,
`weathersoc.threshold.winter-temperature`) are hidden from the generic
Knobs tables and re-rendered inline in the widget. Column headers
(`exp / bat / dis / ext`) become entity-link clickable, opening the
existing entity-inspector modal with new descriptions. The widget is
moved from the Detail panel to the Control panel above
`<section id="knobs">`.

## Resolved open questions (orchestrator decisions)

1. **`knob_name` return type → `String`** (mechanical cascade through
   `discovery.rs::publish_knobs`, `process.rs::all_knob_publish_payloads`,
   any encoder paths). Existing arms add `.to_string()`.
2. **`renderWeatherSocTable(snap, sendCommand)`** — widen the signature
   to take the dispatch capability for boundary-input "set" buttons
   and the cell-modal Save handler.
3. **Reuse `#entity-modal`** with a new `"weathersoc-cell"`
   `EntityType` arm whose body renderer is the cell-edit form. Single
   modal infrastructure; the inspector vocabulary picks up one new
   interactive type. Document the convention.
4. **3-column grouped layout** (`Bucket | Warm group | Cold group`),
   each group cell rendering 4 inline values. Click any group cell →
   modal edits all 4 fields at once. Per-field clicks are less
   informative since the modal touches all four anyway.
5. **Rust→TS drift guard** (option (a)): export a JSON fixture from a
   Rust test that snapshots `Knobs::safe_defaults().weather_soc_table`,
   commit it, consume it in `render.test.ts` to assert
   `WEATHER_SOC_DEFAULTS` matches.

## 2. Sub-task breakdown

### D01 — Update bat defaults to 100 in three cells

- File: `crates/core/src/knobs.rs::WeatherSocTable::safe_defaults`.
- Change: `low_cold.battery_soc_target` 90 → 100;
  `dim_warm.battery_soc_target` 90 → 100;
  `dim_cold.battery_soc_target` 90 → 100. The `extended` bools stay
  `true` for all three. Update the rustdoc default-table at the top of
  `WeatherSocTable::safe_defaults` to reflect the new values (3
  entries change in the doc table).
- Update sibling rustdoc on `weather_soc.rs` module top so Low.cold /
  Dim.warm / Dim.cold all show `100` for `bat`. Strike "extended ⇒
  bat=90" implication wherever it appears in prose.
- Update test `weather_soc_table_default_cells` in
  `crates/core/src/knobs.rs`: three `expect(...)` calls change to
  `bat=100` in those cells.
- **Done when:** `cargo test -p victron-controller-core
  knobs::tests::weather_soc_table_default_cells` passes with the new
  asserts.

### D02 — Adjust three retained cascade-equivalence tests

- File: `crates/core/src/controllers/weather_soc.rs` (test module).
- Tests touched:
  `cold_and_low_energy_extends_charge_and_preserves_morning` (was
  asserting `bat == 90.0`),
  `ok_or_below_always_extends_charge_regardless_of_temp` (was
  asserting `bat == 90.0`). Both now assert `bat == 100.0` with a
  per-test comment block:
  > Operator preference 2026-05-04 (PR-WSOC-EDIT-1): Low.cold / Dim
  > cells charge to 100, not 90; the extended bit no longer implies a
  > 90 % cap.
- `very_low_energy_forces_charge_to_full` already asserts `bat ==
  100.0` and stays.
- **Done when:** cascade-equivalence tests pass with bat=100
  expectations.

### D03 — Adjust 12-cell pinning tests

- File: `crates/core/src/controllers/weather_soc.rs` (test module).
- Touched: `cell_low_cold` (tuple changes from
  `(100,90,30,true,true)` to `(100,100,30,true,true)`),
  `cell_dim_warm` (same), `cell_dim_cold` (same). The
  `override_dim_warm_cf_true_extended_already_true` test stays
  unchanged — its assertion `bat=100` was already correct.
- **Done when:** all 12 `cell_*` tests pass; the 4 override/boundary
  tests still pass.

### D04 — `KnobId::WeathersocTableCell` variant + sibling enums

- File: `crates/core/src/types.rs`.
- Add `KnobId::WeathersocTableCell { bucket: EnergyBucket, temp:
  TempCol, field: CellField }`.
- Add `TempCol { Warm, Cold }` and `CellField { ExportSocThreshold,
  BatterySocTarget, DischargeSocTarget, Extended }` alongside
  `EnergyBucket` (which already lives in
  `crates/core/src/controllers/weather_soc.rs`). Either: (a) move
  `EnergyBucket` into a fresh `crates/core/src/weather_soc_addr.rs`
  and add the two siblings there, then re-export; or (b) keep
  `EnergyBucket` in `weather_soc.rs` and define `TempCol` + `CellField`
  in `types.rs` next to `KnobId`. Recommend (a): one module owns the
  cell-addressing vocabulary.
- Add `EnergyBucket::ALL`, `TempCol::ALL`, `CellField::ALL` const
  slices for cartesian-product enumeration in downstream layers.
- Add helper methods on each: `kebab(self) -> &'static str` returning
  the kebab-case wire token (`very-sunny`, `cold`,
  `export-soc-threshold` …) — single source of truth used by
  `knob_name`, the dashboard convert layer, and the TS knob-name
  generator.
- Add the inverse `from_kebab(&str) -> Option<Self>` for parsing.
- **Done when:** core compiles; `cargo test -p victron-controller-core`
  passes.

### D05 — Programmatic apply_knob arm + helper

- File: `crates/core/src/process.rs::apply_knob`.
- Add a helper `pub(super) fn cell_mut(table: &mut WeatherSocTable,
  bucket: EnergyBucket, temp: TempCol) -> &mut WeatherSocCell` (12-arm
  match → mutable borrow) co-located with `pick_cell` in
  `weather_soc.rs` if borrow-checking allows; else inline in
  `process.rs`.
- Add ONE arm to `apply_knob`'s match, dispatching on (field, value)
  to mutate the right cell field with a `replace`-then-compare pattern
  for change detection.
- Add programmatic enumeration to `all_knob_publish_payloads(&Knobs)`:
  `for bucket in EnergyBucket::ALL { for temp in TempCol::ALL { for
  field in CellField::ALL { push (KnobId::WeathersocTableCell{…},
  KnobValue) } } }`.
- **Done when:** existing `cargo test` plus new tests
  `apply_knob_weathersoc_table_cell_routes_to_field` (≥4 tests, one
  per CellField) and
  `all_knob_publish_payloads_includes_48_weathersoc_table_cells`
  pass.

### D06 — Programmatic MQTT serialize plumbing

- File: `crates/shell/src/mqtt/serialize.rs`.
- `knob_name`: switch return type from `&'static str` to `String`;
  legacy arms wrap with `.to_string()`. New arm:
  `KnobId::WeathersocTableCell { bucket, temp, field } =>
  format!("weathersoc.table.{}.{}.{}", bucket.kebab(), temp.kebab(),
  field.kebab())`.
- `knob_id_from_name`: add a parser that splits on `.`, matches the
  prefix `["weathersoc", "table", _, _, _]`, and uses the
  `from_kebab` helpers.
- `knob_range`: programmatic arm — match
  `KnobId::WeathersocTableCell { field, .. }` and dispatch by `field`
  (`ExportSocThreshold | BatterySocTarget | DischargeSocTarget =>
  Some((0.0, 100.0)); Extended => None`).
- `parse_knob_value`: programmatic arm — `field` matches to either
  `parse_ranged_float(...).map(KnobValue::Float)` or
  `parse_bool(...).map(KnobValue::Bool)`.
- Audit all `knob_name(...)` call-sites for the `&'static str` →
  `String` change.
- **Done when:** new test
  `knob_name_round_trips_for_weathersoc_table_cell` round-trips all 48
  cell knobs through `knob_name → knob_id_from_name`.

### D07 — HA discovery for 48 cell knobs

- File: `crates/shell/src/mqtt/discovery.rs`.
- Add `weathersoc_table_knob_schemas() -> Vec<(KnobId, &'static str,
  serde_json::Value)>` enumerating
  `EnergyBucket::ALL × TempCol::ALL × CellField::ALL`. Float fields use
  `number_knob(id, 1.0, Some("%"))`; `Extended` produces `(id,
  "switch", json!({"payload_on": "true", "payload_off": "false"}))`.
- Splice into `knob_schemas()`.
- **Done when:** new test
  `knob_schemas_includes_all_48_weathersoc_table_cells` asserts each
  triple appears with the right component.

### D08 — config.toml seeding: NOT TOUCHED for cell knobs

- File: `crates/shell/src/config.rs`.
- No new fields. Add a one-line comment near `[knobs]` cross-reference
  noting that the 48 cell knobs are intentionally not seedable from
  config.toml — boot defaults flow from `Knobs::safe_defaults()` and
  runtime state from retained MQTT.
- **Done when:** visual inspection.

### D09 — Dashboard convert: programmatic name parser arm

- File: `crates/shell/src/dashboard/convert.rs::knob_id_from_name`.
- Add a parser branch for `weathersoc.table.<bucket>.<temp>.<field>`
  mirroring the MQTT-layer parser. `knobs_to_model` already wires
  `weather_soc_table` — no change.
- **Done when:** unit test in `convert.rs` round-trips a cell-knob
  dotted name through the dashboard parser.

### D10 — Web: KNOB_SPEC + DISPLAY_NAMES generator + drift guard

- Files: `web/src/knobs.ts`, `web/src/displayNames.ts`.
- In `knobs.ts`, add a generator helper that emits 48 KNOB_SPEC
  entries from a `WEATHER_SOC_DEFAULTS` lookup table (mirror of
  `Knobs::safe_defaults().weather_soc_table`):

  ```ts
  const WEATHER_SOC_BUCKETS_FOR_KNOBS = ["very-sunny", "sunny", "mid", "low", "dim", "very-dim"] as const;
  const WEATHER_SOC_TEMPS = ["warm", "cold"] as const;
  const WEATHER_SOC_DEFAULTS: Record<string, [number, number, number, boolean]> = {
    "very-sunny.warm": [35, 100, 20, false],
    "very-sunny.cold": [80, 100, 30, false],
    // … 12 entries; bat=100 across all extended cells per D01.
  };
  ```

- Pick `category: "config"` and `group: "Weather-SoC table"` (NEW
  group not in `CONFIG_GROUPS` — see D11; the group is referenced for
  KNOB_SPEC consistency only, not for `renderKnobs`).
- In `displayNames.ts`, generate 48 snake_case → dotted-name entries
  programmatically.
- **Drift guard:** add a Rust test (in
  `crates/core/src/knobs.rs`) that serialises
  `WeatherSocTable::safe_defaults()` to JSON at a known fixture path
  (e.g. `web/test-fixtures/weather-soc-defaults.json`); add a TS test
  that reads the fixture and asserts `WEATHER_SOC_DEFAULTS` matches.
- **Done when:** `tsc --noEmit -p .` clean;
  `KNOB_SPEC["weathersoc.table.very-sunny.warm.export-soc-threshold"]`
  resolves with `default: 35`; drift-guard test passes.

### D11 — Web: hide boundary + cell knobs from `renderKnobs`

- File: `web/src/knobs.ts`.
- Introduce `WIDGET_RENDERED_KNOBS = new Set([…])` (parallel to the
  existing `NESTED_KNOB_FIELDS`) populated with the 6 boundary-knob
  snake_case names. Skip both sets in the `renderKnobs` bucketing
  loop. The 48 cell knobs never appear on `snap.knobs` directly (they
  ride inside `weather_soc_table`), so they're already invisible to
  `renderKnobs` — the `WIDGET_RENDERED_KNOBS` set covers only the 6
  flat boundary knobs.
- **Done when:** dashboard reload shows the widget but the Knobs
  tables no longer carry the 6 boundary knob rows.

### D12 — Web: relocate widget Detail → Control tab

- File: `crates/shell/static/index.html`.
- Cut `<section id="weather-soc-table">…</section>` from the Detail
  panel and paste into Control panel immediately ABOVE
  `<section id="knobs">`. Section flow becomes: Actuated, SoC chart,
  Weather-SoC table, Knobs, Decisions.
- File: `web/src/index.ts`.
- `renderWeatherSocTable(snap)` call stays; relocation is HTML-only.
- **Done when:** dashboard reload shows the widget on the Control tab
  between SoC chart and Knobs; Detail tab no longer shows it.

### D13 — Web: rebuild widget with editable cells (popup modal)

- File: `web/src/render.ts`.
- Restructure to a 3-column layout: `Bucket | Warm group | Cold
  group`. Each group cell is a single clickable target rendering the
  4 sub-values inline (e.g. `35 / 100 / 20 / —`). Wrap as
  `entity-link` with `data-entity-id="<bucket>.<temp>"`,
  `data-entity-type="weathersoc-cell"`.
- Add `renderWeatherSocCellModalBody(bucket, temp, snap, sendCommand)`
  that builds the modal body:

  ```text
  <table class="cell-edit-grid">
    <tr><td>exp</td>      <td><input type="number" min="0" max="100" step="1"></td>
        <td>[default: 35]</td><td><button data-revert="export_soc_threshold">↺</button></td></tr>
    <tr><td>bat</td>      <td><input type="number" min="0" max="100" step="1"></td>
        <td>[default: 100]</td><td><button>↺</button></td></tr>
    <tr><td>dis</td>      <td><input type="number" min="0" max="100" step="1"></td>
        <td>[default: 20]</td><td><button>↺</button></td></tr>
    <tr><td>extended</td> <td><input type="checkbox"></td>
        <td>[default: false]</td><td><button>↺</button></td></tr>
  </table>
  <footer><button id="wsoc-cell-cancel">Cancel</button>
          <button id="wsoc-cell-save">Save</button></footer>
  ```

- On Save, compare each input to the snapshot value and dispatch only
  changed fields:
  - 3 floats: `{ SetFloatKnob: { knob_name: "weathersoc.table.<bucket>.<temp>.<field>", value } }`
  - 1 bool:   `{ SetBoolKnob:  { knob_name: "weathersoc.table.<bucket>.<temp>.extended", value } }`
- Defaults sourced from `KNOB_SPEC[<dotted>].default`.
- File: `web/src/index.ts`.
- Add `"weathersoc-cell"` to `EntityType` and `VALID_TYPES`. The
  `renderEntityModal` `switch` gains a new arm dispatching to
  `renderWeatherSocCellModalBody`. The `entityId` for this type
  encodes `"<bucket>.<temp>"`. The existing
  `installEntityInspectorHandlers` flow already opens `#entity-modal`
  for any `data-entity-type` link.
- **Dirty-input rule:** when a snapshot arrives while the modal is
  open, do NOT clobber input values that have focus or that already
  differ from the snapshot value. Mirror the existing `renderKnobs`
  focus-preservation discipline.
- **Done when:** clicking a Warm/Cold group on any row opens the
  modal; Save dispatches only changed fields; Cancel closes; Esc
  closes; values round-trip via retained MQTT.

### D14 — Web: clickable column headers backed by descriptions

- File: `crates/shell/static/index.html` and `web/src/render.ts`.
- Wrap the 4 column-header tokens (`exp`, `bat`, `dis`, `ext`) in
  `entity-link` markup pointing at synthetic dotted ids with
  `entity-type="knob"` (so the existing `renderKnobBody` branch fires
  and reads `entityDescriptions`).
- File: `web/src/descriptions.ts`.
- Add 4 entries:
  - `"weathersoc.table.export-soc-threshold"` — "SoC above which we
    permit export this day. 100 = no export."
  - `"weathersoc.table.battery-soc-target"` — "Daytime SoC target the
    planner drives the battery toward."
  - `"weathersoc.table.discharge-soc-target"` — "Overnight discharge
    floor."
  - `"weathersoc.table.extended"` — "Force-charge battery to target
    overnight via grid (cheap window)."
- `renderKnobBody` should be resilient to a missing `snap.knobs[<id>]`
  for these synthetic ids — render description-only.
- **Done when:** clicking each column header opens the entity
  inspector populated with the matching description.

### D15 — Web: render the 6 boundary knobs inline above the cell grid

- File: `web/src/render.ts`.
- Prepend a "Bucket boundaries" section to the widget: 6 inline number
  inputs (5 kWh + 1 °C). Each input shows the current
  `snap.knobs.weathersoc_*` value, has `min/max/step` from
  `KNOB_SPEC`, and a "set" button mirroring the existing
  `renderSetControl` pattern in `knobs.ts`. Order: low (8), ok (15),
  high (30), too-much (45), very-sunny (67.5), winter-temperature
  (12 °C).
- Each "set" button dispatches
  `{ SetFloatKnob: { knob_name: "weathersoc.threshold.energy.low",
  value } }` (etc.) via `sendCommand`.
- Update `renderWeatherSocTable` signature to `(snap, sendCommand)`;
  update call-site in `web/src/index.ts`.
- HTML container: `<div class="weathersoc-boundaries">` ABOVE the
  `<table>` inside `<section id="weather-soc-table">`.
- Mirror keyed-row preservation pattern: write inputs once, update
  `value` attribute on subsequent renders only if the input is not
  focused.
- **Done when:** dashboard shows 6 inline inputs above the 6×2 cell
  grid; editing one + clicking "set" dispatches; snapshot reflects
  the new value on next tick; focus survives a refresh.

### D16 — Web: render.test.ts updates

- File: `web/src/render.test.ts`.
- Update `wsocDefaults` so Low.cold / Dim.warm / Dim.cold all have
  `bat: 100`.
- Adapt the snapshot assertion to the 3-column shape.
- Add tests:
  - `weathersoc_table_widget_groups_open_modal_with_correct_id`
  - `weathersoc_boundaries_section_renders_six_inputs_with_current_values`
  - `weathersoc_table_defaults_match_core_safe_defaults` (drift guard)
  - `renderEntityModal_weathersoc_column_header_shows_description`
- **Done when:** `web` test runner passes.

### D17 — Final verification

```bash
scripts/regen-baboon.sh   # NO baboon changes expected — confirm no diff
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd web && ./node_modules/.bin/tsc --noEmit -p .
# plus web test runner per package.json
```

Manual:
- Dashboard shows widget on Control tab above Knobs.
- Boundary inputs reflect current snapshot; edit + set → snapshot
  updates next tick.
- Click any cell group → modal opens with 4 fields, defaults shown,
  Save dispatches only changed fields.
- Click each column header → entity-inspector modal opens with
  matching description.
- Knobs section no longer has the 6 boundary knob rows.
- HA discovery: 48 new entities (36 number + 12 switch) under
  `victron-controller` device.
- Retained MQTT: `<root>/knob/weathersoc.table.<bucket>.<temp>.<field>/state`
  topics post-bootstrap.

## 3. Acceptance criteria

1. 48 cell fields individually addressable through MQTT, HA, dashboard
   popup editor, and `apply_knob`. ONE `KnobId::WeathersocTableCell {
   … }` variant carries them all.
2. `Knobs::safe_defaults().weather_soc_table.{low_cold,dim_warm,dim_cold}.battery_soc_target
   == 100.0`.
3. The 11 retained cascade-equivalence tests pass with bat=100 in the
   affected assertions; comments document the 2026-05-04 operator
   preference.
4. The widget lives on Control tab above `<section id="knobs">`. It
   renders: a "Bucket boundaries" inline-input row (6 inputs); 4
   clickable column headers that open the entity-inspector modal
   with descriptions; 6 rows × 2 group cells, each clickable to open
   the cell-edit modal.
5. The Knobs tables (`#knobs-operator-table`, `#knobs-config-table`)
   no longer surface the 6 boundary knobs or any of the 48 cell
   knobs.
6. config.toml unchanged for the 48 cell knobs; the 6 boundary knobs
   retain their existing `[knobs]` entries.
7. All four verification commands pass clean.

## 4. Risks / unknowns

- **Snapshot round-trip with open modal** — dirty-input rule
  documented in D13 must hold: snapshots arriving during an in-progress
  edit must not clobber unsaved input values. Mirror the existing
  `renderKnobs` focus-preservation discipline.
- **Entity-inspector union extension** introduces an interactive form
  to a previously read-only modal. Documented in D13 as a deliberate
  vocabulary extension; the alternative (dedicated modal) was rejected
  for less HTML/CSS churn.
- **Two coexisting edit affordances** in the widget (boundary inline
  inputs, cell popup modal). Tight visual separation in the layout;
  both respect keyed-row focus discipline.
- **TS-Rust drift on the 48 defaults** — handled by the JSON-fixture
  drift guard (D10).
- **`KnobId: Copy` survives** with the new variant — the three sibling
  enums are C-like and trivially Copy. Verify with `cargo build` —
  existing call-sites that pass `KnobId` by value compile unchanged.
- **`knob_name` return type → `String`** cascades through several
  files. All current uses immediately format the value into a topic
  string; mechanical change.
- **HA discovery count** — 48 new entities ~doubles the existing knob
  discovery footprint. The existing `publish_knobs` loop awaits each
  publish in sequence; ~few hundred ms boot delay. Acceptable.

## 5. Test matrix

| # | Layer  | Test                                                                                                             | Pre/post                                                                                                                                                                  |
|---|--------|------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 1 | core   | `knobs::tests::weather_soc_table_default_cells`                                                                  | bat=100 in low_cold / dim_warm / dim_cold; other 9 cells unchanged.                                                                                                       |
| 2 | core   | `controllers::weather_soc::tests::cell_low_cold` / `cell_dim_warm` / `cell_dim_cold`                             | tuple becomes `(100, 100, 30, true, true)` from `(100, 90, 30, true, true)`.                                                                                              |
| 3 | core   | `controllers::weather_soc::tests::cold_and_low_energy_extends_charge_and_preserves_morning`                      | `bat==100` (was 90).                                                                                                                                                      |
| 4 | core   | `controllers::weather_soc::tests::ok_or_below_always_extends_charge_regardless_of_temp`                          | `bat==100` (was 90).                                                                                                                                                      |
| 5 | core   | `process::tests::apply_knob_weathersoc_table_cell_*_routes_to_field` × 4 (one per CellField)                     | `apply_knob` mutates the right field via the programmatic arm.                                                                                                            |
| 6 | core   | `process::tests::all_knob_publish_payloads_includes_48_weathersoc_table_cells`                                   | exactly 48 cell entries; counts 36 Float / 12 Bool.                                                                                                                       |
| 7 | shell  | `mqtt::serialize::tests::knob_name_round_trips_for_all_48_weathersoc_table_cells`                                | for each (bucket, temp, field), `knob_id_from_name(knob_name(id)) == Some(id)`.                                                                                           |
| 8 | shell  | `mqtt::serialize::tests::parse_knob_value_weathersoc_table_cell_float_in_range`                                  | `parse_knob_value(WeathersocTableCell{…ExportSocThreshold}, "55") == Some(KnobValue::Float(55.0))`; out-of-range rejected.                                                |
| 9 | shell  | `mqtt::serialize::tests::parse_knob_value_weathersoc_table_cell_extended_bool`                                   | `"true" → KnobValue::Bool(true)`.                                                                                                                                         |
| 10| shell  | `mqtt::discovery::tests::knob_schemas_includes_all_48_weathersoc_table_cells`                                    | every triple appears with the right component type (number/switch).                                                                                                       |
| 11| shell  | `dashboard::convert::tests::knob_id_from_name_parses_weathersoc_table_cell_dotted`                               | dashboard parser returns the right `KnobId` for a dotted name.                                                                                                            |
| 12| web    | `render.test.ts::buildWeatherSocTableRows_groups_warm_and_cold`                                                  | 6 rows × 3-column shape; `data-entity-id="<bucket>.<temp>"`.                                                                                                              |
| 13| web    | `render.test.ts::weather_soc_boundaries_section_renders_six_inputs_with_current_values`                          | 6 inputs match snapshot's `weathersoc_*` fields.                                                                                                                          |
| 14| web    | `render.test.ts::weathersoc_table_defaults_match_core_safe_defaults`                                             | drift-guard: `WEATHER_SOC_DEFAULTS` mirrors the JSON fixture from `Knobs::safe_defaults()`.                                                                               |
| 15| web    | `render.test.ts::renderEntityModal_weathersoc_column_header_shows_description`                                   | the 4 column-header dotted ids each yield non-empty descriptions.                                                                                                         |
| 16| manual | tab placement                                                                                                    | widget on Control tab between SoC chart and Knobs; not on Detail tab.                                                                                                     |
| 17| manual | popup editor end-to-end                                                                                          | open Low.cold modal → change exp from 100 to 95 → Save → snapshot reflects → retained MQTT shows `weathersoc.table.low.cold.export-soc-threshold/state=95`.               |
| 18| manual | HA discovery                                                                                                     | HA shows 48 new entities (36 number + 12 switch) under `victron-controller`.                                                                                              |

## 6. Verification commands

```
scripts/regen-baboon.sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd web && ./node_modules/.bin/tsc --noEmit -p .
# plus the web test runner per web/package.json
```

Manual tests as itemised above.
