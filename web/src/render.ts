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
import { entityDescriptions } from "./descriptions.js";

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

// Render the canonical entity name with a `title=` description tooltip
// (native browser hover tip). When no description is registered for the
// name, the cell still renders but without a tooltip — matches the
// "missing key → no tooltip" behaviour the registry promises.
export function nameWithTitle(name: string): string {
  const desc = entityDescriptions[name];
  if (desc === undefined) return esc(name);
  return `<span title="${esc(desc)}">${esc(name)}</span>`;
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
    ? '<span class="bool-badge bool-true" title="true">●</span>'
    : '<span class="bool-badge bool-false" title="false">○</span>';
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
      ? `${nameWithTitle(name)} ${copyIcon(mm.identifier)}`
      : nameWithTitle(name);
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
      { cls: "mono", html: nameWithTitle(key) },
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
        { cls: "mono", html: nameWithTitle(name) },
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
          { cls: "mono", html: nameWithTitle(name) },
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
        { cls: "mono", html: nameWithTitle(name) },
        { html: esc(dec.summary as string) },
        { cls: "factors", html: factors },
      ],
    };
  });
  updateKeyedRows(tbody, rows);
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
          { cls: "mono", html: nameWithTitle(name) },
          { cls: "dim", html: "no data" },
          { cls: "dim", html: "—" },
          { cls: "dim", html: "—" },
        ],
      };
    }
    return {
      key: name,
      cells: [
        { cls: "mono", html: nameWithTitle(name) },
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
