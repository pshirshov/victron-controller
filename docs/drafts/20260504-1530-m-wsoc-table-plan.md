# M-WSOC-TABLE / PR-WSOC-TABLE-1 — Plan

## 1. Goal

Replace the cascading `evaluate_weather_soc` ladder in
`crates/core/src/controllers/weather_soc.rs` with a 2D lookup-table
model: 6 energy buckets (VerySunny / Sunny / Mid / Low / Dim / VeryDim)
× 2 temperature columns (warm > 12 °C / cold ≤ 12 °C). Each cell holds
four operator-tunable fields (`export_soc_threshold`,
`battery_soc_target`, `discharge_soc_target`, `extended`);
`disable_night_grid_discharge` is derived from the cell's `extended`
*before* any stacked override applies (so the rung-7 override-only-
mutates-`bat`+`ext` semantics survive verbatim).

Defaults reproduce the current cascade bit-for-bit for the 12 cells. A
new bucket-boundary knob `weathersoc_very_sunny_threshold` (default
67.5 kWh) replaces the hard-coded `1.5 × too_much` multiplier in
former rung 2.

The 12-cell value matrix is wire-modeled as a single nested
`WeatherSocTable` baboon type on `Knobs` and surfaced read-only on the
dashboard for v1; the per-cell fields are NOT individually addressable
through MQTT, HA, config.toml, or the flat `KNOB_SPEC` table. The
weekly `charge_to_full_required` override (former rung 7) is preserved
post-table-lookup with its **legacy strict-kWh inequality verbatim**:
`today_energy < high_energy_threshold_kwh`.

## Bucket boundaries

- VerySunny: `today_energy > weathersoc_very_sunny_threshold`
- Sunny: `(weathersoc_too_much_energy_threshold, weathersoc_very_sunny_threshold]`
- Mid: `(weathersoc_high_energy_threshold, weathersoc_too_much_energy_threshold]`
- Low: `(weathersoc_ok_energy_threshold, weathersoc_high_energy_threshold]`
- Dim: `(weathersoc_low_energy_threshold, weathersoc_ok_energy_threshold]`
- VeryDim: `<= weathersoc_low_energy_threshold`

Temperature column: `warm = today_temp > weathersoc_winter_temperature_threshold`,
else cold. Boundary-at-threshold counts as cold (matches the existing
`exact_temp_threshold_counts_as_cold` test).

## 12-cell defaults (reproduce cascade verbatim)

| Bucket    | Warm: exp / bat / dis / ext | Cold: exp / bat / dis / ext |
|-----------|-----------------------------|-----------------------------|
| VerySunny | 35 / 100 / 20 / no          | 80 / 100 / 30 / no          |
| Sunny     | 50 / 100 / 20 / no          | 80 / 100 / 30 / no          |
| Mid       | 67 / 100 / 20 / no          | 80 / 100 / 30 / no          |
| Low       | 100 / 100 / 30 / no         | 100 / 90 / 30 / yes         |
| Dim       | 100 / 90 / 30 / yes         | 100 / 90 / 30 / yes         |
| VeryDim   | 100 / 100 / 30 / yes        | 100 / 100 / 30 / yes        |

## Stacked override (former rung 7)

```
if g.charge_to_full_required && today_energy < g.high_energy_threshold_kwh {
    battery_soc_target = 100;
    extended = true;
}
let dng = cell.extended;  // derived BEFORE override; cascade-equivalent
```

(The `<` is strict, mirroring the source. The override fires for
Low-with-energy-strictly-below-high, Dim, VeryDim.)

## 2. Sub-task breakdown

### D01 — Embed bucket boundaries + cell defaults table at module top

Add module-level rustdoc to `crates/core/src/controllers/weather_soc.rs`
documenting the boundaries and the 12-cell defaults table. No code yet.

**Done when:** the doc-comment block at the top of the file covers the
new model in enough detail that a reader doesn't need this plan to
understand the controller.

### D02 — `weathersoc_very_sunny_threshold` knob (full 11-step CLAUDE.md walk)

1. `models/dashboard.baboon` — append `weathersoc_very_sunny_threshold: f64`
   inside `data Knobs` (additive, no version bump). Run `scripts/regen-baboon.sh`.
2. `crates/core/src/knobs.rs` — field + default `67.5` in `safe_defaults()` +
   assertion in `safe_defaults_match_spec_7`.
3. `crates/core/src/types.rs` — `KnobId::WeathersocVerySunnyThreshold`.
4. `crates/core/src/process.rs` — `apply_knob` arm.
5. `crates/shell/src/mqtt/serialize.rs` — `knob_name` →
   `"weathersoc.threshold.energy.very-sunny"`; `knob_id_from_name`
   reverse; `knob_range` `(0.0, 1000.0)`; `parse_knob_value` float arm.
6. `crates/shell/src/mqtt/discovery.rs` —
   `number_knob(KnobId::WeathersocVerySunnyThreshold, 1.0, Some("kWh"))`.
7. `crates/shell/src/config.rs` — `KnobsDefaultsConfig` field +
   `set!()` line. `config.example.toml` gets a commented-out line.
8. `crates/shell/src/dashboard/convert.rs` — `knobs_to_model`
   assignment + `knob_id_from_name` snake-case branch.
9. `web/src/displayNames.ts` —
   `weathersoc_very_sunny_threshold: "weathersoc.threshold.energy.very-sunny"`.
10. `web/src/knobs.ts` — `KNOB_SPEC` entry `{ kind: "float", min: 0,
    max: 500, step: 1, default: 67.5, category: "config", group: "Weather-SoC planner" }`.
11. `web/src/descriptions.ts` — human prose.

**Done when:** knob appears under the Weather-SoC planner group in the
dashboard, HA discovery emits a number entity, all four verification
commands pass.

### D03 — Add baboon `WeatherSocCell` + `WeatherSocTable` types

`models/dashboard.baboon`:

```
data WeatherSocCell {
  export_soc_threshold: f64
  battery_soc_target: f64
  discharge_soc_target: f64
  extended: bit
}

data WeatherSocTable {
  very_sunny_warm: WeatherSocCell
  very_sunny_cold: WeatherSocCell
  sunny_warm: WeatherSocCell
  sunny_cold: WeatherSocCell
  mid_warm: WeatherSocCell
  mid_cold: WeatherSocCell
  low_warm: WeatherSocCell
  low_cold: WeatherSocCell
  dim_warm: WeatherSocCell
  dim_cold: WeatherSocCell
  very_dim_warm: WeatherSocCell
  very_dim_cold: WeatherSocCell
}
```

Add field `weather_soc_table: WeatherSocTable` to `data Knobs`. Run
`scripts/regen-baboon.sh`.

**Rationale for flat 12-field shape over `lst[WeatherSocCell]`:** named
accessors in TS / Rust, no index-mapping mistakes possible, baboon's
structural-default story is more predictable for `data` than for `lst`.

**Done when:** `crates/dashboard-model/` and `web/src/model/`
regenerate; downstream Rust + TS still compile after D04.

### D04 — Core knob model: add `WeatherSocTable` to `Knobs`

`crates/core/src/knobs.rs`:

- New `WeatherSocCell` + `WeatherSocTable` structs (Debug, Clone, Copy, PartialEq).
- `WeatherSocTable::safe_defaults()` populated with the 12-cell verbatim.
- Field `weather_soc_table: WeatherSocTable` on `Knobs`;
  `Knobs::safe_defaults()` calls into `WeatherSocTable::safe_defaults()`.

**Done when:** new test `weather_soc_table_default_cells` pins each
of the 12 cells to its `(exp, bat, dis, ext)` tuple from the defaults
table.

### D05 — Replace `evaluate_weather_soc` cascade with table lookup

`crates/core/src/controllers/weather_soc.rs`:

- New `EnergyBucket` enum: `VerySunny | Sunny | Mid | Low | Dim | VeryDim`.
- `classify_energy(g, e, very_sunny) -> EnergyBucket` per the boundary rules.
- New `evaluate_weather_soc` signature takes `&WeatherSocTable` +
  `very_sunny_threshold: f64` in addition to the existing inputs.
- Pick the cell, derive `dng = cell.extended`, apply the strict-kWh
  override, build `Decision` with: bucket label, cold flag,
  override-fired flag, plus the existing factor strings (today_temp,
  today_energy, the four energy thresholds, very_sunny_threshold,
  charge_to_full_required, the 4 outputs).
- Delete the `way_too_much`, `export_more`, `export_max`,
  `preserve_evening_battery`, `disable_export`, `extend_charge`,
  `charge_to_full_extended`, `preserve_morning_battery` closures and
  the rung narration.

Update `crates/core/src/process.rs::run_weather_soc` (~L2386) to pass
`&world.knobs.weather_soc_table` and
`world.knobs.weathersoc_very_sunny_threshold`.

**Done when:** `cargo build` passes.

### D06 — Update existing weather_soc tests for new signature

`crates/core/src/controllers/weather_soc.rs` (test module): each
existing test threads `&Knobs::safe_defaults().weather_soc_table` and
`Knobs::safe_defaults().weathersoc_very_sunny_threshold` into the new
`evaluate_weather_soc` signature. Asserted values stay the same — they
are the cascade-equivalence golden.

**Done when:** all 11 existing tests pass unchanged in their assertions.

### D07 — Add 12-cell pinning tests + override + boundary tests

Add new tests per the test matrix in §5. Each cell test picks a
representative `(today_energy, today_temp)` strictly inside the
bucket and asserts the `(exp, bat, dis, ext, dng)` tuple.

**Done when:** ≥12 new cell tests + 4 override/boundary tests pass.

### D08 — Wire `WeatherSocTable` value through dashboard convert layer

`crates/shell/src/dashboard/convert.rs`:

- New `weather_soc_table_to_model(&WeatherSocTable) -> ModelWeatherSocTable`
  helper near the other small per-type helpers.
- Fill `ModelKnobs.weather_soc_table` in `knobs_to_model`.
- Do NOT add anything to `knob_id_from_name` (the table value is not a
  flat KnobId).
- Do NOT add anything to `command_to_event` (no command shape — read-only
  on dashboard for v1).

**Done when:** snapshot serialization includes
`knobs.weather_soc_table` with the safe-defaults values.

### D09 — Display name + description for the table widget

`web/src/displayNames.ts`:
`weather_soc_table: "weathersoc.table"`.

`web/src/descriptions.ts`:
`"weathersoc.table"` → human prose explaining the 6×2 grid, that
defaults flow from `safe_defaults()`, and that per-cell editing is a
future PR.

**Done when:** TS compile clean.

### D10 — Add the read-only widget to the dashboard

`crates/shell/static/index.html`: add a new
`<section id="weather-soc-table">` inside the **Detail** tab panel
(between `forecasts` and `zappi-drain-section`), heading "Weather-SoC
table", containing one table with 6 rows × `[Bucket | exp | bat | dis | ext]`
× 2 temp groups (8 data columns + 1 label column).

`web/src/render.ts`: new
`export function renderWeatherSocTable(snap: WorldSnapshot): void`.
Reads `snap.knobs.weather_soc_table` and renders one row per bucket
using `updateKeyedRows` for stability. Two-row column header with
`colspan=4` "Warm" / "Cold" group cells. `extended` rendered as `✓` /
`—`. No per-cell controls.

`web/src/index.ts`: import + dispatch `renderWeatherSocTable(snap)`
gated on `!prev || !deepEqual(prev.knobs.weather_soc_table,
snap.knobs.weather_soc_table)`.

**Done when:** dashboard reload shows the table populated with the 12
default cells; values match §1; no console errors.

### D11 — Snapshot test for the new widget

`web/src/render.test.ts`: snapshot-style test asserting
`renderWeatherSocTable` produces expected HTML for safe-defaults.
Mirror granularity of existing tests in this file.

**Done when:** `tsc --noEmit -p .` clean; whatever runner the project
uses for `render.test.ts` passes.

### D12 — Final verification

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd web && ./node_modules/.bin/tsc --noEmit -p .
# plus whatever web/package.json defines for render.test.ts
```

Manual:
- Reload dashboard → Detail tab → confirm "Weather-SoC table" renders
  with the 12 default values matching §1.
- Toggle `weathersoc_very_sunny_threshold` from the Knobs section
  (Configuration / Weather-SoC planner group); confirm the boundary
  moves on the next planner tick.

## 3. Acceptance criteria

1. `weather_soc.rs` no longer contains the cascading rung-by-rung
   ladder — replaced with discrete bucket classification + table
   lookup + single override.
2. For any input strictly inside a bucket, the new
   `evaluate_weather_soc` returns the same
   `(export_soc_threshold, discharge_soc_target, battery_soc_target,
   charge_battery_extended, disable_night_grid_discharge)` tuple as
   the previous cascade — verified by retaining all 11 existing tests
   with their original assertions and adding 12 cell-pinning tests.
3. `dng = cell.extended` (derived **before** override). Override-only
   mutates `bat` + `ext`, leaving `dng` intact — preserves cascade
   semantics in the rung-7 case.
4. New knob `weathersoc_very_sunny_threshold` (default 67.5 kWh) is
   registered in all 11 layers per CLAUDE.md.
5. `WeatherSocTable` is wire-modeled, populated from
   `Knobs::safe_defaults()` at boot, propagated into the snapshot via
   `convert.rs::knobs_to_model`, and rendered read-only in the
   dashboard's Detail tab.
6. Per-cell values are NOT individually addressable through
   `apply_knob`, MQTT `knob/<name>/set`, HA discovery, `[knobs]`
   `config.toml`, or the flat `KNOB_SPEC` table.
7. All four verification commands pass clean.

## 4. Risks / unknowns

- **Baboon nested-data defaults at deserialize time.** CLAUDE.md says
  `convert__…` stubs are never called at runtime, so we don't need to
  hand-implement them. Defaults flow from `Knobs::safe_defaults()` in
  Rust regardless. Mitigation: invoke the `baboon` skill if a build
  warning surfaces.
- **Dashboard styling consistency.** A 6×8 matrix is a different
  visual idiom than the existing tall-row tables. Acceptable to inline
  some `<style>` or borrow patterns from `forecasts-table` /
  `pinned-registers-table`. Cross-check `web/src/style.css` (if
  present) or the inline styles in `index.html`.
- **`render.test.ts` runner.** The exact test runner isn't pinned in
  the plan; D11 should adapt to whatever's in `web/package.json`.
- **Bucket-edge semantics.** `today_energy == high` lands in Low
  (closed at top); `today_energy == too_much` lands in Mid (closed at
  top); `today_energy == very_sunny_threshold` lands in Sunny (closed
  at top). All match the cascade's `<=` boundaries and the existing
  `exact_energy_threshold_counts_as_below` test.

## 5. Test matrix

| # | Bucket    | Temp  | Inputs (energy / temp °C / cf) | Expected (exp, bat, dis, ext, dng) |
|---|-----------|-------|--------------------------------|-------------------------------------|
| 1 | VerySunny | warm  | 100 / 25 / false               | 35, 100, 20, false, false           |
| 2 | VerySunny | cold  | 100 /  5 / false               | 80, 100, 30, false, false           |
| 3 | Sunny     | warm  | 50  / 25 / false               | 50, 100, 20, false, false           |
| 4 | Sunny     | cold  | 50  /  5 / false               | 80, 100, 30, false, false           |
| 5 | Mid       | warm  | 40  / 25 / false               | 67, 100, 20, false, false           |
| 6 | Mid       | cold  | 40  /  5 / false               | 80, 100, 30, false, false           |
| 7 | Low       | warm  | 25  / 25 / false               | 100, 100, 30, false, false          |
| 8 | Low       | cold  | 25  /  5 / false               | 100, 90, 30, true, true             |
| 9 | Dim       | warm  | 12  / 25 / false               | 100, 90, 30, true, true             |
| 10| Dim       | cold  | 12  /  5 / false               | 100, 90, 30, true, true             |
| 11| VeryDim   | warm  | 5   / 25 / false               | 100, 100, 30, true, true            |
| 12| VeryDim   | cold  | 5   /  5 / false               | 100, 100, 30, true, true            |
| 13| Rung-7 / Low warm + cf=true                       | 25 / 25 / true     | exp=100, bat=100, dis=30, ext=true, dng=false (override only mutates bat+ext) |
| 14| Rung-7 / Dim warm + cf=true                       | 12 / 25 / true     | exp=100, bat=100, dis=30, ext=true, dng=true (extended already true)         |
| 15| Rung-7 skipped on VerySunny + cf=true             | 100 / 25 / true    | exp=35, bat=100, dis=20, ext=false, dng=false                                  |
| 16| Boundary `temp == 12` counts as cold              | 50 / 12 / false    | exp=80, dis=30 (Sunny cold)                                                    |
| 17| Boundary `today_energy == high`                   | 30 / 25 / false    | Low warm: 100, 100, 30, false, false                                           |
| 18| Boundary `today_energy == too_much`               | 45 / 25 / false    | Mid warm: 67, 100, 20, false, false                                            |
| 19| Boundary `today_energy == very_sunny_threshold`   | 67.5 / 25 / false  | Sunny warm: 50, 100, 20, false, false                                          |

## 6. Verification commands

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd web && ./node_modules/.bin/tsc --noEmit -p .
```

Plus the `render.test.ts` runner (lookup in `web/package.json`).

Manual dashboard reload + Detail tab inspection.
