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

// Knobs split into two user-facing tables:
//   - "operator" — daily/weekly decisions (planner overrides, EV session
//     ad-hocs, schedule overrides). Lives at the top of the Knobs section.
//   - "config" — installation / tariff / firmware-pinned values that
//     change rarely. Lives below operator.
// `group` is a subsection label inside its table; rows inside a group
// stay alphabetically sorted (matches the prior single-table behaviour).
export type KnobCategory = "operator" | "config";

type KnobSpecBase = { category: KnobCategory; group: string };

export type KnobSpec = (
  | { kind: "bool"; default: boolean }
  | { kind: "float"; min: number; max: number; step: number; default: number }
  | { kind: "int"; min: number; max: number; step: number; default: number }
  | { kind: "enum"; cmdVariant: string; options: string[]; default: string }
) & KnobSpecBase;

// Order in which subsections are rendered inside each table. Knob entries
// reference these exact strings via `group`; rendering iterates this list
// so the on-screen order is stable and intentional rather than alpha by
// group name.
export const OPERATOR_GROUPS: ReadonlyArray<string> = [
  "Planner overrides",
  "EV / Zappi",
  "Schedule overrides",
  "Heat pump",
];
export const CONFIG_GROUPS: ReadonlyArray<string> = [
  "Tariff / scheduling",
  "Hard installation caps",
  "Forecast",
  "Eddi",
  "Zappi calibration",
  "Weather-SoC planner",
  "Actuator retry",
];

// PR-rename-entities: keyed by the dotted display name (the user-facing
// surface). Renderers look up via `displayNameOfTyped(canonical, "knob")`
// — `specFor()` below.
//
// `default` mirrors `crates/core/src/knobs.rs::Knobs::safe_defaults()`.
// Powers the "Default" column + reset icon in the Knobs table. Edit
// here when the safe_defaults change in core.
export const KNOB_SPEC: Record<string, KnobSpec> = {
  // --- Operator: Planner overrides ---
  "grid.export.force-disable": { kind: "bool", default: false, category: "operator", group: "Planner overrides" },
  "battery.soc.threshold.export.forced-value": { kind: "float", min: 0, max: 100, step: 1, default: 80, category: "operator", group: "Planner overrides" },
  "battery.soc.target.discharge.forced-value": { kind: "float", min: 0, max: 100, step: 1, default: 80, category: "operator", group: "Planner overrides" },
  "battery.soc.target.charge.forced-value": { kind: "float", min: 0, max: 100, step: 1, default: 80, category: "operator", group: "Planner overrides" },
  "grid.night.discharge.disable.forced-value": { kind: "bool", default: false, category: "operator", group: "Planner overrides" },
  // PR-gamma-hold-redesign mode knobs (4): Weather selects the
  // weathersoc-derived bookkeeping value; Forced uses the matching
  // *.forced-value knob instead. cmdVariant is `SetMode`; the Rust
  // backend dispatches via `knob_name` field on the command.
  "battery.soc.threshold.export.mode": {
    kind: "enum",
    cmdVariant: "SetMode",
    options: ["Weather", "Forced"],
    default: "Weather",
    category: "operator",
    group: "Planner overrides",
  },
  "battery.soc.target.discharge.mode": {
    kind: "enum",
    cmdVariant: "SetMode",
    options: ["Weather", "Forced"],
    default: "Weather",
    category: "operator",
    group: "Planner overrides",
  },
  "battery.soc.target.charge.mode": {
    kind: "enum",
    cmdVariant: "SetMode",
    options: ["Weather", "Forced"],
    default: "Weather",
    category: "operator",
    group: "Planner overrides",
  },
  "grid.night.discharge.disable.mode": {
    kind: "enum",
    cmdVariant: "SetMode",
    options: ["Weather", "Forced"],
    default: "Weather",
    category: "operator",
    group: "Planner overrides",
  },

  // --- Operator: EV / Zappi ---
  "evcharger.boost.enable": { kind: "bool", default: true, category: "operator", group: "EV / Zappi" },
  // PR-auto-extended-charge: tri-state mode replaces the legacy bool.
  "evcharger.extended.mode": {
    kind: "enum",
    cmdVariant: "SetExtendedChargeMode",
    options: ["Auto", "Forced", "Disabled"],
    default: "Auto",
    category: "operator",
    group: "EV / Zappi",
  },
  "evcharger.current.target": { kind: "float", min: 6, max: 32, step: 0.5, default: 9.5, category: "operator", group: "EV / Zappi" },
  // A-14: kWh (per-session EV charge ceiling), not %. Default 65 kWh
  // covers a Tesla Model 3 LR full charge and sits on the auto-stop
  // gate boundary.
  "evcharger.session.limit": { kind: "float", min: 0, max: 100, step: 0.5, default: 65, category: "operator", group: "EV / Zappi" },
  "battery.export.car.allow": { kind: "bool", default: false, category: "operator", group: "EV / Zappi" },

  // --- Operator: Schedule overrides ---
  "debug.full-charge.mode": {
    kind: "enum",
    cmdVariant: "SetDebugFullCharge",
    options: ["Forbid", "Force", "Auto"],
    default: "Auto",
    category: "operator",
    group: "Schedule overrides",
  },
  "schedule.extended.charge.mode": {
    kind: "enum",
    cmdVariant: "SetChargeBatteryExtendedMode",
    options: ["Auto", "Forced", "Disabled"],
    default: "Auto",
    category: "operator",
    group: "Schedule overrides",
  },

  // --- Config: Tariff / scheduling ---
  "battery.discharge.time": {
    kind: "enum",
    cmdVariant: "SetDischargeTime",
    options: ["At0200", "At2300"],
    default: "At0200",
    category: "config",
    group: "Tariff / scheduling",
  },
  "battery.soc.target.full-charge.discharge": { kind: "float", min: 0, max: 100, step: 1, default: 57, category: "config", group: "Tariff / scheduling" },
  "battery.soc.threshold.full-charge.export": { kind: "float", min: 0, max: 100, step: 1, default: 100, category: "config", group: "Tariff / scheduling" },

  // --- Config: Hard installation caps ---
  "grid.export.limit": { kind: "int", min: 0, max: 10000, step: 50, default: 5000, category: "config", group: "Hard installation caps" },
  "grid.import.limit": { kind: "int", min: 0, max: 10000, step: 10, default: 10, category: "config", group: "Hard installation caps" },
  // PR-inverter-safe-discharge-knob.
  "inverter.safe-discharge.enable": { kind: "bool", default: false, category: "config", group: "Hard installation caps" },

  // --- Config: Forecast ---
  "forecast.pessimism.modifier": { kind: "float", min: 0, max: 2, step: 0.05, default: 1, category: "config", group: "Forecast" },
  "forecast.disagreement.strategy": {
    kind: "enum",
    cmdVariant: "SetForecastDisagreementStrategy",
    options: ["Max", "Min", "Mean", "SolcastIfAvailableElseMean"],
    default: "SolcastIfAvailableElseMean",
    category: "config",
    group: "Forecast",
  },
  // PR-baseline-forecast: 4 runtime knobs steering the local last-resort
  // baseline. Dates are MMDD (1101 = Nov 1, 301 = Mar 1); per-hour Wh
  // are the average daylight-hour production split by season.
  // Defaults mirror `Knobs::safe_defaults` in core. Ranges mirror
  // `knob_range` in shell/src/mqtt/serialize.rs.
  "forecast.baseline.winter.start.mmdd": {
    kind: "int", min: 101, max: 1231, step: 1, default: 1101,
    category: "config", group: "Forecast",
  },
  "forecast.baseline.winter.end.mmdd": {
    kind: "int", min: 101, max: 1231, step: 1, default: 301,
    category: "config", group: "Forecast",
  },
  "forecast.baseline.wh-per-hour.winter": {
    kind: "float", min: 0, max: 10000, step: 10, default: 100,
    category: "config", group: "Forecast",
  },
  "forecast.baseline.wh-per-hour.summer": {
    kind: "float", min: 0, max: 10000, step: 10, default: 1000,
    category: "config", group: "Forecast",
  },

  // PR-keep-batteries-charged. Operator-table pair gating the daytime
  // ESS state-9 (KeepBatteriesCharged) override on full-charge days.
  "ess.full-charge.keep-batteries-charged": {
    kind: "bool", default: false,
    category: "operator", group: "Schedule overrides",
  },
  "ess.full-charge.sunrise-sunset-offset-min": {
    kind: "int", min: 0, max: 480, step: 5, default: 60,
    category: "operator", group: "Schedule overrides",
  },
  "full-charge.defer-to-next-sunday": {
    kind: "bool", default: false,
    category: "operator", group: "Schedule overrides",
  },
  "full-charge.snap-back-max-weekday": {
    kind: "int", min: 1, max: 5, step: 1, default: 3,
    category: "operator", group: "Schedule overrides",
  },

  // --- Config: Eddi ---
  "eddi.soc.enable": { kind: "float", min: 50, max: 100, step: 1, default: 96, category: "config", group: "Eddi" },
  "eddi.soc.disable": { kind: "float", min: 50, max: 100, step: 1, default: 94, category: "config", group: "Eddi" },
  "eddi.dwell.seconds": { kind: "int", min: 0, max: 3600, step: 5, default: 60, category: "config", group: "Eddi" },

  // --- Config: Zappi compensated drain (PR-ZD-2) ---
  "zappi.battery-drain.threshold-w": { kind: "int", min: 0, max: 10000, step: 50, default: 1000, category: "config", group: "Zappi compensated drain" },
  "zappi.battery-drain.relax-step-w": { kind: "int", min: 0, max: 5000, step: 25, default: 100, category: "config", group: "Zappi compensated drain" },
  "zappi.battery-drain.kp": { kind: "float", min: 0, max: 50, step: 0.05, default: 1.0, category: "config", group: "Zappi compensated drain" },
  "zappi.battery-drain.target-w": { kind: "float", min: -5000, max: 5000, step: 25, default: 0, category: "config", group: "Zappi compensated drain" },
  "zappi.battery-drain.hard-clamp-w": { kind: "int", min: 0, max: 10000, step: 25, default: 200, category: "config", group: "Zappi compensated drain" },
  // PR-ZDP-1: MPPT curtailment probe offset.
  "zappi.battery-drain.mppt-probe-w": { kind: "int", min: 0, max: 5000, step: 50, default: 500, category: "config", group: "Zappi compensated drain" },

  // --- Config: PR-ACT-RETRY-1 universal actuator retry ---
  "actuator.retry.s": { kind: "int", min: 10, max: 600, step: 1, default: 60, category: "config", group: "Actuator retry" },

  // --- Config: Zappi calibration ---
  "evcharger.current.margin": { kind: "float", min: 0, max: 10, step: 0.5, default: 5, category: "config", group: "Zappi calibration" },

  // --- Operator: Heat pump (PR-LG-THINQ-B) ---
  "lg.heat-pump.power": { kind: "bool", default: false, category: "operator", group: "Heat pump" },
  "lg.dhw.power": { kind: "bool", default: false, category: "operator", group: "Heat pump" },
  // Temperature ranges here use the defaults from LgThinqConfig; the
  // HA discovery min/max are overridden by the runtime OnceLock.
  "lg.heating-water.target-c": { kind: "int", min: 25, max: 55, step: 1, default: 42, category: "operator", group: "Heat pump" },
  "lg.dhw.target-c": { kind: "int", min: 30, max: 65, step: 1, default: 60, category: "operator", group: "Heat pump" },

  // --- Config: Weather-SoC planner ---
  "weathersoc.threshold.winter-temperature": { kind: "float", min: -30, max: 40, step: 0.5, default: 12, category: "config", group: "Weather-SoC planner" },
  "weathersoc.threshold.energy.low": { kind: "float", min: 0, max: 500, step: 1, default: 8, category: "config", group: "Weather-SoC planner" },
  "weathersoc.threshold.energy.ok": { kind: "float", min: 0, max: 500, step: 1, default: 15, category: "config", group: "Weather-SoC planner" },
  "weathersoc.threshold.energy.high": { kind: "float", min: 0, max: 500, step: 1, default: 30, category: "config", group: "Weather-SoC planner" },
  "weathersoc.threshold.energy.too-much": { kind: "float", min: 0, max: 500, step: 1, default: 45, category: "config", group: "Weather-SoC planner" },
  // PR-WSOC-TABLE-1: bucket-boundary knob for the 6×2 lookup table.
  // Default 67.5 matches the legacy `1.5 × too_much` (45 × 1.5) crossover.
  "weathersoc.threshold.energy.very-sunny": { kind: "float", min: 0, max: 500, step: 1, default: 67.5, category: "config", group: "Weather-SoC planner" },
};

// --- PR-WSOC-EDIT-1: 48 weather-SoC table cell knobs ---------------------
//
// Programmatic generator: 12 cells × 4 fields = 48 KNOB_SPEC entries.
// Defaults mirror `Knobs::safe_defaults().weather_soc_table` in core
// (drift-guarded by `web/test-fixtures/weather-soc-defaults.json` —
// see `weathersoc_table_defaults_match_core_safe_defaults` in
// `render.test.ts` and `weathersoc_defaults_fixture_matches_safe_defaults`
// in `crates/shell/src/dashboard/convert.rs`).

export const WEATHER_SOC_BUCKETS_FOR_KNOBS = [
  "very-sunny",
  "sunny",
  "mid",
  "low",
  "dim",
  "very-dim",
] as const;
export const WEATHER_SOC_TEMPS = ["warm", "cold"] as const;

/// Per-cell defaults: `[exp, bat, dis, ext]`. Mirrors
/// `Knobs::safe_defaults().weather_soc_table` in core. The drift-guard
/// test in `render.test.ts` asserts this matches the JSON fixture
/// produced by the corresponding Rust test.
export const WEATHER_SOC_DEFAULTS: Record<string, [number, number, number, boolean]> = {
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

/// Splice 48 cell-knob KNOB_SPEC entries into the registry. `category`
/// is `"config"` (install-time) and `group` is `"Weather-SoC table"` —
/// the latter is referenced for KNOB_SPEC consistency only; it doesn't
/// appear in `CONFIG_GROUPS` because the cell knobs are hidden from
/// `renderKnobs` (rendered by the dedicated widget instead).
for (const bucket of WEATHER_SOC_BUCKETS_FOR_KNOBS) {
  for (const temp of WEATHER_SOC_TEMPS) {
    const cellKey = `${bucket}.${temp}`;
    const [exp, bat, dis, ext] = WEATHER_SOC_DEFAULTS[cellKey];
    KNOB_SPEC[`weathersoc.table.${cellKey}.export-soc-threshold`] = {
      kind: "float", min: 0, max: 100, step: 1, default: exp,
      category: "config", group: "Weather-SoC table",
    };
    KNOB_SPEC[`weathersoc.table.${cellKey}.battery-soc-target`] = {
      kind: "float", min: 0, max: 100, step: 1, default: bat,
      category: "config", group: "Weather-SoC table",
    };
    KNOB_SPEC[`weathersoc.table.${cellKey}.discharge-soc-target`] = {
      kind: "float", min: 0, max: 100, step: 1, default: dis,
      category: "config", group: "Weather-SoC table",
    };
    KNOB_SPEC[`weathersoc.table.${cellKey}.extended`] = {
      kind: "bool", default: ext,
      category: "config", group: "Weather-SoC table",
    };
  }
}

/// PR-WSOC-EDIT-1: knob names rendered by the dedicated widget rather
/// than the generic flat knobs table. The 6 boundary knobs live in
/// `snap.knobs.*` directly; the 48 cell knobs ride inside
/// `weather_soc_table` and never appear on `snap.knobs` as scalar
/// fields, so they don't need entries here. `renderKnobs` skips both
/// `NESTED_KNOB_FIELDS` (which contains `weather_soc_table`) and this
/// set when bucketing rows.
export const WIDGET_RENDERED_KNOBS: ReadonlySet<string> = new Set([
  "weathersoc_winter_temperature_threshold",
  "weathersoc_low_energy_threshold",
  "weathersoc_ok_energy_threshold",
  "weathersoc_high_energy_threshold",
  "weathersoc_too_much_energy_threshold",
  "weathersoc_very_sunny_threshold",
]);

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

  // Knob tables: one delegated click handler covers both operator and
  // config tbodies. The handler fishes out the action button on click;
  // its spec lookup uses `data-knob` (the dotted display name) so both
  // tables share the same dispatcher.
  const handler = (ev: Event) => {
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
  };
  document.querySelector("#knobs-operator-table tbody")?.addEventListener("click", handler);
  document.querySelector("#knobs-config-table tbody")?.addEventListener("click", handler);
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

// Knobs whose value is a structured nested type (rendered by a dedicated widget, not the flat knobs table).
const NESTED_KNOB_FIELDS = new Set(["weather_soc_table"]);

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

  const opTbody = document.querySelector("#knobs-operator-table tbody") as HTMLElement | null;
  const cfgTbody = document.querySelector("#knobs-config-table tbody") as HTMLElement | null;
  if (!opTbody || !cfgTbody) return;

  // Bucket every knob in the snapshot by category + group based on its
  // `KNOB_SPEC` entry. `writes_enabled` lives in its own kill-switch
  // banner, not either table. Knobs without a spec (defensive — could
  // happen if backend is ahead of frontend) land in an "Other" group
  // under operator so they're at least visible.
  const opGroups = new Map<string, Array<[string, unknown]>>();
  const cfgGroups = new Map<string, Array<[string, unknown]>>();
  // PR-tier-3-ueba: see render.ts::toPlain — UEBA-decoded class
  // instances need toJSON() to expose public field names.
  const knobsPlain: Record<string, unknown> =
    typeof (snap.knobs as { toJSON?: () => unknown }).toJSON === "function"
      ? (snap.knobs as { toJSON: () => Record<string, unknown> }).toJSON()
      : (snap.knobs as unknown as Record<string, unknown>);
  Object.entries(knobsPlain).forEach(([name, val]) => {
    if (name === "writes_enabled") return;
    if (NESTED_KNOB_FIELDS.has(name)) return;
    // PR-WSOC-EDIT-1: the 6 boundary knobs are rendered inline by the
    // weather-SoC widget; suppress them from the generic Knobs tables.
    // The 48 cell knobs (`KnobId::WeathersocTableCell`) ride inside
    // `weather_soc_table` and never appear on `snap.knobs` directly,
    // so the NESTED_KNOB_FIELDS guard above already covers them.
    if (WIDGET_RENDERED_KNOBS.has(name)) return;
    const spec = specFor(name);
    if (!spec) {
      const bucket = opGroups.get("Other") ?? [];
      bucket.push([name, val]);
      opGroups.set("Other", bucket);
      return;
    }
    const target = spec.category === "operator" ? opGroups : cfgGroups;
    const bucket = target.get(spec.group) ?? [];
    bucket.push([name, val]);
    target.set(spec.group, bucket);
  });

  // Sort each bucket alphabetically by dotted display name (matches the
  // pre-split single-table behaviour for stable row positions).
  const sortBucket = (bucket: Array<[string, unknown]>) =>
    bucket.sort(([a], [b]) =>
      displayNameOfTyped(a, "knob").localeCompare(displayNameOfTyped(b, "knob")),
    );
  opGroups.forEach(sortBucket);
  cfgGroups.forEach(sortBucket);

  const opOrder = [...OPERATOR_GROUPS, "Other"];
  updateKeyedRows(opTbody, buildGroupedRows(opOrder, opGroups));
  updateKeyedRows(cfgTbody, buildGroupedRows([...CONFIG_GROUPS], cfgGroups));
}

// Render a list of groups (header row + entry rows) in `groupOrder`
// against the supplied bucket map. Empty groups are skipped so the
// table doesn't sprout headers above zero rows.
function buildGroupedRows(
  groupOrder: ReadonlyArray<string>,
  buckets: Map<string, Array<[string, unknown]>>,
): KeyedRow[] {
  const rows: KeyedRow[] = [];
  groupOrder.forEach((group) => {
    const bucket = buckets.get(group);
    if (!bucket || bucket.length === 0) return;
    rows.push({
      key: `__group__${group}`,
      cls: "knob-group-header",
      cells: [{ cls: "knob-group-label", colspan: 4, html: esc(group) }],
    });
    bucket.forEach(([name, val]) => {
      const spec = specFor(name);
      const valStr =
        typeof val === "boolean"
          ? val ? "true" : "false"
          : typeof val === "number"
            ? fmtNum(val)
            : esc(String(val));
      const setHtml = spec ? renderSetControl(name, val, spec) : "";
      const defaultHtml = spec ? renderDefaultCell(name, val, spec) : `<span class="dim">—</span>`;
      // Highlight rows whose current value drifts from the spec
      // default. `valuesEqual` mirrors the float-tolerance check used
      // inside `renderDefaultCell` so the pill and the row class agree.
      const modified = spec ? !valuesEqual(val, spec.default) : false;
      rows.push({
        key: name,
        cls: modified ? "knob-modified" : "",
        cells: [
          { cls: "mono", html: entityLink(name, "knob") },
          { cls: "mono", html: valStr },
          { cls: "mono", html: defaultHtml },
          { html: setHtml },
        ],
      });
    });
  });
  return rows;
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
