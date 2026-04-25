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
import { entityLink, updateKeyedRows, type KeyedRow } from "./render.js";

export type KnobSpec =
  | { kind: "bool" }
  | { kind: "float"; min: number; max: number; step: number }
  | { kind: "int"; min: number; max: number; step: number }
  | { kind: "enum"; cmdVariant: string; options: string[] };

export const KNOB_SPEC: Record<string, KnobSpec> = {
  force_disable_export: { kind: "bool" },
  export_soc_threshold: { kind: "float", min: 0, max: 100, step: 1 },
  discharge_soc_target: { kind: "float", min: 0, max: 100, step: 1 },
  battery_soc_target: { kind: "float", min: 0, max: 100, step: 1 },
  full_charge_discharge_soc_target: { kind: "float", min: 0, max: 100, step: 1 },
  full_charge_export_soc_threshold: { kind: "float", min: 0, max: 100, step: 1 },
  discharge_time: { kind: "enum", cmdVariant: "SetDischargeTime", options: ["At0200", "At2300"] },
  debug_full_charge: { kind: "enum", cmdVariant: "SetDebugFullCharge", options: ["Forbid", "Force", "None_"] },
  pessimism_multiplier_modifier: { kind: "float", min: 0, max: 2, step: 0.05 },
  disable_night_grid_discharge: { kind: "bool" },
  charge_car_boost: { kind: "bool" },
  charge_car_extended: { kind: "bool" },
  zappi_current_target: { kind: "float", min: 6, max: 32, step: 0.5 },
  // A-14: kWh (per-session EV charge ceiling), not %.
  zappi_limit: { kind: "float", min: 0, max: 100, step: 0.5 },
  zappi_emergency_margin: { kind: "float", min: 0, max: 10, step: 0.5 },
  grid_export_limit_w: { kind: "int", min: 0, max: 10000, step: 50 },
  grid_import_limit_w: { kind: "int", min: 0, max: 10000, step: 10 },
  allow_battery_to_car: { kind: "bool" },
  eddi_enable_soc: { kind: "float", min: 50, max: 100, step: 1 },
  eddi_disable_soc: { kind: "float", min: 50, max: 100, step: 1 },
  eddi_dwell_s: { kind: "int", min: 0, max: 3600, step: 5 },
  weathersoc_winter_temperature_threshold: { kind: "float", min: -30, max: 40, step: 0.5 },
  weathersoc_low_energy_threshold: { kind: "float", min: 0, max: 500, step: 1 },
  weathersoc_ok_energy_threshold: { kind: "float", min: 0, max: 500, step: 1 },
  weathersoc_high_energy_threshold: { kind: "float", min: 0, max: 500, step: 1 },
  weathersoc_too_much_energy_threshold: { kind: "float", min: 0, max: 500, step: 1 },
  forecast_disagreement_strategy: {
    kind: "enum",
    cmdVariant: "SetForecastDisagreementStrategy",
    options: ["Max", "Min", "Mean", "SolcastIfAvailableElseMean"],
  },
  charge_battery_extended_mode: {
    kind: "enum",
    cmdVariant: "SetChargeBatteryExtendedMode",
    options: ["Auto", "Forced", "Disabled"],
  },
};

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
    const spec = KNOB_SPEC[name];
    if (!spec) return;
    const td = btn.closest("td");
    if (!td) return;

    if (spec.kind === "bool") {
      const cur = (currentSnap.knobs as unknown as Record<string, boolean>)[name];
      currentSend({ SetBoolKnob: { knob_name: name, value: !cur } });
    } else if (spec.kind === "float") {
      const input = td.querySelector("input") as HTMLInputElement;
      const v = parseFloat(input.value);
      if (!isFinite(v)) return;
      currentSend({ SetFloatKnob: { knob_name: name, value: v } });
    } else if (spec.kind === "int") {
      const input = td.querySelector("input") as HTMLInputElement;
      const v = parseInt(input.value, 10);
      if (!isFinite(v)) return;
      currentSend({ SetUintKnob: { knob_name: name, value: v } });
    } else if (spec.kind === "enum") {
      const sel = td.querySelector("select") as HTMLSelectElement;
      currentSend({ [spec.cmdVariant]: { value: sel.value } });
    }
  });
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
    const spec = KNOB_SPEC[name];
    const valStr =
      typeof val === "boolean"
        ? val ? "true" : "false"
        : typeof val === "number"
          ? fmtNum(val)
          : esc(String(val));
    const setHtml = spec ? renderSetControl(name, val, spec) : "";
    return {
      key: name,
      cells: [
        { cls: "mono", html: entityLink(name, "knob") },
        { cls: "mono", html: valStr },
        { html: setHtml },
      ],
    };
  });
  updateKeyedRows(tbody, rows);
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
