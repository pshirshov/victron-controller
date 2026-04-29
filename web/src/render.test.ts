// PR-ZD-5: smoke-check for MPPT operation-mode rendering.
//
// No test framework is present in this project (only tsc + esbuild).
// This file is type-checked by: cd web && ./node_modules/.bin/tsc --noEmit -p .
//
// The assertions below are compile-time (TypeScript). Any runtime failure
// throws an Error (non-zero exit when run via ts-node or similar).

import { fmtMpptOperationMode, fmtSensorValue } from "./render.js";

function assert(label: string, actual: string, expected: string): void {
  if (actual !== expected) {
    throw new Error(`FAIL [${label}]: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
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
