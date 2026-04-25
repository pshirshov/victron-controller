// Render helpers — convert WorldSnapshot into HTML, incrementally.
//
// Every table renderer goes through `updateKeyedRows`, which diffs
// against the existing DOM: rows are keyed by stable identifiers,
// only changed cells get their innerHTML/className replaced, and cells
// that contain the currently-focused element are left alone so a user
// typing into a knob input or selecting text in a decision factor
// isn't disrupted by the next incoming snapshot.

import type { WorldSnapshot } from "./model/victron_controller/dashboard/WorldSnapshot.js";
import type { ActualF64 } from "./model/victron_controller/dashboard/ActualF64.js";
import type { ActuatedI32 } from "./model/victron_controller/dashboard/ActuatedI32.js";
import type { ActuatedF64 } from "./model/victron_controller/dashboard/ActuatedF64.js";
import type { ActuatedEnumName } from "./model/victron_controller/dashboard/ActuatedEnumName.js";
import type { ActuatedSchedule } from "./model/victron_controller/dashboard/ActuatedSchedule.js";
import {
  bookkeepingWriters,
  entityDescriptions,
  forecastProviderLabels,
} from "./descriptions.js";
import { displayNameOfTyped } from "./displayNames.js";
import { KNOB_SPEC, type KnobSpec } from "./knobs.js";

// Entity types recognised by the inspector dispatcher.
export type EntityType =
  | "sensor"
  | "knob"
  | "actuated"
  | "bookkeeping"
  | "decision"
  | "forecast"
  | "core"
  | "timer";

// --- incremental update primitives ---------------------------------------

export type RowCell = { cls?: string; html: string };
export type KeyedRow = { key: string; cells: RowCell[] };

/// Diff-update the children of `tbody` so they match `rows` exactly,
/// by row key. Existing rows are kept in place; only cells whose class
/// or innerHTML has actually changed are written to the DOM. Cells
/// that contain `document.activeElement` are never touched — that's
/// what makes focused inputs (knob values, search highlighting, text
/// selection inside a cell) survive a snapshot tick.
export function updateKeyedRows(tbody: HTMLElement, rows: KeyedRow[]): void {
  const active = document.activeElement;
  const existing = new Map<string, HTMLTableRowElement>();
  Array.from(tbody.children).forEach((el) => {
    const tr = el as HTMLTableRowElement;
    const k = tr.dataset.key;
    if (k !== undefined) existing.set(k, tr);
  });

  const seen = new Set<string>();
  rows.forEach((row, idx) => {
    seen.add(row.key);
    let tr = existing.get(row.key);
    if (!tr) {
      tr = document.createElement("tr");
      tr.dataset.key = row.key;
    }
    while (tr.children.length < row.cells.length) tr.appendChild(document.createElement("td"));
    while (tr.children.length > row.cells.length) tr.removeChild(tr.lastChild!);
    row.cells.forEach((cell, i) => {
      const td = tr!.children[i] as HTMLTableCellElement;
      if (active && td.contains(active)) return;
      const cls = cell.cls ?? "";
      if (td.className !== cls) td.className = cls;
      if (td.innerHTML !== cell.html) td.innerHTML = cell.html;
    });
    // Position row at the correct index without disturbing untouched ones.
    if (tbody.children[idx] !== tr) tbody.insertBefore(tr, tbody.children[idx] ?? null);
  });

  // Drop rows that no longer appear.
  Array.from(tbody.children).forEach((el) => {
    const tr = el as HTMLTableRowElement;
    const k = tr.dataset.key;
    if (k !== undefined && !seen.has(k)) tr.remove();
  });
}

// --- formatting helpers --------------------------------------------------

function fmtNum(v: number | null | undefined, digits = 1): string {
  if (v === null || v === undefined) return "—";
  if (!isFinite(v)) return String(v);
  return v.toFixed(digits);
}

function fmtEpoch(ms: number): string {
  if (!ms) return "—";
  // Clamp future timestamps (tiny clock skew between Venus and browser).
  const dt = Math.max(0, (Date.now() - ms) / 1000);
  if (dt < 60) return `${dt.toFixed(0)} s ago`;
  if (dt < 3600) return `${(dt / 60).toFixed(0)} min ago`;
  return new Date(ms).toLocaleString();
}

function esc(s: string): string {
  return s.replace(/[&<>]/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" } as Record<string, string>)[ch]!);
}

// Render the canonical entity name as a clickable inspector link.
// Click handling is delegated in `web/src/index.ts`. The description
// lives inside the popup, not in a hover tooltip — discoverability
// moves from "hover" to "click" (see the "click any name to inspect"
// hint at the top of the dashboard).
//
// PR-rename-entities: `data-entity-id` carries the snake_case canonical
// key (matches snapshot field names — what the inspector dispatcher
// uses to read values out of `snap.knobs.<id>` etc). The visible text
// is the dotted display name for the entity's type; same canonical may
// resolve to different dotted names across classes (actuated vs
// decision vs core).
export function entityLink(name: string, type: EntityType): string {
  const visible = displayNameOfTyped(name, type);
  return `<a class="entity-link" data-entity-id="${esc(name)}" data-entity-type="${type}">${esc(visible)}</a>`;
}

// Compact identifier copy button: a faint icon glyph that lives inline
// after the entity name in the same cell. Hover-darkens via CSS.
function copyIcon(identifier: string): string {
  const ident = esc(identifier);
  return `<button class="copy-btn icon" data-copy="${ident}" title="Copy ${ident}">⧉</button>`;
}

// Boolean → coloured badge. The string forms `"true"` / `"false"` show
// up everywhere bookkeeping/decision factors stringify booleans, so
// detection on the literal string is robust to JSON-vs-typed origins.
// The textual key stays visible elsewhere; this replaces only the value.
function boolBadge(value: boolean): string {
  return value
    ? '<span class="bool-badge bool-true" title="true">✓</span>'
    : '<span class="bool-badge bool-false" title="false">✗</span>';
}

// Convert `"true"` / `"false"` (or the booleans themselves) to a badge,
// passing every other value through unchanged. Defensive: applied
// everywhere a stringified boolean might surface.
function maybeBoolBadge(value: string): string | null {
  if (value === "true") return boolBadge(true);
  if (value === "false") return boolBadge(false);
  return null;
}

// Format a duration in ms as "500ms", "2s", "30s", "15m", "1h 30m".
function fmtDurationMs(totalMs: number): string {
  if (!isFinite(totalMs) || totalMs <= 0) return "—";
  if (totalMs < 1000) return `${totalMs}ms`;
  const totalSec = Math.round(totalMs / 1000);
  if (totalSec < 60) return `${totalSec}s`;
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  if (m < 60) return s === 0 ? `${m}m` : `${m}m ${s}s`;
  const h = Math.floor(m / 60);
  const mm = m % 60;
  return mm === 0 ? `${h}h` : `${h}h ${mm}m`;
}

// --- table renderers -----------------------------------------------------

export function renderSensors(snap: WorldSnapshot) {
  const tbody = document.querySelector("#sensors-table tbody") as HTMLElement;
  const entries = Object.entries(snap.sensors).sort(([a], [b]) => a.localeCompare(b));
  const meta = snap.sensors_meta as unknown as Record<
    string,
    { origin: string; identifier: string; cadence_ms: number; staleness_ms: number }
  >;
  const rows: KeyedRow[] = entries.map(([name, a]) => {
    const act = a as ActualF64;
    const valText = act.value === null ? "—" : fmtNum(act.value, 2);
    const mm = meta[name];
    const origin = mm ? esc(mm.origin) : `<span class="dim">—</span>`;
    const cadence = mm ? fmtDurationMs(mm.cadence_ms) : `<span class="dim">—</span>`;
    const staleness = mm ? fmtDurationMs(mm.staleness_ms) : `<span class="dim">—</span>`;
    const nameCell = mm
      ? `${entityLink(name, "sensor")} ${copyIcon(mm.identifier)}`
      : entityLink(name, "sensor");
    return {
      key: name,
      cells: [
        { cls: "mono", html: nameCell },
        { cls: "mono", html: valText },
        {
          cls: `freshness-${act.freshness}`,
          html: `${act.freshness} <span class="dim">(${fmtEpoch(
            act.since_epoch_ms as unknown as number,
          )})</span>`,
        },
        { cls: "mono", html: cadence },
        { cls: "mono", html: staleness },
        { cls: "mono", html: origin },
      ],
    };
  });
  updateKeyedRows(tbody, rows);
}

function fmtScheduleTime(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

/** Render a ScheduleSpec as "ENABLED 02:00–05:00 soc=80" / "DISABLED 02:00–05:00 soc=80" / "—".
 *  Victron encodes the day mask as 7 = every day enabled, -7 = every day disabled.
 *  Users read raw JSON `{"days":-7}` as "disabled" (correctly — that IS the wire code for disabled),
 *  but the JSON view is ambiguous on purpose; this formatter makes the enabled/disabled bit unambiguous. */
function fmtSchedule(
  spec: { start_s: number; duration_s: number; discharge: number; soc: number; days: number } | undefined
): string {
  if (!spec) return "—";
  const enabled = spec.days === 7;
  const label = enabled
    ? '<span class="freshness-Fresh">ENABLED</span>'
    : '<span class="freshness-Stale">DISABLED</span>';
  const start = fmtScheduleTime(spec.start_s);
  const end = fmtScheduleTime(spec.start_s + spec.duration_s);
  const soc = Math.round(spec.soc);
  return `${label} <span class="dim">${start}–${end} soc=${soc}%</span>`;
}

export function renderActuated(snap: WorldSnapshot) {
  const tbody = document.querySelector("#actuated-table tbody") as HTMLElement;
  const a = snap.actuated;

  const mkRow = (
    key: string,
    target: string,
    owner: string,
    phase: string,
    actual: string,
    fresh: string,
    since: number,
  ): KeyedRow => ({
    key,
    cells: [
      { cls: "mono", html: entityLink(key, "actuated") },
      { cls: "mono", html: target },
      { html: owner },
      { cls: `phase-${phase}`, html: phase },
      { cls: "mono", html: actual },
      { cls: `freshness-${fresh}`, html: `${fresh} <span class="dim">(${fmtEpoch(since)})</span>` },
    ],
  });

  const gs: ActuatedI32 = a.grid_setpoint;
  const cl: ActuatedF64 = a.input_current_limit;
  const zm: ActuatedEnumName = a.zappi_mode;
  const em: ActuatedEnumName = a.eddi_mode;
  const s0: ActuatedSchedule = a.schedule_0;
  const s1: ActuatedSchedule = a.schedule_1;

  const rows: KeyedRow[] = [
    mkRow(
      "grid_setpoint",
      gs.target_value === null ? "—" : String(gs.target_value),
      String(gs.target_owner),
      String(gs.target_phase),
      gs.actual.value === null ? "—" : String(gs.actual.value),
      String(gs.actual.freshness),
      gs.actual.since_epoch_ms as unknown as number,
    ),
    mkRow(
      "input_current_limit",
      cl.target_value === null ? "—" : fmtNum(cl.target_value, 2),
      String(cl.target_owner),
      String(cl.target_phase),
      cl.actual.value === null ? "—" : fmtNum(cl.actual.value, 2),
      String(cl.actual.freshness),
      cl.actual.since_epoch_ms as unknown as number,
    ),
    mkRow(
      "zappi_mode",
      zm.target_value ?? "—",
      String(zm.target_owner),
      String(zm.target_phase),
      zm.actual_value ?? "—",
      String(zm.actual_freshness),
      zm.actual_since_epoch_ms as unknown as number,
    ),
    mkRow(
      "eddi_mode",
      em.target_value ?? "—",
      String(em.target_owner),
      String(em.target_phase),
      em.actual_value ?? "—",
      String(em.actual_freshness),
      em.actual_since_epoch_ms as unknown as number,
    ),
    mkRow(
      "schedule_0",
      fmtSchedule(s0.target),
      String(s0.target_owner),
      String(s0.target_phase),
      fmtSchedule(s0.actual),
      String(s0.actual_freshness),
      s0.actual_since_epoch_ms as unknown as number,
    ),
    mkRow(
      "schedule_1",
      fmtSchedule(s1.target),
      String(s1.target_owner),
      String(s1.target_phase),
      fmtSchedule(s1.actual),
      String(s1.actual_freshness),
      s1.actual_since_epoch_ms as unknown as number,
    ),
  ];
  updateKeyedRows(tbody, rows);
}

export function renderBookkeeping(snap: WorldSnapshot) {
  const tbody = document.querySelector("#bk-table tbody") as HTMLElement;
  const rows: KeyedRow[] = Object.entries(snap.bookkeeping).map(([name, val]) => {
    let disp: string;
    if (val === null || val === undefined) disp = "—";
    else if (typeof val === "boolean") disp = boolBadge(val);
    else if (typeof val === "number") disp = fmtNum(val, 2);
    else {
      const s = String(val);
      disp = maybeBoolBadge(s) ?? esc(s);
    }
    return {
      key: name,
      cells: [
        { cls: "mono", html: entityLink(name, "bookkeeping") },
        { html: disp },
      ],
    };
  });
  updateKeyedRows(tbody, rows);
}

export function renderDecisions(snap: WorldSnapshot) {
  const tbody = document.querySelector("#decisions-table tbody") as HTMLElement;
  const d = snap.decisions;
  const ordered: Array<[string, any]> = [
    ["grid_setpoint", d.grid_setpoint],
    ["input_current_limit", d.input_current_limit],
    ["schedule_0", d.schedule_0],
    ["schedule_1", d.schedule_1],
    ["zappi_mode", d.zappi_mode],
    ["eddi_mode", d.eddi_mode],
    ["weather_soc", d.weather_soc],
  ];
  const rows: KeyedRow[] = ordered.map(([name, dec]) => {
    if (!dec) {
      return {
        key: name,
        cells: [
          { cls: "mono", html: entityLink(name, "decision") },
          { cls: "dim", html: "—" },
          { cls: "dim", html: "—" },
        ],
      };
    }
    const factors = (dec.factors as Array<{ name: string; value: string }>)
      .map((f) => {
        const valHtml = maybeBoolBadge(f.value) ?? esc(f.value);
        return `<span class="factor"><b>${esc(f.name)}</b>=${valHtml}</span>`;
      })
      .join(" ");
    return {
      key: name,
      cells: [
        { cls: "mono", html: entityLink(name, "decision") },
        { html: esc(dec.summary as string) },
        { cls: "factors", html: factors },
      ],
    };
  });
  updateKeyedRows(tbody, rows);
}

// PR-tass-dag-view: TASS-core DAG view. One row per core in canonical
// topological order; columns mirror the CoreState wire fields.
export function renderCoresState(snap: WorldSnapshot) {
  const tbody = document.querySelector("#cores-table tbody") as HTMLElement;
  const cs = snap.cores_state as unknown as {
    cores: Array<{ id: string; depends_on: string[]; last_run_outcome: string; last_payload: string | null | undefined }>;
    topo_order: string[];
  };
  const cores = cs?.cores ?? [];
  const rows: KeyedRow[] = cores.map((c) => {
    const deps = c.depends_on.length === 0
      ? '<span class="dim">—</span>'
      : c.depends_on.map((d) => esc(d)).join(", ");
    const payload = (() => {
      const v = c.last_payload;
      if (v === null || v === undefined) return '<span class="dim">—</span>';
      const badge = maybeBoolBadge(v);
      if (badge !== null) return badge;
      return esc(v);
    })();
    return {
      key: c.id,
      cells: [
        // Core name is a clickable link that opens the inspector modal
        // (renderEntityModal with type="core"). Description lives in
        // the popup; click is wired in `web/src/index.ts`.
        { cls: "mono", html: entityLink(c.id, "core") },
        { cls: "mono", html: deps },
        { html: esc(c.last_run_outcome) },
        { html: payload },
      ],
    };
  });
  updateKeyedRows(tbody, rows);
}

// PR-timers-section: render the per-timer table. Columns mirror the
// wire `Timer` shape: id | description | period | last_fire | next_fire
// | status. Periods of zero render as "—" via fmtDurationMs.
type TimerRow = {
  id: string;
  description: string;
  period_ms: number;
  last_fire_epoch_ms: number | null;
  next_fire_epoch_ms: number | null;
  status: string;
};

export function renderTimers(snap: WorldSnapshot) {
  const tbody = document.querySelector("#timers-table tbody") as HTMLElement;
  if (!tbody) return;
  const t = snap.timers as unknown as { entries?: TimerRow[] } | undefined;
  const entries = t?.entries ?? [];
  const rows: KeyedRow[] = entries.map((e) => {
    const last = e.last_fire_epoch_ms ?? 0;
    const next = e.next_fire_epoch_ms;
    return {
      key: e.id,
      cells: [
        { cls: "mono", html: entityLink(e.id, "timer") },
        { html: esc(e.description) },
        { cls: "mono", html: fmtDurationMs(e.period_ms) },
        { cls: "dim", html: last ? fmtEpoch(last) : "—" },
        { cls: "dim", html: next ? fmtEpoch(next) : "—" },
        { html: esc(e.status) },
      ],
    };
  });
  updateKeyedRows(tbody, rows);
}

function renderTimerBody(entityId: string, snap: WorldSnapshot): string {
  const t = snap.timers as unknown as { entries?: TimerRow[] } | undefined;
  const entry = t?.entries?.find((e) => e.id === entityId);
  const sections: string[] = [descriptionSection(entityId, "timer")];
  if (!entry) {
    sections.push(`<section><p>no timer "${esc(entityId)}" in snapshot</p></section>`);
    return sections.filter(Boolean).join("");
  }
  const last = entry.last_fire_epoch_ms ?? 0;
  const next = entry.next_fire_epoch_ms;
  sections.push(
    `<section><h3>Timer</h3>` +
      `<table><tbody>` +
      `<tr><th>id</th><td>${esc(entry.id)}</td></tr>` +
      `<tr><th>description</th><td>${esc(entry.description)}</td></tr>` +
      `<tr><th>period</th><td>${esc(fmtDurationMs(entry.period_ms))}</td></tr>` +
      `<tr><th>last fire</th><td>${last ? esc(fmtEpoch(last)) : "—"}</td></tr>` +
      `<tr><th>next fire</th><td>${next ? esc(fmtEpoch(next)) : "—"}</td></tr>` +
      `<tr><th>status</th><td>${esc(entry.status)}</td></tr>` +
      `</tbody></table></section>`,
  );
  return sections.filter(Boolean).join("");
}

/// Render the entity inspector modal for `entityId` of `type` against
/// the current snapshot. Idempotent — safe to call on every
/// applySnapshot while the modal is open so the body refreshes live.
export function renderEntityModal(
  entityId: string,
  type: EntityType,
  snap: WorldSnapshot,
) {
  const titleEl = document.getElementById("entity-modal-title");
  const bodyEl = document.getElementById("entity-modal-body");
  if (!titleEl || !bodyEl) return;
  titleEl.textContent = `${type}: ${entityId}`;

  let html = "";
  switch (type) {
    case "sensor":      html = renderSensorBody(entityId, snap); break;
    case "knob":        html = renderKnobBody(entityId, snap); break;
    case "actuated":    html = renderActuatedBody(entityId, snap); break;
    case "bookkeeping": html = renderBookkeepingBody(entityId, snap); break;
    case "decision":    html = renderDecisionBody(entityId, snap); break;
    case "forecast":    html = renderForecastBody(entityId, snap); break;
    case "core":        html = renderCoreBody(entityId, snap); break;
    case "timer":       html = renderTimerBody(entityId, snap); break;
  }
  bodyEl.innerHTML = html;
}

// --- per-type modal bodies ---------------------------------------------

function descriptionSection(entityId: string, type?: EntityType): string {
  const desc = entityDescriptionFor(entityId, type);
  if (!desc) return "";
  return `<section><p style="color:var(--muted);margin:0">${esc(desc)}</p></section>`;
}

// PR-rename-entities: `entityDescriptions` is keyed by the dotted display
// name (the user-facing surface). Translate the canonical id before
// lookup. When `type` is provided, use the type-aware display name so
// collision keys (zappi_mode, eddi_mode, weather_soc, schedule_0/1)
// resolve to the right dotted form per class.
function entityDescriptionFor(key: string, type?: EntityType): string {
  const dotted = type ? displayNameOfTyped(key, type) : displayNameOfTyped(key, "");
  return (entityDescriptions as Record<string, string>)[dotted]
    ?? (entityDescriptions as Record<string, string>)[key]
    ?? "";
}

function renderFactorTable(
  rows: Array<{ name: string; value: string }> | undefined,
): string {
  if (!rows || rows.length === 0) {
    return '<p class="dim" style="margin:0">—</p>';
  }
  const tr = rows
    .map((f) => {
      const v = maybeBoolBadge(f.value) ?? esc(f.value);
      return `<tr><th>${esc(f.name)}</th><td>${v}</td></tr>`;
    })
    .join("");
  return `<table><tbody>${tr}</tbody></table>`;
}

function renderSensorBody(entityId: string, snap: WorldSnapshot): string {
  const sensors = snap.sensors as unknown as Record<string, ActualF64 | undefined>;
  const meta = snap.sensors_meta as unknown as Record<
    string,
    { origin: string; identifier: string; cadence_ms: number; staleness_ms: number } | undefined
  >;
  const a = sensors[entityId];
  const mm = meta[entityId];

  const sections: string[] = [descriptionSection(entityId, "sensor")];
  if (!a) {
    sections.push(`<section><p>no sensor "${esc(entityId)}" in snapshot</p></section>`);
    return sections.filter(Boolean).join("");
  }

  const valText = a.value === null ? "—" : fmtNum(a.value, 2);
  const since = a.since_epoch_ms as unknown as number;
  sections.push(
    `<section><h3>Current value</h3>` +
      `<table><tbody>` +
      `<tr><th>value</th><td>${valText}</td></tr>` +
      `<tr><th>freshness</th><td class="freshness-${esc(String(a.freshness))}">${esc(String(a.freshness))}</td></tr>` +
      `<tr><th>age</th><td>${esc(fmtEpoch(since))}</td></tr>` +
      `</tbody></table></section>`,
  );

  if (mm) {
    const ident = esc(mm.identifier);
    sections.push(
      `<section><h3>Origin</h3>` +
        `<table><tbody>` +
        `<tr><th>origin</th><td>${esc(mm.origin)}</td></tr>` +
        `<tr><th>identifier</th><td>${ident} ${copyIcon(mm.identifier)}</td></tr>` +
        `<tr><th>cadence</th><td>${esc(fmtDurationMs(mm.cadence_ms))}</td></tr>` +
        `<tr><th>stale after</th><td>${esc(fmtDurationMs(mm.staleness_ms))}</td></tr>` +
        `</tbody></table></section>`,
    );
  }
  return sections.filter(Boolean).join("");
}

function renderKnobBody(entityId: string, snap: WorldSnapshot): string {
  const knobs = snap.knobs as unknown as Record<string, unknown>;
  const val = knobs[entityId];
  // KNOB_SPEC is keyed by dotted display name (PR-rename-entities); the
  // canonical id maps to it via displayNameOfTyped.
  const spec: KnobSpec | undefined = KNOB_SPEC[displayNameOfTyped(entityId, "knob")] ?? KNOB_SPEC[entityId];

  const sections: string[] = [descriptionSection(entityId, "knob")];

  const valDisp = (() => {
    if (val === undefined) return '<span class="dim">—</span>';
    if (typeof val === "boolean") return val ? "true" : "false";
    if (typeof val === "number") return fmtNum(val, 3);
    return esc(String(val));
  })();
  sections.push(
    `<section><h3>Current value</h3>` +
      `<table><tbody>` +
      `<tr><th>value</th><td>${valDisp}</td></tr>` +
      `</tbody></table></section>`,
  );

  if (spec) {
    const rangeRows: string[] = [`<tr><th>kind</th><td>${esc(spec.kind)}</td></tr>`];
    if (spec.kind === "float" || spec.kind === "int") {
      rangeRows.push(`<tr><th>min</th><td>${spec.min}</td></tr>`);
      rangeRows.push(`<tr><th>max</th><td>${spec.max}</td></tr>`);
      rangeRows.push(`<tr><th>step</th><td>${spec.step}</td></tr>`);
    } else if (spec.kind === "enum") {
      rangeRows.push(`<tr><th>options</th><td>${spec.options.map(esc).join(", ")}</td></tr>`);
    }
    sections.push(
      `<section><h3>Range</h3><table><tbody>${rangeRows.join("")}</tbody></table></section>`,
    );
  }

  // Knob owner provenance was relevant during γ-hold (PR-11) when
  // controller-driven writes had to defer to dashboard writes for 1 s.
  // PR-gamma-hold-redesign deleted γ-hold entirely — knobs are
  // user-only now (Dashboard / HaMqtt / System write paths, no
  // priority queue) — so "last-write owner" carries no actionable
  // information and the row is omitted.
  return sections.filter(Boolean).join("");
}

function renderActuatedBody(entityId: string, snap: WorldSnapshot): string {
  const a = snap.actuated as unknown as Record<string, any>;
  const ent = a[entityId];
  const sections: string[] = [descriptionSection(entityId, "actuated")];
  if (!ent) {
    sections.push(`<section><p>no actuated "${esc(entityId)}" in snapshot</p></section>`);
    return sections.filter(Boolean).join("");
  }

  // Target side: target_value / target_owner / target_phase / target_set_at_epoch_ms.
  const targetValue = (() => {
    if ("target_value" in ent) {
      const v = ent.target_value;
      if (v === null || v === undefined) return "—";
      return typeof v === "number" ? fmtNum(v, 2) : String(v);
    }
    if ("target" in ent) return fmtSchedule(ent.target);
    return "—";
  })();
  const targetOwner = "target_owner" in ent ? String(ent.target_owner) : "—";
  const targetPhase = "target_phase" in ent ? String(ent.target_phase) : "—";
  const targetSetAt: number | undefined =
    (ent.target_set_at_epoch_ms as number | undefined) ??
    (ent.target_since_epoch_ms as number | undefined);
  const targetAge = targetSetAt ? fmtEpoch(targetSetAt) : "—";

  sections.push(
    `<section><h3>Target</h3>` +
      `<table><tbody>` +
      `<tr><th>value</th><td>${esc(targetValue)}</td></tr>` +
      `<tr><th>owner</th><td>${esc(targetOwner)}</td></tr>` +
      `<tr><th>phase</th><td class="phase-${esc(targetPhase)}">${esc(targetPhase)}</td></tr>` +
      `<tr><th>age since target_set</th><td>${esc(targetAge)}</td></tr>` +
      `</tbody></table></section>`,
  );

  // Actual side: shape varies — ActuatedI32/F64 nest it in `.actual`,
  // ActuatedEnumName/Schedule flatten via actual_*.
  let actualValue = "—";
  let actualFresh = "—";
  let actualSince: number | undefined;
  if (ent.actual && typeof ent.actual === "object" && "value" in ent.actual) {
    const v = ent.actual.value;
    actualValue = v === null || v === undefined
      ? "—"
      : typeof v === "number" ? fmtNum(v, 2) : String(v);
    actualFresh = String(ent.actual.freshness);
    actualSince = ent.actual.since_epoch_ms as number;
  } else if (ent.actual && typeof ent.actual === "object") {
    // schedule's actual is a ScheduleSpec
    actualValue = fmtSchedule(ent.actual);
    actualFresh = String(ent.actual_freshness);
    actualSince = ent.actual_since_epoch_ms as number;
  } else if ("actual_value" in ent) {
    actualValue = ent.actual_value ?? "—";
    actualFresh = String(ent.actual_freshness);
    actualSince = ent.actual_since_epoch_ms as number;
  }
  const actualAge = actualSince ? fmtEpoch(actualSince) : "—";
  sections.push(
    `<section><h3>Actual</h3>` +
      `<table><tbody>` +
      `<tr><th>value</th><td>${actualValue}</td></tr>` +
      `<tr><th>freshness</th><td class="freshness-${esc(actualFresh)}">${esc(actualFresh)}</td></tr>` +
      `<tr><th>age</th><td>${esc(actualAge)}</td></tr>` +
      `</tbody></table></section>`,
  );

  // Decision summary linking to the matching decision entity.
  const decision = (snap.decisions as Record<string, any> | undefined)?.[entityId];
  const decLink = entityLink(entityId, "decision");
  if (decision && typeof decision === "object" && "summary" in decision) {
    sections.push(
      `<section><h3>Decision</h3>` +
        `<p style="margin:0 0 4px">${decLink}</p>` +
        `<p style="margin:0"><b>${esc(decision.summary as string)}</b></p>` +
        `</section>`,
    );
  } else {
    sections.push(
      `<section><h3>Decision</h3>` +
        `<p style="margin:0 0 4px">${decLink}</p>` +
        `<p class="dim" style="margin:0">no Decision recorded</p>` +
        `</section>`,
    );
  }
  return sections.filter(Boolean).join("");
}

function renderBookkeepingBody(entityId: string, snap: WorldSnapshot): string {
  const bk = snap.bookkeeping as unknown as Record<string, unknown>;
  const sections: string[] = [descriptionSection(entityId, "bookkeeping")];

  const val = bk[entityId];
  const valDisp = (() => {
    if (val === null || val === undefined) return "—";
    if (typeof val === "boolean") return boolBadge(val);
    if (typeof val === "number") return fmtNum(val, 2);
    const s = String(val);
    return maybeBoolBadge(s) ?? esc(s);
  })();
  sections.push(
    `<section><h3>Current value</h3>` +
      `<table><tbody>` +
      `<tr><th>value</th><td>${valDisp}</td></tr>` +
      `</tbody></table></section>`,
  );

  // bookkeepingWriters is keyed by the dotted display name post
  // PR-rename-entities; canonical lookup is a fallback for safety.
  const dotted = displayNameOfTyped(entityId, "bookkeeping");
  const writers = bookkeepingWriters[dotted] ?? bookkeepingWriters[entityId];
  const writersHtml = writers && writers.length > 0
    ? writers.map((w) => entityLink(w, "core")).join(", ")
    : '<span class="dim">—</span>';
  sections.push(
    `<section><h3>Writers</h3>` +
      `<p style="margin:0">${writersHtml}</p>` +
      `</section>`,
  );
  // Per-field last-write-at for bookkeeping isn't tracked: most
  // bookkeeping fields are recomputed every tick from world state
  // (e.g. `effective_export_soc_threshold`, `weathersoc.derived.*`),
  // so "last write" is meaningless. The few that ARE event-driven
  // (`next_full_charge`, `above_soc_date`) are stamped at the value
  // itself. Use the Decision panel of the writing core to see when
  // a field's value was decided.
  return sections.filter(Boolean).join("");
}

function renderDecisionBody(entityId: string, snap: WorldSnapshot): string {
  const d = (snap.decisions as Record<string, any> | undefined)?.[entityId];
  const sections: string[] = [descriptionSection(entityId, "decision")];
  if (!d || typeof d !== "object" || !("summary" in d)) {
    sections.push(
      `<section><h3>Decision</h3>` +
        `<p class="dim" style="margin:0">no Decision recorded for "${esc(entityId)}"</p></section>`,
    );
    return sections.filter(Boolean).join("");
  }
  const dec = d as { summary?: string; factors?: Array<{ name: string; value: string }> };
  const summary = dec.summary ? esc(dec.summary) : "—";
  sections.push(
    `<section><h3>Summary</h3><p style="margin:0"><b>${summary}</b></p></section>`,
  );
  sections.push(
    `<section><h3>Factors</h3>${renderFactorTable(dec.factors)}</section>`,
  );
  return sections.filter(Boolean).join("");
}

function renderForecastBody(entityId: string, snap: WorldSnapshot): string {
  const f = (snap.forecasts as unknown as Record<string, any> | undefined)?.[entityId];
  const sections: string[] = [descriptionSection(entityId, "forecast")];
  // forecastProviderLabels is keyed by the dotted display name.
  const dotted = displayNameOfTyped(entityId, "forecast");
  const provider = forecastProviderLabels[dotted] ?? forecastProviderLabels[entityId] ?? entityId;
  if (!f) {
    sections.push(
      `<section><h3>Provider</h3>` +
        `<table><tbody>` +
        `<tr><th>provider</th><td>${esc(provider)}</td></tr>` +
        `<tr><th>data</th><td class="dim">no data</td></tr>` +
        `</tbody></table></section>`,
    );
    return sections.filter(Boolean).join("");
  }
  sections.push(
    `<section><h3>Provider</h3>` +
      `<table><tbody>` +
      `<tr><th>provider</th><td>${esc(provider)}</td></tr>` +
      `<tr><th>today_kwh</th><td>${fmtNum(f.today_kwh, 1)}</td></tr>` +
      `<tr><th>tomorrow_kwh</th><td>${fmtNum(f.tomorrow_kwh, 1)}</td></tr>` +
      `<tr><th>last fetch</th><td>${esc(fmtEpoch(f.fetched_at_epoch_ms))}</td></tr>` +
      `</tbody></table></section>`,
  );
  return sections.filter(Boolean).join("");
}

function renderCoreBody(entityId: string, snap: WorldSnapshot): string {
  // PR-rename-entities: cores arrive in the snapshot with their dotted
  // form already (CoreState.id is filled from CoreId::name() which now
  // returns dotted). The description registry is keyed on dotted names,
  // so descriptionSection lookup-by-typed handles either flavor — but
  // the bookkeeping writers list still uses the dotted form too.
  const cs = snap.cores_state as unknown as {
    cores: Array<{
      id: string;
      depends_on: string[];
      last_run_outcome: string;
      last_payload: string | null | undefined;
      last_inputs?: Array<{ name: string; value: string }>;
      last_outputs?: Array<{ name: string; value: string }>;
    }>;
  };
  const core = cs?.cores?.find((c) => c.id === entityId);
  const decision = (snap.decisions as Record<string, any> | undefined)?.[entityId];

  const sections: string[] = [descriptionSection(entityId, "core")];

  if (core) {
    const depsTxt = core.depends_on.length === 0
      ? '<span class="dim">—</span>'
      : core.depends_on.map(esc).join(", ");
    const payload = core.last_payload;
    const payloadDisp = (() => {
      if (payload == null) return '<span class="dim">—</span>';
      const badge = maybeBoolBadge(payload);
      return badge ?? esc(payload);
    })();
    sections.push(
      `<section><h3>Dependencies & outcome</h3>` +
        `<table><tbody>` +
        `<tr><th>depends_on</th><td>${depsTxt}</td></tr>` +
        `<tr><th>last_run_outcome</th><td>${esc(core.last_run_outcome)}</td></tr>` +
        `<tr><th>last_payload</th><td>${payloadDisp}</td></tr>` +
        `</tbody></table></section>`,
    );
    sections.push(`<section><h3>Inputs</h3>${renderFactorTable(core.last_inputs)}</section>`);
    sections.push(`<section><h3>Outputs</h3>${renderFactorTable(core.last_outputs)}</section>`);
  } else {
    sections.push(`<section><p>no entry in cores_state for "${esc(entityId)}"</p></section>`);
  }

  if (decision && typeof decision === "object" && "summary" in decision) {
    const d = decision as { summary?: string; factors?: Array<{ name: string; value: string }> };
    const summary = d.summary ? esc(d.summary) : "—";
    const factors = (d.factors ?? [])
      .map((f) => {
        const v = maybeBoolBadge(f.value) ?? esc(f.value);
        return `<tr><th>${esc(f.name)}</th><td>${v}</td></tr>`;
      })
      .join("");
    sections.push(
      `<section><h3>Decision</h3>` +
        `<p style="margin:0 0 8px"><b>${summary}</b></p>` +
        (factors ? `<table><tbody>${factors}</tbody></table>` : "") +
        `</section>`,
    );
  } else {
    sections.push(
      `<section><h3>Decision</h3><p class="dim" style="margin:0">no Decision recorded for this core (e.g. derivation cores or SensorBroadcastCore record per-tick state via last_payload only)</p></section>`,
    );
  }
  return sections.filter(Boolean).join("");
}

export function renderForecasts(snap: WorldSnapshot) {
  const tbody = document.querySelector("#forecasts-table tbody") as HTMLElement;
  const providers: Array<[string, any]> = [
    ["solcast", snap.forecasts.solcast],
    ["forecast_solar", snap.forecasts.forecast_solar],
    ["open_meteo", snap.forecasts.open_meteo],
  ];
  const rows: KeyedRow[] = providers.map(([name, f]) => {
    if (!f) {
      return {
        key: name,
        cells: [
          { cls: "mono", html: entityLink(name, "forecast") },
          { cls: "dim", html: "no data" },
          { cls: "dim", html: "—" },
          { cls: "dim", html: "—" },
        ],
      };
    }
    return {
      key: name,
      cells: [
        { cls: "mono", html: entityLink(name, "forecast") },
        { cls: "mono", html: fmtNum(f.today_kwh, 1) },
        { cls: "mono", html: fmtNum(f.tomorrow_kwh, 1) },
        { cls: "dim", html: fmtEpoch(f.fetched_at_epoch_ms) },
      ],
    };
  });
  updateKeyedRows(tbody, rows);
}

// --- copy-button handler (delegated, installed once) --------------------

let copyHandlerInstalled = false;
export function installCopyHandler() {
  if (copyHandlerInstalled) return;
  copyHandlerInstalled = true;
  document.addEventListener("click", (ev) => {
    const el = (ev.target as HTMLElement).closest(".copy-btn") as HTMLButtonElement | null;
    if (!el) return;
    const value = el.getAttribute("data-copy") ?? "";
    doCopy(value).then(
      (ok) => flashButton(el, ok ? "copied" : "failed", ok),
    );
  });
}

function flashButton(el: HTMLButtonElement, label: string, good: boolean) {
  const isIcon = el.classList.contains("icon");
  const orig = el.textContent ?? "copy";
  if (!isIcon) el.textContent = label;
  el.classList.toggle("copied", good);
  el.classList.toggle("copy-failed", !good);
  setTimeout(() => {
    if (!isIcon) el.textContent = orig;
    el.classList.remove("copied", "copy-failed");
  }, 900);
}

async function doCopy(value: string): Promise<boolean> {
  const cb = (navigator as Navigator & { clipboard?: Clipboard }).clipboard;
  if (cb && typeof cb.writeText === "function") {
    try { await cb.writeText(value); return true; } catch { /* fall through */ }
  }
  try {
    const ta = document.createElement("textarea");
    ta.value = value;
    ta.setAttribute("readonly", "");
    ta.style.position = "fixed";
    ta.style.top = "-9999px";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(ta);
    return ok;
  } catch {
    return false;
  }
}
