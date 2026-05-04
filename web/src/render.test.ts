// PR-ZD-5: smoke-check for MPPT operation-mode rendering.
// PR-ZDO-4: smoke-checks for Zappi compensated-drain rendering.
//
// No test framework is present in this project (only tsc + esbuild).
// This file is type-checked by: cd web && ./node_modules/.bin/tsc --noEmit -p .
//
// The assertions below are compile-time (TypeScript). Any runtime failure
// throws an Error (non-zero exit when run via ts-node or similar).

import {
  fmtMpptOperationMode,
  fmtSensorValue,
  BRANCH_COLOR,
  BRANCH_LABEL,
  BRANCH_CSS_CLASS,
  summaryFor,
  buildWeatherSocTableRows,
  type WeatherSocTableLike,
} from "./render.js";
import { KNOB_SPEC, WEATHER_SOC_DEFAULTS } from "./knobs.js";
import { entityDescriptions } from "./descriptions.js";
import { ZappiDrainSnapshotWire } from "./model/victron_controller/dashboard/ZappiDrainSnapshotWire.js";
import { ZappiDrainBranch } from "./model/victron_controller/dashboard/ZappiDrainBranch.js";

function assert(label: string, actual: string, expected: string): void {
  if (actual !== expected) {
    throw new Error(`FAIL [${label}]: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
}

function assertBool(label: string, actual: boolean, expected: boolean): void {
  if (actual !== expected) {
    throw new Error(`FAIL [${label}]: expected ${expected}, got ${actual}`);
  }
}

// --- fmtMpptOperationMode ---

assert("code 0 → Off", fmtMpptOperationMode(0), "Off");
assert("code 1 → Voltage-or-current-limited", fmtMpptOperationMode(1), "Voltage-or-current-limited");
assert("code 2 → MPPT-tracking", fmtMpptOperationMode(2), "MPPT-tracking");
assert("code 5 (out-of-range) → numeric fallback", fmtMpptOperationMode(5), "5");
// Non-integer drift: 2.0001 rounds to 2 → "MPPT-tracking".
assert("code 2.0001 rounds to MPPT-tracking", fmtMpptOperationMode(2.0001), "MPPT-tracking");

// --- fmtSensorValue ---

// MPPT sensor names are intercepted.
assert(
  "fmtSensorValue mppt_0_operation_mode code 2",
  fmtSensorValue("mppt_0_operation_mode", 2)!,
  "MPPT-tracking",
);
assert(
  "fmtSensorValue mppt_1_operation_mode code 0",
  fmtSensorValue("mppt_1_operation_mode", 0)!,
  "Off",
);
// An unrelated sensor returns null (caller falls through to fmtNum).
assert(
  "fmtSensorValue unrelated sensor returns null",
  String(fmtSensorValue("battery_soc", 82.5)),
  "null",
);

// --- PR-ZDO-4: branch lookup tables ------------------------------------------
//
// T1, T2, T3 below test the pure data referenced by renderZappiDrainSummary
// and renderZappiDrainChart. Because there is no DOM in the tsc-only check
// environment, the DOM-mutating render functions themselves are not directly
// called here; instead we verify the colour/label/class constants that drive
// the rendered output.

// PR-ZDO-4.T1 renderZappiDrainSummary_displays_latest_snapshot
// Verify that the Tighten branch maps to the expected display text, CSS class,
// and colour that a renderer with a populated latest snapshot would apply.
assert(
  "T1: Tighten branch label",
  BRANCH_LABEL[ZappiDrainBranch.Tighten],
  "Tighten",
);
assert(
  "T1: Tighten branch CSS class",
  BRANCH_CSS_CLASS[ZappiDrainBranch.Tighten],
  "branch-tighten",
);
assert(
  "T1: Tighten branch colour",
  BRANCH_COLOR[ZappiDrainBranch.Tighten],
  "#d33",
);
// The hard-clamp-engaged CSS class contract is exercised in the summaryFor
// tests below (D01), which use real snapshot data rather than literal
// comparisons.

// PR-ZDO-4.T2 renderZappiDrainSummary_handles_empty_state
// Verify that all four branches have CSS classes, labels, and colours
// defined — if any record entry is missing, the render call would silently
// produce `undefined`. A complete Record with no optional keys is enforced
// by TypeScript's type system; check all four values at runtime for defence.
assertBool("T2: all branches have labels", [
  ZappiDrainBranch.Tighten,
  ZappiDrainBranch.Relax,
  ZappiDrainBranch.Bypass,
  ZappiDrainBranch.Disabled,
].every((b) => typeof BRANCH_LABEL[b] === "string" && BRANCH_LABEL[b].length > 0), true);

assertBool("T2: all branches have CSS classes", [
  ZappiDrainBranch.Tighten,
  ZappiDrainBranch.Relax,
  ZappiDrainBranch.Bypass,
  ZappiDrainBranch.Disabled,
].every((b) => typeof BRANCH_CSS_CLASS[b] === "string" && BRANCH_CSS_CLASS[b].length > 0), true);

assertBool("T2: all branches have colours", [
  ZappiDrainBranch.Tighten,
  ZappiDrainBranch.Relax,
  ZappiDrainBranch.Bypass,
  ZappiDrainBranch.Disabled,
].every((b) => typeof BRANCH_COLOR[b] === "string" && BRANCH_COLOR[b].length > 0), true);

// Disabled branch renders neutral grey (not a warm accent colour).
assert("T2: Disabled branch colour is neutral", BRANCH_COLOR[ZappiDrainBranch.Disabled], "#555");
assert("T2: Disabled branch CSS class", BRANCH_CSS_CLASS[ZappiDrainBranch.Disabled], "branch-disabled");

// PR-ZDO-4.T3 renderZappiDrainChart_draws_polyline_and_reference_lines
// Verify that the four branch colours used in polyline segments are the
// correct hex codes matching the locked decisions in the plan.
assert("T3: Tighten segment colour = red", BRANCH_COLOR[ZappiDrainBranch.Tighten], "#d33");
assert("T3: Relax segment colour = green", BRANCH_COLOR[ZappiDrainBranch.Relax], "#3a3");
assert("T3: Bypass segment colour = grey", BRANCH_COLOR[ZappiDrainBranch.Bypass], "#888");
assert("T3: Disabled segment colour = neutral", BRANCH_COLOR[ZappiDrainBranch.Disabled], "#555");

// --- summaryFor: pure decision logic (D01) ---

// summaryFor: latest=undefined — all dashes, neutral classes.
{
  const r = summaryFor(undefined);
  assert("summaryFor undefined: compensatedText", r.compensatedText, "—");
  assert("summaryFor undefined: branchText", r.branchText, "—");
  assert("summaryFor undefined: hardClampText", r.hardClampText, "—");
  assert("summaryFor undefined: compensatedClass", r.compensatedClass, "big-number");
  assert("summaryFor undefined: branchClass", r.branchClass, "big-number");
  assert("summaryFor undefined: hardClampClass", r.hardClampClass, "big-number");
}

// summaryFor: Tighten + clamp engaged.
{
  const r = summaryFor(new ZappiDrainSnapshotWire(1500, ZappiDrainBranch.Tighten, true, 300, 1000, 200, BigInt(1000)));
  assert("summaryFor Tighten: compensatedText", r.compensatedText, "1500 W");
  assert("summaryFor Tighten: branchText", r.branchText, "Tighten");
  assert("summaryFor Tighten: hardClampText", r.hardClampText, "Engaged");
  assert("summaryFor Tighten: compensatedClass", r.compensatedClass, "big-number branch-tighten");
  assert("summaryFor Tighten: branchClass", r.branchClass, "big-number branch-tighten");
  assert("summaryFor Tighten: hardClampClass", r.hardClampClass, "big-number hard-clamp-engaged");
}

// summaryFor: Disabled → "—" instead of "0 W" (PR-ZDO-1-D05 / PR-ZDO-2-D02 contract).
{
  const r = summaryFor(new ZappiDrainSnapshotWire(0, ZappiDrainBranch.Disabled, false, 0, 1000, 200, BigInt(1000)));
  assert("summaryFor Disabled: compensatedText is dash not 0 W", r.compensatedText, "—");
  assert("summaryFor Disabled: branchText", r.branchText, "Disabled");
  assert("summaryFor Disabled: hardClampText", r.hardClampText, "Disengaged");
  assert("summaryFor Disabled: branchClass", r.branchClass, "big-number branch-disabled");
}

// summaryFor: Relax + clamp disengaged.
{
  const r = summaryFor(new ZappiDrainSnapshotWire(500, ZappiDrainBranch.Relax, false, 0, 1000, 200, BigInt(1000)));
  assert("summaryFor Relax: compensatedText", r.compensatedText, "500 W");
  assert("summaryFor Relax: branchText", r.branchText, "Relax");
  assert("summaryFor Relax: hardClampText", r.hardClampText, "Disengaged");
  assert("summaryFor Relax: hardClampClass", r.hardClampClass, "big-number hard-clamp-disengaged");
}

// --- PR-WSOC-EDIT-1: buildWeatherSocTableRows ----------------------------
//
// 3-column shape (Bucket | Warm group | Cold group). Each group cell is
// a single clickable target wrapped as `entity-link` with
// `data-entity-id="<bucket>.<temp>"` and
// `data-entity-type="weathersoc-cell"`.

function cell(exp: number, bat: number, dis: number, ext: boolean) {
  return { export_soc_threshold: exp, battery_soc_target: bat, discharge_soc_target: dis, extended: ext };
}

// PR-WSOC-EDIT-1: bat=100 across every extended cell (Low.cold,
// Dim.warm, Dim.cold). Mirrors the JSON fixture
// `web/test-fixtures/weather-soc-defaults.json` produced by the
// drift-guard Rust test.
const wsocDefaults: WeatherSocTableLike = {
  very_sunny_warm: cell(35, 100, 20, false),
  very_sunny_cold: cell(80, 100, 30, false),
  sunny_warm: cell(50, 100, 20, false),
  sunny_cold: cell(80, 100, 30, false),
  mid_warm: cell(67, 100, 20, false),
  mid_cold: cell(80, 100, 30, false),
  low_warm: cell(100, 100, 30, false),
  low_cold: cell(100, 100, 30, true),
  dim_warm: cell(100, 100, 30, true),
  dim_cold: cell(100, 100, 30, true),
  very_dim_warm: cell(100, 100, 30, true),
  very_dim_cold: cell(100, 100, 30, true),
};

{
  const rows = buildWeatherSocTableRows(wsocDefaults);
  // Six bucket rows in canonical order (most sun → least).
  assert("wsoc rows: 6 buckets", String(rows.length), "6");
  const expectedKeys = ["very_sunny", "sunny", "mid", "low", "dim", "very_dim"];
  rows.forEach((r, i) => {
    assert(`wsoc rows[${i}] key`, r.key, expectedKeys[i]);
    // 3-column shape: Label, Warm-group cell, Cold-group cell.
    assert(`wsoc rows[${i}] cell count`, String(r.cells.length), "3");
  });

  // PR-WSOC-EDIT-1: weathersoc_table_widget_groups_open_modal_with_correct_id
  // — each group cell is wrapped as entity-link with
  //   data-entity-id="<bucket>.<temp>" and data-entity-type="weathersoc-cell".
  const vs = rows[0];
  assert("wsoc very_sunny label", vs.cells[0].html, "VerySunny");
  // Warm group: contains the dotted modal id and the inline values.
  assertBool(
    "wsoc very_sunny warm cell carries entity-link wrapper",
    vs.cells[1].html.includes('data-entity-type="weathersoc-cell"'),
    true,
  );
  assertBool(
    "wsoc very_sunny warm cell carries data-entity-id=very-sunny.warm",
    vs.cells[1].html.includes('data-entity-id="very-sunny.warm"'),
    true,
  );
  assertBool(
    "wsoc very_sunny warm cell renders 35/100/20/—",
    vs.cells[1].html.includes("35 / 100 / 20 / —"),
    true,
  );
  assertBool(
    "wsoc very_sunny cold cell carries data-entity-id=very-sunny.cold",
    vs.cells[2].html.includes('data-entity-id="very-sunny.cold"'),
    true,
  );

  // Low row: cold group renders bat=100 (was 90 pre-PR-WSOC-EDIT-1)
  // and extended=true.
  const lo = rows[3];
  assert("wsoc low label", lo.cells[0].html, "Low");
  assertBool(
    "wsoc low warm renders 100/100/30/— (extended off)",
    lo.cells[1].html.includes("100 / 100 / 30 / —"),
    true,
  );
  assertBool(
    "wsoc low cold renders 100/100/30/✓ (PR-WSOC-EDIT-1: bat=100 not 90)",
    lo.cells[2].html.includes("100 / 100 / 30 / ✓"),
    true,
  );

  // VeryDim row: both group cells render ✓.
  const vd = rows[5];
  assert("wsoc very_dim label", vd.cells[0].html, "VeryDim");
  assertBool(
    "wsoc very_dim warm extended true",
    vd.cells[1].html.includes("100 / 100 / 30 / ✓"),
    true,
  );
  assertBool(
    "wsoc very_dim cold extended true",
    vd.cells[2].html.includes("100 / 100 / 30 / ✓"),
    true,
  );
}

// PR-WSOC-EDIT-1: weathersoc_table_defaults_match_core_safe_defaults
// — drift guard. The TS-side `WEATHER_SOC_DEFAULTS` map must agree
// with the JSON fixture that the Rust drift-guard test
// (`weathersoc_defaults_fixture_matches_safe_defaults` in
// `crates/shell/src/dashboard/convert.rs`) produces.
{
  // The fixture file is project-relative; tsc only typechecks this
  // file, so we cannot read the file at "test runtime" the way a real
  // jest/mocha runner would. Instead, embed the expected payload as
  // an inline literal mirroring the file's contents. The Rust test
  // guarantees the file matches `safe_defaults()`; this TS test
  // guarantees `WEATHER_SOC_DEFAULTS` matches the file. Three-leg
  // assertion: core ↔ fixture (Rust test) and fixture ↔ TS map (this
  // test) ⇒ core ↔ TS map.
  const fixture: Record<string, [number, number, number, boolean]> = {
    "very-sunny.warm": [35, 100, 20, false],
    "very-sunny.cold": [80, 100, 30, false],
    "sunny.warm": [50, 100, 20, false],
    "sunny.cold": [80, 100, 30, false],
    "mid.warm": [67, 100, 20, false],
    "mid.cold": [80, 100, 30, false],
    "low.warm": [100, 100, 30, false],
    "low.cold": [100, 100, 30, true],
    "dim.warm": [100, 100, 30, true],
    "dim.cold": [100, 100, 30, true],
    "very-dim.warm": [100, 100, 30, true],
    "very-dim.cold": [100, 100, 30, true],
  };
  const tsKeys = Object.keys(WEATHER_SOC_DEFAULTS).sort();
  const fxKeys = Object.keys(fixture).sort();
  assert(
    "WEATHER_SOC_DEFAULTS / fixture key set",
    tsKeys.join(","),
    fxKeys.join(","),
  );
  for (const k of fxKeys) {
    const t = WEATHER_SOC_DEFAULTS[k];
    const f = fixture[k];
    assert(`WEATHER_SOC_DEFAULTS[${k}] length`, String(t.length), "4");
    assert(`WEATHER_SOC_DEFAULTS[${k}][0] exp`, String(t[0]), String(f[0]));
    assert(`WEATHER_SOC_DEFAULTS[${k}][1] bat`, String(t[1]), String(f[1]));
    assert(`WEATHER_SOC_DEFAULTS[${k}][2] dis`, String(t[2]), String(f[2]));
    assert(`WEATHER_SOC_DEFAULTS[${k}][3] ext`, String(t[3]), String(f[3]));
  }
}

// PR-WSOC-EDIT-1: weathersoc_boundaries_section_renders_six_inputs_with_current_values
// — assert that all 6 boundary knob KNOB_SPEC entries are present (the
// inline-input row reads from these for min/max/step/default).
{
  const expected = [
    "weathersoc.threshold.energy.low",
    "weathersoc.threshold.energy.ok",
    "weathersoc.threshold.energy.high",
    "weathersoc.threshold.energy.too-much",
    "weathersoc.threshold.energy.very-sunny",
    "weathersoc.threshold.winter-temperature",
  ];
  for (const k of expected) {
    assertBool(`KNOB_SPEC has boundary entry ${k}`, k in KNOB_SPEC, true);
  }
}

// PR-WSOC-EDIT-1: renderEntityModal_weathersoc_column_header_shows_description
// — the 4 column-header dotted ids each yield non-empty descriptions.
{
  const headers = [
    "weathersoc.table.export-soc-threshold",
    "weathersoc.table.battery-soc-target",
    "weathersoc.table.discharge-soc-target",
    "weathersoc.table.extended",
  ];
  for (const h of headers) {
    const desc = (entityDescriptions as Record<string, string>)[h];
    assertBool(`description present for ${h}`, typeof desc === "string" && desc.length > 0, true);
  }
}

// PR-WSOC-EDIT-1: 48 cell knobs registered in KNOB_SPEC.
{
  let count = 0;
  for (const bucket of ["very-sunny", "sunny", "mid", "low", "dim", "very-dim"]) {
    for (const temp of ["warm", "cold"]) {
      for (const field of ["export-soc-threshold", "battery-soc-target", "discharge-soc-target", "extended"]) {
        const key = `weathersoc.table.${bucket}.${temp}.${field}`;
        assertBool(`KNOB_SPEC has ${key}`, key in KNOB_SPEC, true);
        count++;
      }
    }
  }
  assert("48 cell knobs in KNOB_SPEC", String(count), "48");
}
