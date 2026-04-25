// Knob section rendering + command dispatch.
//
// Knobs live in a keyed-row table maintained by `updateKeyedRows`, so
// cells that are being edited (focused input, open <select>) are
// preserved across incoming snapshots — that's how toggles and
// "set" buttons become reliable instead of racing the next refresh.
//
// Click + change handling is delegated to the tbody element, so newly
// inserted rows pick up the same logic without re-attaching listeners.

import type { WorldSnapshot } from "./model/victron_controller/dashboard/WorldSnapshot.js";
import { displayNameOfTyped } from "./displayNames.js";
import { entityLink, updateKeyedRows, type KeyedRow } from "./render.js";

export type KnobSpec =
  | { kind: "bool"; default: boolean }
  | { kind: "float"; min: number; max: number; step: number; default: number }
  | { kind: "int"; min: number; max: number; step: number; default: number }
  | { kind: "enum"; cmdVariant: string; options: string[]; default: string };

// PR-rename-entities: keyed by the dotted display name (the user-facing
// surface). Renderers look up via `displayNameOfTyped(canonical, "knob")`
// — `specFor()` below.
//
// `default` mirrors `crates/core/src/knobs.rs::Knobs::safe_defaults()`.
// Powers the "Default" column + reset icon in the Knobs table. Edit
// here when the safe_defaults change in core.
export const KNOB_SPEC: Record<string, KnobSpec> = {
  "grid.export.force-disable": { kind: "bool", default: false },
  "battery.soc.threshold.export.forced-value": { kind: "float", min: 0, max: 100, step: 1, default: 80 },
  "battery.soc.target.discharge.forced-value": { kind: "float", min: 0, max: 100, step: 1, default: 80 },
  "battery.soc.target.charge.forced-value": { kind: "float", min: 0, max: 100, step: 1, default: 80 },
  "battery.soc.target.full-charge.discharge": { kind: "float", min: 0, max: 100, step: 1, default: 57 },
  "battery.soc.threshold.full-charge.export": { kind: "float", min: 0, max: 100, step: 1, default: 100 },
  "battery.discharge.time": { kind: "enum", cmdVariant: "SetDischargeTime", options: ["At0200", "At2300"], default: "At0200" },
  "debug.full-charge.mode": { kind: "enum", cmdVariant: "SetDebugFullCharge", options: ["Forbid", "Force", "None_"], default: "None_" },
  "forecast.pessimism.modifier": { kind: "float", min: 0, max: 2, step: 0.05, default: 1 },
  "grid.night.discharge.disable.forced-value": { kind: "bool", default: false },
  "evcharger.boost.enable": { kind: "bool", default: false },
  "evcharger.extended.enable": { kind: "bool", default: false },
  "evcharger.current.target": { kind: "float", min: 6, max: 32, step: 0.5, default: 9.5 },
  // A-14: kWh (per-session EV charge ceiling), not %. Default 65 kWh
  // covers a Tesla Model 3 LR full charge and sits on the auto-stop
  // gate boundary.
  "evcharger.session.limit": { kind: "float", min: 0, max: 100, step: 0.5, default: 65 },
  "evcharger.current.margin": { kind: "float", min: 0, max: 10, step: 0.5, default: 5 },
  "grid.export.limit": { kind: "int", min: 0, max: 10000, step: 50, default: 4900 },
  "grid.import.limit": { kind: "int", min: 0, max: 10000, step: 10, default: 10 },
  "battery.export.car.allow": { kind: "bool", default: false },
  "eddi.soc.enable": { kind: "float", min: 50, max: 100, step: 1, default: 96 },
  "eddi.soc.disable": { kind: "float", min: 50, max: 100, step: 1, default: 94 },
  "eddi.dwell.seconds": { kind: "int", min: 0, max: 3600, step: 5, default: 60 },
  "weathersoc.threshold.winter-temperature": { kind: "float", min: -30, max: 40, step: 0.5, default: 12 },
  "weathersoc.threshold.energy.low": { kind: "float", min: 0, max: 500, step: 1, default: 8 },
  "weathersoc.threshold.energy.ok": { kind: "float", min: 0, max: 500, step: 1, default: 15 },
  "weathersoc.threshold.energy.high": { kind: "float", min: 0, max: 500, step: 1, default: 30 },
  "weathersoc.threshold.energy.too-much": { kind: "float", min: 0, max: 500, step: 1, default: 45 },
  "forecast.disagreement.strategy": {
    kind: "enum",
    cmdVariant: "SetForecastDisagreementStrategy",
    options: ["Max", "Min", "Mean", "SolcastIfAvailableElseMean"],
    default: "SolcastIfAvailableElseMean",
  },
  "schedule.extended.charge.mode": {
    kind: "enum",
    cmdVariant: "SetChargeBatteryExtendedMode",
    options: ["Auto", "Forced", "Disabled"],
    default: "Auto",
  },
  // PR-gamma-hold-redesign mode knobs (4): Weather selects the
  // weathersoc-derived bookkeeping value; Forced uses the matching
  // *.forced-value knob instead. cmdVariant is `SetMode`; the Rust
  // backend dispatches via `knob_name` field on the command.
  "battery.soc.threshold.export.mode": {
    kind: "enum",
    cmdVariant: "SetMode",
    options: ["Weather", "Forced"],
    default: "Weather",
  },
  "battery.soc.target.discharge.mode": {
    kind: "enum",
    cmdVariant: "SetMode",
    options: ["Weather", "Forced"],
    default: "Weather",
  },
  "battery.soc.target.charge.mode": {
    kind: "enum",
    cmdVariant: "SetMode",
    options: ["Weather", "Forced"],
    default: "Weather",
  },
  "grid.night.discharge.disable.mode": {
    kind: "enum",
    cmdVariant: "SetMode",
    options: ["Weather", "Forced"],
    default: "Weather",
  },
};

/// Look up a `KnobSpec` by either the canonical snake_case key (as it
/// appears in `snap.knobs`) or its dotted display name. Renderers call
/// this rather than indexing KNOB_SPEC directly.
export function specFor(canonical: string): KnobSpec | undefined {
  return KNOB_SPEC[displayNameOfTyped(canonical, "knob")] ?? KNOB_SPEC[canonical];
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" } as Record<string, string>)[ch]!);
}

function fmtNum(v: number, digits = 3): string {
  if (!isFinite(v)) return String(v);
  const s = v.toFixed(digits);
  return s.replace(/\.?0+$/, "");
}

// Installed once; subsequent calls are no-ops.
let handlersInstalled = false;
let currentSnap: WorldSnapshot | null = null;
let currentSend: ((cmd: unknown) => void) | null = null;

function installHandlers() {
  if (handlersInstalled) return;
  handlersInstalled = true;

  // Kill-switch button (always present, fixed markup).
  document.getElementById("kill-switch")?.addEventListener("click", (ev) => {
    const btn = (ev.target as HTMLElement).closest("[data-kill]") as HTMLButtonElement | null;
    if (!btn || !currentSend) return;
    const target = btn.getAttribute("data-kill") === "true";
    currentSend({ SetKillSwitch: { value: target } });
  });

  // Knob table: one delegated click handler for all "toggle" / "set" buttons.
  const tbody = document.querySelector("#knobs-table tbody") as HTMLElement | null;
  if (!tbody) return;
  tbody.addEventListener("click", (ev) => {
    const btn = (ev.target as HTMLElement).closest("button[data-knob-action]") as HTMLButtonElement | null;
    if (!btn || !currentSend || !currentSnap) return;
    const name = btn.getAttribute("data-knob") ?? "";
    const spec = specFor(name);
    if (!spec) return;
    const td = btn.closest("td");
    if (!td) return;

    const action = btn.getAttribute("data-knob-action");
    // `reset` dispatches the spec's `default` value; otherwise read the
    // value from the input/select on the same row.
    if (action === "reset") {
      dispatchKnobValue(name, spec, spec.default);
    } else if (spec.kind === "bool") {
      const cur = (currentSnap.knobs as unknown as Record<string, boolean>)[name];
      dispatchKnobValue(name, spec, !cur);
    } else if (spec.kind === "float") {
      const input = td.querySelector("input") as HTMLInputElement;
      const v = parseFloat(input.value);
      if (!isFinite(v)) return;
      dispatchKnobValue(name, spec, v);
    } else if (spec.kind === "int") {
      const input = td.querySelector("input") as HTMLInputElement;
      const v = parseInt(input.value, 10);
      if (!isFinite(v)) return;
      dispatchKnobValue(name, spec, v);
    } else if (spec.kind === "enum") {
      const sel = td.querySelector("select") as HTMLSelectElement;
      dispatchKnobValue(name, spec, sel.value);
    }
  });
}

function dispatchKnobValue(name: string, spec: KnobSpec, value: unknown) {
  if (!currentSend) return;
  if (spec.kind === "bool") {
    currentSend({ SetBoolKnob: { knob_name: name, value: value as boolean } });
  } else if (spec.kind === "float") {
    currentSend({ SetFloatKnob: { knob_name: name, value: value as number } });
  } else if (spec.kind === "int") {
    currentSend({ SetUintKnob: { knob_name: name, value: value as number } });
  } else if (spec.kind === "enum") {
    // SetMode is the only generic enum command — one variant covers all
    // four mode knobs, disambiguated by knob_name. The other enum
    // commands (SetDischargeTime / SetDebugFullCharge / etc.) are
    // dedicated per-knob variants whose wire shape is `{value}` only.
    if (spec.cmdVariant === "SetMode") {
      currentSend({ SetMode: { knob_name: name, value: value as string } });
    } else {
      currentSend({ [spec.cmdVariant]: { value: value as string } });
    }
  }
}

export function renderKnobs(
  snap: WorldSnapshot,
  sendCommand: (cmd: unknown) => void,
) {
  currentSnap = snap;
  currentSend = sendCommand;
  installHandlers();

  // Kill-switch banner. Rendered imperatively (one fixed node; focus
  // isn't a concern here — only a toggle button).
  const kill = document.getElementById("kill-switch") as HTMLElement;
  const enabled = snap.knobs.writes_enabled;
  const killHtml = `
    <div>Kill switch: <strong class="${enabled ? "freshness-Fresh" : "freshness-Unknown"}">${enabled ? "writes ENABLED" : "writes DISABLED (observer mode)"}</strong></div>
    <button data-kill="${!enabled}">${enabled ? "Disable writes" : "Enable writes"}</button>
  `;
  if (kill.innerHTML !== killHtml) kill.innerHTML = killHtml;

  const tbody = document.querySelector("#knobs-table tbody") as HTMLElement;
  const entries = Object.entries(snap.knobs)
    .filter(([name]) => name !== "writes_enabled")
    .sort(([a], [b]) => a.localeCompare(b));

  const rows: KeyedRow[] = entries.map(([name, val]) => {
    const spec = specFor(name);
    const valStr =
      typeof val === "boolean"
        ? val ? "true" : "false"
        : typeof val === "number"
          ? fmtNum(val)
          : esc(String(val));
    const setHtml = spec ? renderSetControl(name, val, spec) : "";
    const defaultHtml = spec ? renderDefaultCell(name, val, spec) : `<span class="dim">—</span>`;
    return {
      key: name,
      cells: [
        { cls: "mono", html: entityLink(name, "knob") },
        { cls: "mono", html: valStr },
        { cls: "mono", html: defaultHtml },
        { html: setHtml },
      ],
    };
  });
  updateKeyedRows(tbody, rows);
}

/// Render the Default column: shows the spec.default value plus a
/// reset icon (↺) when the current value differs from the default.
/// Click → dispatches the appropriate Set command with the default
/// value. The icon hides when value already equals default to reduce
/// visual noise.
function renderDefaultCell(name: string, value: unknown, spec: KnobSpec): string {
  const defaultStr =
    typeof spec.default === "boolean"
      ? spec.default ? "true" : "false"
      : typeof spec.default === "number"
        ? String(spec.default)
        : esc(String(spec.default));
  const isDefault = valuesEqual(value, spec.default);
  const escName = esc(name);
  const resetBtn = isDefault
    ? ""
    : ` <button class="copy-btn icon" data-knob-action="reset" data-knob="${escName}" title="Reset to default (${defaultStr})">↺</button>`;
  return `${defaultStr}${resetBtn}`;
}

function valuesEqual(a: unknown, b: unknown): boolean {
  if (typeof a === "number" && typeof b === "number") {
    return Math.abs(a - b) < 1e-9;
  }
  return a === b;
}

function renderSetControl(name: string, value: unknown, spec: KnobSpec): string {
  const escName = esc(name);
  switch (spec.kind) {
    case "bool":
      return `<button class="secondary" data-knob-action="toggle" data-knob="${escName}">toggle</button>`;
    case "float":
    case "int":
      return `<input type="number" step="${spec.step}" min="${spec.min}" max="${spec.max}" value="${Number(value)}">
              <button data-knob-action="set" data-knob="${escName}">set</button>`;
    case "enum": {
      const cur = String(value);
      const opts = spec.options
        .map((o) => `<option value="${o}"${o === cur ? " selected" : ""}>${o}</option>`)
        .join("");
      return `<select>${opts}</select><button data-knob-action="set" data-knob="${escName}">set</button>`;
    }
  }
}
