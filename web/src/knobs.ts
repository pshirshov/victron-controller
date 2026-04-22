// Knob section rendering + command dispatch.

import type { WorldSnapshot } from "./model/victron_controller/dashboard/WorldSnapshot.js";

type KnobSpec =
  | { kind: "bool" }
  | { kind: "float"; min: number; max: number; step: number }
  | { kind: "int"; min: number; max: number; step: number }
  | { kind: "enum"; cmdVariant: string; options: string[] };

const KNOB_SPEC: Record<string, KnobSpec> = {
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
  zappi_limit: { kind: "float", min: 1, max: 100, step: 1 },
  zappi_emergency_margin: { kind: "float", min: 0, max: 10, step: 0.5 },
  grid_export_limit_w: { kind: "int", min: 0, max: 10000, step: 50 },
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
};

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" } as Record<string, string>)[ch]!);
}

function fmtNum(v: number, digits = 3): string {
  if (!isFinite(v)) return String(v);
  const s = v.toFixed(digits);
  return s.replace(/\.?0+$/, "");
}

export function renderKnobs(
  snap: WorldSnapshot,
  sendCommand: (cmd: unknown) => void,
) {
  const kill = document.getElementById("kill-switch") as HTMLElement;
  const enabled = snap.knobs.writes_enabled;
  kill.innerHTML = `
    <div>Kill switch: <strong class="${enabled ? "freshness-Fresh" : "freshness-Unknown"}">${enabled ? "writes ENABLED" : "writes DISABLED (observer mode)"}</strong></div>
    <button data-kill="${!enabled}">${enabled ? "Disable writes" : "Enable writes"}</button>
  `;
  const killBtn = kill.querySelector("button");
  killBtn?.addEventListener("click", () => {
    sendCommand({ SetKillSwitch: { value: !enabled } });
  });

  const tbody = document.querySelector("#knobs-table tbody") as HTMLElement;
  const entries = Object.entries(snap.knobs)
    .filter(([name]) => name !== "writes_enabled")
    .sort(([a], [b]) => a.localeCompare(b));
  tbody.innerHTML = entries
    .map(([name, val]) => {
      const spec = KNOB_SPEC[name];
      const setHtml = spec ? renderSetControl(name, val, spec) : "";
      const valStr =
        typeof val === "boolean"
          ? val
            ? "true"
            : "false"
          : typeof val === "number"
            ? fmtNum(val)
            : esc(String(val));
      return `<tr>
        <td class="mono">${esc(name)}</td>
        <td class="mono">${valStr}</td>
        <td data-knob="${esc(name)}">${setHtml}</td>
      </tr>`;
    })
    .join("");

  // Attach handlers to each row's button.
  tbody.querySelectorAll("td[data-knob]").forEach((td) => {
    const name = td.getAttribute("data-knob")!;
    const spec = KNOB_SPEC[name];
    if (!spec) return;
    const btn = td.querySelector("button");
    if (!btn) return;
    btn.addEventListener("click", () => {
      if (spec.kind === "bool") {
        const cur = (snap.knobs as unknown as Record<string, boolean>)[name];
        sendCommand({ SetBoolKnob: { knob_name: name, value: !cur } });
      } else if (spec.kind === "float") {
        const input = td.querySelector("input") as HTMLInputElement;
        const v = parseFloat(input.value);
        if (!isFinite(v)) return;
        sendCommand({ SetFloatKnob: { knob_name: name, value: v } });
      } else if (spec.kind === "int") {
        const input = td.querySelector("input") as HTMLInputElement;
        const v = parseInt(input.value, 10);
        if (!isFinite(v)) return;
        sendCommand({ SetUintKnob: { knob_name: name, value: v } });
      } else if (spec.kind === "enum") {
        const sel = td.querySelector("select") as HTMLSelectElement;
        const v = sel.value;
        sendCommand({ [spec.cmdVariant]: { value: v } });
      }
    });
  });
}

function renderSetControl(_name: string, value: unknown, spec: KnobSpec): string {
  switch (spec.kind) {
    case "bool":
      return `<button class="secondary">toggle</button>`;
    case "float":
    case "int":
      return `<input type="number" step="${spec.step}" min="${spec.min}" max="${spec.max}" value="${Number(value)}">
              <button>set</button>`;
    case "enum": {
      const cur = String(value);
      const opts = spec.options
        .map((o) => `<option value="${o}"${o === cur ? " selected" : ""}>${o}</option>`)
        .join("");
      return `<select>${opts}</select><button>set</button>`;
    }
  }
}
