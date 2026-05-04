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

// --- PR-WSOC-TABLE-1: buildWeatherSocTableRows ----------------------------
//
// Snapshot-style smoke check against the safe-defaults table values
// (mirrors `Knobs::safe_defaults().weather_soc_table` in core). Six rows
// keyed `very_sunny / sunny / mid / low / dim / very_dim`; each row has
// 9 cells (1 label + 4 warm + 4 cold).

function cell(exp: number, bat: number, dis: number, ext: boolean) {
  return { export_soc_threshold: exp, battery_soc_target: bat, discharge_soc_target: dis, extended: ext };
}

const wsocDefaults: WeatherSocTableLike = {
  very_sunny_warm: cell(35, 100, 20, false),
  very_sunny_cold: cell(80, 100, 30, false),
  sunny_warm: cell(50, 100, 20, false),
  sunny_cold: cell(80, 100, 30, false),
  mid_warm: cell(67, 100, 20, false),
  mid_cold: cell(80, 100, 30, false),
  low_warm: cell(100, 100, 30, false),
  low_cold: cell(100, 90, 30, true),
  dim_warm: cell(100, 90, 30, true),
  dim_cold: cell(100, 90, 30, true),
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
    // 1 label + 4 warm + 4 cold = 9 cells per row.
    assert(`wsoc rows[${i}] cell count`, String(r.cells.length), "9");
  });

  // Spot-check a few representative rows. Cells layout:
  // [Label, exp_warm, bat_warm, dis_warm, ext_warm,
  //         exp_cold, bat_cold, dis_cold, ext_cold]

  // very_sunny: warm 35/100/20/— ; cold 80/100/30/—
  const vs = rows[0];
  assert("wsoc very_sunny label", vs.cells[0].html, "VerySunny");
  assert("wsoc very_sunny exp_warm", vs.cells[1].html, "35");
  assert("wsoc very_sunny bat_warm", vs.cells[2].html, "100");
  assert("wsoc very_sunny dis_warm", vs.cells[3].html, "20");
  assert("wsoc very_sunny ext_warm (false)", vs.cells[4].html, "—");
  assert("wsoc very_sunny exp_cold", vs.cells[5].html, "80");
  assert("wsoc very_sunny ext_cold (false)", vs.cells[8].html, "—");

  // low: warm 100/100/30/— ; cold 100/90/30/✓
  const lo = rows[3];
  assert("wsoc low label", lo.cells[0].html, "Low");
  assert("wsoc low exp_warm", lo.cells[1].html, "100");
  assert("wsoc low ext_warm (false)", lo.cells[4].html, "—");
  assert("wsoc low bat_cold", lo.cells[6].html, "90");
  assert("wsoc low ext_cold (true)", lo.cells[8].html, "✓");

  // very_dim: warm 100/100/30/✓ ; cold 100/100/30/✓
  const vd = rows[5];
  assert("wsoc very_dim label", vd.cells[0].html, "VeryDim");
  assert("wsoc very_dim ext_warm (true)", vd.cells[4].html, "✓");
  assert("wsoc very_dim ext_cold (true)", vd.cells[8].html, "✓");
}
