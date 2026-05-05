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
import { ZappiDrainBranch } from "./model/victron_controller/dashboard/ZappiDrainBranch.js";
import type { ZappiDrainState } from "./model/victron_controller/dashboard/ZappiDrainState.js";
import type { ZappiDrainSnapshotWire } from "./model/victron_controller/dashboard/ZappiDrainSnapshotWire.js";
import type { ZappiDrainSample } from "./model/victron_controller/dashboard/ZappiDrainSample.js";
import {
  bookkeepingWriters,
  entityDescriptions,
  forecastProviderLabels,
} from "./descriptions.js";
import { displayNameOfTyped } from "./displayNames.js";
import { KNOB_SPEC, type KnobSpec } from "./knobs.js";

// Entity types recognised by the inspector dispatcher.
//
// PR-WSOC-EDIT-2 replaces `"weathersoc-cell"` with `"single-knob-edit"`
// — the only INTERACTIVE entry in this union (every other arm renders
// a read-only modal body). The single-knob-edit modal carries one
// input (number / checkbox) + default hint + revert button +
// Save/Cancel; `entityId` is the dotted KNOB_SPEC key the modal
// targets. Used for the 48 weather-SoC cell knobs, the 6 boundary
// kWh knobs, and the shared winter-temperature knob.
export type EntityType =
  | "sensor"
  | "knob"
  | "actuated"
  | "bookkeeping"
  | "decision"
  | "forecast"
  | "core"
  | "timer"
  | "single-knob-edit";

// --- incremental update primitives ---------------------------------------

export type RowCell = { cls?: string; html: string; colspan?: number };
export type KeyedRow = { key: string; cells: RowCell[]; cls?: string };

/// Per-tbody scratch collections. Reusing them across ticks instead of
/// allocating fresh `Map`/`Set` per call removes ~22 collection allocs
/// per snapshot (one pair × 11 renderers). The WeakMap lets a tbody
/// removed from the DOM be GC'd without leaking its scratch.
type TbodyScratch = {
  existing: Map<string, HTMLTableRowElement>;
  seen: Set<string>;
};
const tbodyScratch: WeakMap<HTMLElement, TbodyScratch> = new WeakMap();

function getScratch(tbody: HTMLElement): TbodyScratch {
  let s = tbodyScratch.get(tbody);
  if (!s) {
    s = { existing: new Map(), seen: new Set() };
    tbodyScratch.set(tbody, s);
  }
  return s;
}

/// Diff-update the children of `tbody` so they match `rows` exactly,
/// by row key. Existing rows are kept in place; only cells whose class
/// or innerHTML has actually changed are written to the DOM. Cells
/// that contain `document.activeElement` are never touched — that's
/// what makes focused inputs (knob values, search highlighting, text
/// selection inside a cell) survive a snapshot tick.
export function updateKeyedRows(tbody: HTMLElement, rows: KeyedRow[]): void {
  const active = document.activeElement;
  const scratch = getScratch(tbody);
  const existing = scratch.existing;
  const seen = scratch.seen;
  existing.clear();
  seen.clear();

  // Index existing rows by key. Iterate by index to skip the
  // Array.from(tbody.children) snapshot allocation.
  const childCount = tbody.children.length;
  for (let i = 0; i < childCount; i++) {
    const tr = tbody.children[i] as HTMLTableRowElement;
    const k = tr.dataset.key;
    if (k !== undefined) existing.set(k, tr);
  }

  for (let idx = 0; idx < rows.length; idx++) {
    const row = rows[idx];
    seen.add(row.key);
    let tr = existing.get(row.key);
    if (!tr) {
      tr = document.createElement("tr");
      tr.dataset.key = row.key;
    }
    const trCls = row.cls ?? "";
    if (tr.className !== trCls) tr.className = trCls;
    while (tr.children.length < row.cells.length) tr.appendChild(document.createElement("td"));
    while (tr.children.length > row.cells.length) tr.removeChild(tr.lastChild!);
    for (let i = 0; i < row.cells.length; i++) {
      const cell = row.cells[i];
      const td = tr.children[i] as HTMLTableCellElement;
      if (active && td.contains(active)) continue;
      const cls = cell.cls ?? "";
      if (td.className !== cls) td.className = cls;
      if (td.innerHTML !== cell.html) td.innerHTML = cell.html;
      const colspan = cell.colspan ?? 1;
      if (colspan === 1) {
        if (td.hasAttribute("colspan")) td.removeAttribute("colspan");
      } else {
        const cur = td.getAttribute("colspan");
        const want = String(colspan);
        if (cur !== want) td.setAttribute("colspan", want);
      }
    }
    // Position row at the correct index without disturbing untouched ones.
    if (tbody.children[idx] !== tr) tbody.insertBefore(tr, tbody.children[idx] ?? null);
  }

  // Drop rows that no longer appear. Iterate backwards so removals
  // don't shift indices we're about to visit.
  for (let i = tbody.children.length - 1; i >= 0; i--) {
    const tr = tbody.children[i] as HTMLTableRowElement;
    const k = tr.dataset.key;
    if (k !== undefined && !seen.has(k)) tr.remove();
  }
}

// PR-tier-3-ueba: small adapter for the renderers that iterate top-level
// snapshot slices. The UEBA decoder produces baboon class instances with
// private backing fields (`_battery_soc`, …) and public getters
// (`battery_soc`); `toJSON()` returns a plain object keyed by the public
// names. We accept both shapes so a future protocol swap doesn't break
// renderers (and unit-test harnesses can keep feeding plain objects).
function toPlain(v: unknown): Record<string, unknown> {
  if (v && typeof v === "object" && typeof (v as { toJSON?: () => unknown }).toJSON === "function") {
    return (v as { toJSON: () => Record<string, unknown> }).toJSON();
  }
  return v as Record<string, unknown>;
}

// --- formatting helpers --------------------------------------------------
//
// Memoization: most formatters get called with stable inputs every tick
// (a sensor at 82.4% formats to "82.4" frame after frame). Caching the
// result keeps the returned string identity stable, which both saves
// allocation AND lets the per-cell `td.innerHTML !== cell.html` check
// short-circuit at the comparison stage.
//
// Caches are bounded by `MEMO_CACHE_MAX` and cleared (not LRU) when
// they grow past that — simpler than an LRU and correct because keys
// are content-addressed.
const MEMO_CACHE_MAX = 4096;

function bumpCache<K, V>(cache: Map<K, V>): void {
  if (cache.size > MEMO_CACHE_MAX) cache.clear();
}

const fmtNumCache = new Map<string, string>();
function fmtNum(v: number | null | undefined, digits = 1): string {
  if (v === null || v === undefined) return "—";
  if (!isFinite(v)) return String(v);
  // Key folds the discriminator + digits + the value's bit pattern so
  // -0 / +0 don't collide and NaN handling stays explicit (already
  // returned above).
  const key = `${digits}:${v}`;
  let s = fmtNumCache.get(key);
  if (s === undefined) {
    s = v.toFixed(digits);
    fmtNumCache.set(key, s);
    bumpCache(fmtNumCache);
  }
  return s;
}

// fmtEpoch / fmtFuture depend on `Date.now()`, so cache keys must
// include a wall-clock bucket. Bucketed at one-second granularity:
// per-second the function returns at most one string for a given `ms`,
// and the cache resets every second to avoid unbounded growth.
let fmtEpochSecond = 0;
const fmtEpochCache = new Map<number, string>();
function fmtEpoch(ms: number): string {
  if (!ms) return "—";
  const nowSec = Math.floor(Date.now() / 1000);
  if (nowSec !== fmtEpochSecond) {
    fmtEpochCache.clear();
    fmtEpochSecond = nowSec;
  }
  let s = fmtEpochCache.get(ms);
  if (s === undefined) {
    const dt = Math.max(0, (Date.now() - ms) / 1000);
    if (dt < 60) s = `${dt.toFixed(0)} s ago`;
    else if (dt < 3600) s = `${(dt / 60).toFixed(0)} min ago`;
    else s = new Date(ms).toLocaleString();
    fmtEpochCache.set(ms, s);
    bumpCache(fmtEpochCache);
  }
  return s;
}

// Future-tense sibling of fmtEpoch — used by the Schedule section.
// Returns "now" / "in 12m" / "in 4h 23m" / "in 2d 3h" depending on how
// far ahead `ms` sits. Past timestamps clamp to "now" (a snapshot
// arriving slightly after a scheduled fire shouldn't say "−2 s").
let fmtFutureSecond = 0;
const fmtFutureCache = new Map<number, string>();
function fmtFuture(ms: number): string {
  if (!ms) return "—";
  const nowSec = Math.floor(Date.now() / 1000);
  if (nowSec !== fmtFutureSecond) {
    fmtFutureCache.clear();
    fmtFutureSecond = nowSec;
  }
  let s = fmtFutureCache.get(ms);
  if (s !== undefined) return s;
  const dtSec = (ms - Date.now()) / 1000;
  if (dtSec <= 30) s = "now";
  else if (dtSec < 60) s = `in ${dtSec.toFixed(0)}s`;
  else {
    const m = Math.round(dtSec / 60);
    if (m < 60) s = `in ${m}m`;
    else {
      const h = Math.floor(m / 60);
      const mm = m % 60;
      if (h < 24) s = mm === 0 ? `in ${h}h` : `in ${h}h ${mm}m`;
      else {
        const d = Math.floor(h / 24);
        const hh = h % 24;
        s = hh === 0 ? `in ${d}d` : `in ${d}d ${hh}h`;
      }
    }
  }
  fmtFutureCache.set(ms, s);
  bumpCache(fmtFutureCache);
  return s;
}

// Wall-clock for the next-fire epoch — used in the Schedule section
// alongside fmtFuture, so the operator sees both "in 4h" and the actual
// HH:MM the action will land.
function fmtClock(ms: number): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
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
// Inputs are stable across ticks (cadence/staleness constants per
// sensor), so memoization gives a near-100% hit rate.
const fmtDurationCache = new Map<number, string>();
function fmtDurationMs(totalMs: number): string {
  if (!isFinite(totalMs) || totalMs <= 0) return "—";
  let s = fmtDurationCache.get(totalMs);
  if (s !== undefined) return s;
  if (totalMs < 1000) s = `${totalMs}ms`;
  else {
    const totalSec = Math.round(totalMs / 1000);
    if (totalSec < 60) s = `${totalSec}s`;
    else {
      const m = Math.floor(totalSec / 60);
      const sec = totalSec % 60;
      if (m < 60) s = sec === 0 ? `${m}m` : `${m}m ${sec}s`;
      else {
        const h = Math.floor(m / 60);
        const mm = m % 60;
        s = mm === 0 ? `${h}h` : `${h}h ${mm}m`;
      }
    }
  }
  fmtDurationCache.set(totalMs, s);
  bumpCache(fmtDurationCache);
  return s;
}

// --- per-sensor formatters -----------------------------------------------

// MPPT operation mode codes per Victron VE.Direct documentation:
//   0 = Off
//   1 = Voltage-or-current-limited  (curtailed by the inverter)
//   2 = MPPT-tracking               (running unconstrained — the normal state)
// Out-of-range codes fall back to String(code) so future firmware drift
// degrades visibly rather than silently mis-rendering.
const MPPT_OP_MODES: Record<number, string> = {
  0: "Off",
  1: "Voltage-or-current-limited",
  2: "MPPT-tracking",
};

export function fmtMpptOperationMode(value: number): string {
  const code = Math.round(value);
  return MPPT_OP_MODES[code] ?? String(code);
}

// Sensor names that use a custom formatter instead of the generic fmtNum.
const MPPT_OP_MODE_NAMES = new Set([
  "mppt_0_operation_mode",
  "mppt_1_operation_mode",
]);

// Dispatch per-sensor formatting. Returns null when no override applies.
export function fmtSensorValue(name: string, value: number): string | null {
  if (MPPT_OP_MODE_NAMES.has(name)) return fmtMpptOperationMode(value);
  return null;
}

// --- table renderers -----------------------------------------------------

export function renderSensors(snap: WorldSnapshot) {
  const tbody = document.querySelector("#sensors-table tbody") as HTMLElement;
  // PR-tier-3-ueba: snapshot is now a baboon class instance with private
  // backing fields and public getters. `toJSON()` returns a plain object
  // keyed by the public field names (`battery_soc`, …) so the existing
  // entry iteration works against either wire format.
  const entries = Object.entries(toPlain(snap.sensors)).sort(([a], [b]) =>
    displayNameOfTyped(a, "sensor").localeCompare(displayNameOfTyped(b, "sensor")),
  );
  const meta = snap.sensors_meta as unknown as Record<
    string,
    { origin: string; identifier: string; cadence_ms: number; staleness_ms: number }
  >;
  const rows: KeyedRow[] = entries.map(([name, a]) => {
    const act = a as ActualF64;
    const v = act.value;
    const valText =
      v == null
        ? "—"
        : (fmtSensorValue(name, v) ?? fmtNum(v, 2));
    const mm = meta[name];
    const origin = mm ? esc(mm.origin) : `<span class="dim">—</span>`;
    const cadence = mm ? fmtDurationMs(mm.cadence_ms) : `<span class="dim">—</span>`;
    const staleness = mm ? fmtDurationMs(mm.staleness_ms) : `<span class="dim">—</span>`;
    const nameCell = mm
      ? `${entityLink(name, "sensor")} ${copyIcon(mm.identifier)}`
      : entityLink(name, "sensor");
    // PR-DESYN-1: same boot-stamp guard as the typed-sensor rows
    // (PR-EDDI-SENSORS-1, PR-TS-META-1). When the f64 sensor has never
    // been observed, `since_epoch_ms` is the fresh-boot stamp — render
    // "—" instead of "X seconds ago".
    const sinceText =
      act.freshness === "Unknown"
        ? "—"
        : fmtEpoch(act.since_epoch_ms as unknown as number);
    return {
      key: name,
      cells: [
        { cls: "mono", html: nameCell },
        { cls: "mono", html: valText },
        {
          cls: `freshness-${act.freshness}`,
          html: `${act.freshness} <span class="dim">(${sinceText})</span>`,
        },
        { cls: "mono", html: cadence },
        { cls: "mono", html: staleness },
        { cls: "mono", html: origin },
      ],
    };
  });

  // PR-EDDI-SENSORS-1 / PR-DESYN-1: typed-sensor wire block. Carries
  // parsed values from non-f64 sources (myenergi Eddi/Zappi, plus
  // string-valued rows for timezone / sunrise / sunset added in
  // PR-DESYN-1) alongside an `opt[str]` raw_json that the entity
  // inspector surfaces in a "Raw response" panel where applicable.
  type TypedSensorStringWire = {
    value: string | null | undefined;
    freshness: string;
    since_epoch_ms: number | bigint;
    cadence_ms: number | bigint;
    staleness_ms: number | bigint;
    origin: string;
    identifier: string;
  };
  const ts = (snap as unknown as {
    typed_sensors?: {
      eddi_mode: {
        value: string | null | undefined;
        freshness: string;
        since_epoch_ms: number | bigint;
        cadence_ms: number | bigint;
        staleness_ms: number | bigint;
        origin: string;
        identifier: string;
      };
      zappi: {
        mode: string | null | undefined;
        status: string | null | undefined;
        plug_state: string | null | undefined;
        freshness: string;
        since_epoch_ms: number | bigint;
        cadence_ms: number | bigint;
        staleness_ms: number | bigint;
        origin: string;
        identifier: string;
      };
      timezone: TypedSensorStringWire;
      sunrise: TypedSensorStringWire;
      sunset: TypedSensorStringWire;
    };
  }).typed_sensors;
  if (ts) {
    const ev = ts.eddi_mode;
    // Existing f64 sensor rows have the same boot-time defect; deferred for scope (PR-EDDI-SENSORS-1).
    const evSinceText = ev.freshness === "Unknown"
      ? "—"
      : fmtEpoch(ev.since_epoch_ms as unknown as number);
    rows.push({
      key: "eddi.mode",
      cells: [
        {
          cls: "mono",
          html: `${entityLink("eddi.mode", "sensor")} ${copyIcon(ev.identifier)}`,
        },
        { cls: "mono", html: ev.value == null ? "—" : esc(ev.value) },
        {
          cls: `freshness-${ev.freshness}`,
          html: `${esc(String(ev.freshness))} <span class="dim">(${evSinceText})</span>`,
        },
        { cls: "mono", html: fmtDurationMs(ev.cadence_ms as unknown as number) },
        { cls: "mono", html: fmtDurationMs(ev.staleness_ms as unknown as number) },
        { cls: "mono", html: esc(ev.origin) },
      ],
    });

    const z = ts.zappi;
    const zParts = [z.mode, z.status, z.plug_state].filter(
      (p): p is string => p != null,
    );
    const zVal = zParts.length === 0 ? "—" : esc(zParts.join(" · "));
    const zSinceText = z.freshness === "Unknown"
      ? "—"
      : fmtEpoch(z.since_epoch_ms as unknown as number);
    rows.push({
      key: "zappi",
      cells: [
        {
          cls: "mono",
          html: `${entityLink("zappi", "sensor")} ${copyIcon(z.identifier)}`,
        },
        { cls: "mono", html: zVal },
        {
          cls: `freshness-${z.freshness}`,
          html: `${esc(String(z.freshness))} <span class="dim">(${zSinceText})</span>`,
        },
        { cls: "mono", html: fmtDurationMs(z.cadence_ms as unknown as number) },
        { cls: "mono", html: fmtDurationMs(z.staleness_ms as unknown as number) },
        { cls: "mono", html: esc(z.origin) },
      ],
    });

    // PR-DESYN-1: timezone / sunrise / sunset previously hand-built
    // here as synthetic rows. They now flow through the typed-sensor
    // wire surface (`TypedSensorString`) so cadence / staleness /
    // origin / identifier come from the convert layer rather than
    // inline literals.
    const stringRow = (
      key: string,
      s: TypedSensorStringWire,
    ): KeyedRow => {
      const sinceText =
        s.freshness === "Unknown"
          ? "—"
          : fmtEpoch(s.since_epoch_ms as unknown as number);
      const nameCell =
        s.identifier === ""
          ? entityLink(key, "sensor")
          : `${entityLink(key, "sensor")} ${copyIcon(s.identifier)}`;
      return {
        key,
        cells: [
          { cls: "mono", html: nameCell },
          { cls: "mono", html: s.value == null ? "—" : esc(s.value) },
          {
            cls: `freshness-${s.freshness}`,
            html: `${esc(String(s.freshness))} <span class="dim">(${sinceText})</span>`,
          },
          { cls: "mono", html: fmtDurationMs(s.cadence_ms as unknown as number) },
          { cls: "mono", html: fmtDurationMs(s.staleness_ms as unknown as number) },
          { cls: "mono", html: esc(s.origin) },
        ],
      };
    };
    rows.push(stringRow("system.timezone", ts.timezone));
    rows.push(stringRow("solar.sunrise", ts.sunrise));
    rows.push(stringRow("solar.sunset", ts.sunset));
  }

  // Re-sort so the synthetic row lands alphabetically alongside the rest.
  rows.sort((a, b) =>
    displayNameOfTyped(a.key, "sensor").localeCompare(displayNameOfTyped(b.key, "sensor")),
  );
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
  // PR-keep-batteries-charged.
  const ess: ActuatedI32 = a.ess_state_target;

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
    mkRow(
      "ess_state_target",
      ess.target_value === null ? "—" : String(ess.target_value),
      String(ess.target_owner),
      String(ess.target_phase),
      ess.actual.value === null ? "—" : String(ess.actual.value),
      String(ess.actual.freshness),
      ess.actual.since_epoch_ms as unknown as number,
    ),
  ];
  rows.sort((a, b) => displayNameOfTyped(a.key, "actuated").localeCompare(displayNameOfTyped(b.key, "actuated")));
  updateKeyedRows(tbody, rows);
}

// Snapshot field names that round-trip through retained MQTT and seed
// at boot — the `BookkeepingKey` variants in `crates/core/src/types.rs`
// (`NextFullCharge`, `AboveSocDate`). Everything else in
// `world.bookkeeping` is recomputed every tick from sensors / knobs /
// forecast and is effectively a derivation surfaced through the
// bookkeeping struct rather than persistent state.
const BK_PERSISTED_FIELDS: ReadonlySet<string> = new Set([
  "next_full_charge_iso",
  "above_soc_date_iso",
]);

function bkPersistenceTag(name: string): string {
  if (BK_PERSISTED_FIELDS.has(name)) {
    return '<span class="bk-tag bk-tag-persisted" title="Persisted to retained MQTT; restored at boot">persisted</span>';
  }
  return '<span class="bk-tag bk-tag-derived" title="Recomputed every tick from world state; not restored at boot">derived</span>';
}

export function renderBookkeeping(snap: WorldSnapshot) {
  const tbody = document.querySelector("#bk-table tbody") as HTMLElement;
  const entries = Object.entries(toPlain(snap.bookkeeping))
    .sort(([a], [b]) => displayNameOfTyped(a, "bookkeeping").localeCompare(displayNameOfTyped(b, "bookkeeping")));
  const rows: KeyedRow[] = entries.map(([name, val]) => {
    let disp: string;
    if (val === null || val === undefined) disp = "—";
    else if (typeof val === "boolean") disp = boolBadge(val);
    else if (typeof val === "number") disp = fmtNum(val, 2);
    else {
      const s = String(val);
      disp = maybeBoolBadge(s) ?? esc(s);
    }
    // PR-bookkeeping-edit: pencil icon for editable bookkeeping fields.
    // Only `next_full_charge` is editable today (allowlisted in
    // `apply_set_bookkeeping`). The click handler swaps the cell into
    // edit mode. The snapshot field name is `next_full_charge_iso`
    // (it carries an ISO 8601 string); the data-edit-bk attribute
    // carries the canonical key the click handler dispatches on.
    if (name === "next_full_charge_iso") {
      const editBtn =
        `<button class="edit-btn icon" data-edit-bk="next_full_charge" title="Edit next full charge">&#9998;</button>`;
      disp = `${disp} ${editBtn}`;
    }
    const nameCell = `${entityLink(name, "bookkeeping")} ${bkPersistenceTag(name)}`;
    return {
      key: name,
      cells: [
        { cls: "mono", html: nameCell },
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
  ordered.sort(([a], [b]) => displayNameOfTyped(a, "decision").localeCompare(displayNameOfTyped(b, "decision")));
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
      .join("");
    return {
      key: name,
      cells: [
        { cls: "mono", html: entityLink(name, "decision") },
        { html: esc(dec.summary as string) },
        // Wrap the pill list in a flex-wrap container so the column's
        // `max-content` width collapses to the longest pill instead of
        // the (un-wrapped) sum of all pills. Without this the table's
        // `min-width: max-content` blows the column up far past the
        // viewport, forcing desktop horizontal scroll and mobile
        // clipping (body has `overflow-x: hidden`).
        { cls: "factors", html: `<div class="factor-list">${factors}</div>` },
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

// PR-DIAG-1: format an integer byte count with the largest unit that
// keeps the number ≥ 1 (mirrors how HA's frontend formats data_size
// sensors). Negative values shouldn't occur in practice but render as
// "—" rather than throwing the table off.
function fmtBytes(n: number | bigint | null | undefined): string {
  if (n === null || n === undefined) return "—";
  const x = typeof n === "bigint" ? Number(n) : n;
  if (!Number.isFinite(x) || x < 0) return "—";
  if (x === 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let u = 0;
  let v = x;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u += 1;
  }
  // Below MiB: integer; MiB+: one decimal so MB-scale leaks are
  // visible (~50 MiB → 51.0 MiB doesn't collapse to "51 MiB").
  const decimals = u <= 1 ? 0 : 1;
  return `${v.toFixed(decimals)} ${units[u]}`;
}

// PR-DIAG-1: format an uptime (seconds → "Xd Xh Xm Xs") trimming
// leading zero-units.
function fmtUptimeSeconds(secs: number | bigint): string {
  const total = typeof secs === "bigint" ? Number(secs) : secs;
  if (!Number.isFinite(total) || total < 0) return "—";
  const s = Math.floor(total);
  const days = Math.floor(s / 86400);
  const hours = Math.floor((s % 86400) / 3600);
  const mins = Math.floor((s % 3600) / 60);
  const secsR = s % 60;
  const parts: string[] = [];
  if (days > 0) parts.push(`${days}d`);
  if (hours > 0 || days > 0) parts.push(`${hours}h`);
  if (mins > 0 || hours > 0 || days > 0) parts.push(`${mins}m`);
  parts.push(`${secsR}s`);
  return parts.join(" ");
}

// PR-DIAG-1: process + host memory diagnostics. Uses a fixed row order
// so the layout is stable across ticks; `updateKeyedRows` only
// rewrites cells whose innerHTML actually changed.
type DiagRow = { key: string; label: string; value: string; hint?: string };
export function renderDiagnostics(snap: WorldSnapshot): void {
  const tbody = document.querySelector("#diagnostics-table tbody") as HTMLElement | null;
  if (!tbody) return;
  const d = (snap as unknown as { diagnostics?: Record<string, number | bigint> }).diagnostics;
  if (!d) {
    updateKeyedRows(tbody, []);
    return;
  }
  const entries: DiagRow[] = [
    { key: "uptime", label: "Process uptime", value: fmtUptimeSeconds(d.process_uptime_s) },
    { key: "rss", label: "Process RSS", value: fmtBytes(d.process_rss_bytes), hint: "Resident memory in use right now" },
    { key: "hwm", label: "Process RSS peak", value: fmtBytes(d.process_vm_hwm_bytes), hint: "High-water mark since process start" },
    { key: "vmsize", label: "Process VmSize", value: fmtBytes(d.process_vm_size_bytes), hint: "Virtual address space" },
    { key: "jeallocated", label: "Heap (jemalloc allocated)", value: fmtBytes(d.jemalloc_allocated_bytes), hint: "Bytes the program holds via Box/Vec/etc." },
    { key: "jeresident", label: "Heap (jemalloc resident)", value: fmtBytes(d.jemalloc_resident_bytes), hint: "Bytes jemalloc has mapped from the OS — divergence vs allocated is fragmentation" },
    { key: "host-total", label: "Host RAM total", value: fmtBytes(d.host_mem_total_bytes) },
    { key: "host-avail", label: "Host RAM available", value: fmtBytes(d.host_mem_available_bytes), hint: "Use this, not MemFree — accounts for reclaimable cache" },
    { key: "host-swap", label: "Host swap used", value: fmtBytes(d.host_swap_used_bytes) },
    {
      key: "sampled",
      label: "Last sampled",
      value: d.sampled_at_epoch_ms
        ? fmtEpoch(typeof d.sampled_at_epoch_ms === "bigint" ? Number(d.sampled_at_epoch_ms) : d.sampled_at_epoch_ms)
        : "—",
    },
  ];
  const rows: KeyedRow[] = entries.map((e) => ({
    key: e.key,
    cells: [
      { html: e.hint ? `${esc(e.label)} <span class="dim" title="${esc(e.hint)}">ⓘ</span>` : esc(e.label) },
      { cls: "mono", html: esc(e.value) },
    ],
  }));
  updateKeyedRows(tbody, rows);
}

export function renderTimers(snap: WorldSnapshot) {
  const tbody = document.querySelector("#timers-table tbody") as HTMLElement;
  if (!tbody) return;
  const t = snap.timers as unknown as { entries?: TimerRow[] } | undefined;
  const entries = (t?.entries ?? [])
    .slice()
    .sort((a, b) => displayNameOfTyped(a.id, "timer").localeCompare(displayNameOfTyped(b.id, "timer")));
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

// PR-schedule-section: render the forward-looking controller-action
// table. Backend already sorts by next_fire_epoch_ms ascending, so the
// frontend renders entries in arrival order — `When` is the column the
// section is sorted on, not `Source`/`Action`.
type ScheduledActionRow = {
  label: string;
  source: string;
  next_fire_epoch_ms: number;
  period_ms: number | null;
};

export function renderSchedule(snap: WorldSnapshot): void {
  const tbody = document.querySelector("#schedule-table tbody") as HTMLElement | null;
  if (!tbody) return;
  const sa = (snap as unknown as { scheduled_actions?: { entries?: ScheduledActionRow[] } }).scheduled_actions;
  const entries = sa?.entries ?? [];
  const rows: KeyedRow[] = entries.map((e, idx) => ({
    // Composite key — multiple eddi.tariff entries (one per edge) need
    // distinct keys so updateKeyedRows doesn't collapse them. Source
    // alone isn't unique.
    key: `${e.source}-${idx}`,
    cells: [
      {
        cls: "mono",
        html:
          `${esc(fmtFuture(e.next_fire_epoch_ms))} ` +
          `<span class="dim">(${esc(fmtClock(e.next_fire_epoch_ms))})</span>`,
      },
      { html: esc(e.source) },
      { html: esc(e.label) },
    ],
  }));
  updateKeyedRows(tbody, rows);
}

// PR-pinned-registers: render the per-register table on the Detail
// tab. Hidden when no `[[dbus_pinned_registers]]` are configured (the
// section's <h2> is left in the DOM but the table renders zero rows).
type PinnedRegisterRow = {
  path: string;
  target_value_str: string;
  current_value_str: string | null | undefined;
  status: string;
  drift_count: number;
  last_drift_iso: string | null | undefined;
  last_check_iso: string | null | undefined;
};

export function renderPinnedRegisters(snap: WorldSnapshot): void {
  const section = document.getElementById("pinned-registers-section");
  const tbody = document.querySelector(
    "#pinned-registers-table tbody",
  ) as HTMLElement | null;
  if (!section || !tbody) return;
  const list =
    ((snap as unknown) as { pinned_registers?: PinnedRegisterRow[] })
      .pinned_registers ?? [];
  if (list.length === 0) {
    section.setAttribute("hidden", "");
    tbody.replaceChildren();
    return;
  }
  section.removeAttribute("hidden");
  // Sort by path ascending (the backend already does this; resort
  // defensively so the dashboard is deterministic regardless of
  // future wire-format reordering).
  const sorted = list.slice().sort((a, b) => a.path.localeCompare(b.path));
  const rows: KeyedRow[] = sorted.map((e) => {
    const statusCls = (() => {
      switch (e.status) {
        case "drifted":
          return "pinned-status pinned-status-drifted";
        case "confirmed":
          return "pinned-status pinned-status-confirmed";
        default:
          return "pinned-status pinned-status-unknown";
      }
    })();
    const current = e.current_value_str ?? "—";
    const lastDrift = e.last_drift_iso ?? "—";
    const lastCheck = e.last_check_iso ?? "—";
    return {
      key: e.path,
      cells: [
        { cls: "mono pinned-path", html: esc(e.path) },
        {
          cls: "mono",
          html: `${esc(e.target_value_str)} <span class="dim">(saw ${esc(current)})</span>`,
        },
        {
          html: `<span class="${statusCls}">${esc(e.status)}</span>`,
        },
        { cls: "mono", html: String(e.drift_count) },
        { cls: "dim", html: esc(lastDrift) },
        { cls: "dim", html: esc(lastCheck) },
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
///
/// PR-WSOC-EDIT-2: the `single-knob-edit` arm is the only INTERACTIVE
/// modal body — its `<input>` element must NOT be clobbered by a
/// snapshot arriving while the user is editing. The renderer
/// preserves any input value that has focus or already differs from
/// the snapshot value (mirrors the `renderKnobs` focus-preservation
/// discipline). All other arms render read-only bodies; idempotent
/// `innerHTML` replacement is fine for them.
export function renderEntityModal(
  entityId: string,
  type: EntityType,
  snap: WorldSnapshot,
) {
  const titleEl = document.getElementById("entity-modal-title");
  const bodyEl = document.getElementById("entity-modal-body");
  if (!titleEl || !bodyEl) return;
  titleEl.textContent = `${type}: ${entityId}`;

  if (type === "single-knob-edit") {
    renderSingleKnobEditModalBody(entityId, snap, bodyEl);
    return;
  }

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

  const sections: string[] = [descriptionSection(entityId, "sensor")];

  // PR-EDDI-SENSORS-1 / PR-DESYN-1: typed-sensor entries live on a
  // sibling wire block, not in `snap.sensors`. The eddi.mode / zappi
  // rows carry an `opt[str]` raw_json that surfaces in a "Raw
  // response" panel; the PR-DESYN-1 string rows (timezone, sunrise,
  // sunset) carry no raw body — their sources are local computations
  // or D-Bus settings, not poll-driven JSON.
  type TypedSensorStringInspectorWire = {
    value: string | null | undefined;
    freshness: string;
    since_epoch_ms: number | bigint;
    cadence_ms: number | bigint;
    staleness_ms: number | bigint;
    origin: string;
    identifier: string;
  };
  const ts = (snap as unknown as {
    typed_sensors?: {
      eddi_mode: {
        value: string | null | undefined;
        freshness: string;
        since_epoch_ms: number | bigint;
        cadence_ms: number | bigint;
        staleness_ms: number | bigint;
        origin: string;
        identifier: string;
        raw_json: string | null | undefined;
      };
      zappi: {
        mode: string | null | undefined;
        status: string | null | undefined;
        plug_state: string | null | undefined;
        freshness: string;
        since_epoch_ms: number | bigint;
        cadence_ms: number | bigint;
        staleness_ms: number | bigint;
        origin: string;
        identifier: string;
        raw_json: string | null | undefined;
      };
      timezone: TypedSensorStringInspectorWire;
      sunrise: TypedSensorStringInspectorWire;
      sunset: TypedSensorStringInspectorWire;
    };
  }).typed_sensors;

  if (entityId === "eddi.mode" && ts) {
    const ev = ts.eddi_mode;
    const valText = ev.value == null ? "—" : esc(ev.value);
    const since = ev.since_epoch_ms as unknown as number;
    // Existing f64 sensor rows have the same boot-time defect; deferred for scope (PR-EDDI-SENSORS-1).
    const ageText = ev.freshness === "Unknown" ? "—" : esc(fmtEpoch(since));
    sections.push(
      `<section><h3>Current value</h3>` +
        `<table><tbody>` +
        `<tr><th>value</th><td>${valText}</td></tr>` +
        `<tr><th>freshness</th><td class="freshness-${esc(String(ev.freshness))}">${esc(String(ev.freshness))}</td></tr>` +
        `<tr><th>age</th><td>${ageText}</td></tr>` +
        `</tbody></table></section>`,
    );
    sections.push(originSection({
      origin: ev.origin,
      identifier: ev.identifier,
      cadence_ms: ev.cadence_ms as unknown as number,
      staleness_ms: ev.staleness_ms as unknown as number,
    }));
    sections.push(rawResponseSection(ev.raw_json));
    return sections.filter(Boolean).join("");
  }
  if (entityId === "zappi" && ts) {
    const z = ts.zappi;
    const since = z.since_epoch_ms as unknown as number;
    const ageText = z.freshness === "Unknown" ? "—" : esc(fmtEpoch(since));
    sections.push(
      `<section><h3>Current value</h3>` +
        `<table><tbody>` +
        `<tr><th>mode</th><td>${z.mode == null ? "—" : esc(z.mode)}</td></tr>` +
        `<tr><th>status</th><td>${z.status == null ? "—" : esc(z.status)}</td></tr>` +
        `<tr><th>plug_state</th><td>${z.plug_state == null ? "—" : esc(z.plug_state)}</td></tr>` +
        `<tr><th>freshness</th><td class="freshness-${esc(String(z.freshness))}">${esc(String(z.freshness))}</td></tr>` +
        `<tr><th>age</th><td>${ageText}</td></tr>` +
        `</tbody></table></section>`,
    );
    sections.push(originSection({
      origin: z.origin,
      identifier: z.identifier,
      cadence_ms: z.cadence_ms as unknown as number,
      staleness_ms: z.staleness_ms as unknown as number,
    }));
    sections.push(rawResponseSection(z.raw_json));
    return sections.filter(Boolean).join("");
  }

  // PR-DESYN-1: string-valued typed sensors (timezone, sunrise,
  // sunset). No "Raw response" panel — values are local computations
  // (sunrise/sunset) or D-Bus settings (timezone), not poll bodies.
  const stringEntityKey = (() => {
    if (entityId === "system.timezone") return "timezone" as const;
    if (entityId === "solar.sunrise") return "sunrise" as const;
    if (entityId === "solar.sunset") return "sunset" as const;
    return null;
  })();
  if (stringEntityKey !== null && ts) {
    const s = ts[stringEntityKey];
    const since = s.since_epoch_ms as unknown as number;
    const ageText = s.freshness === "Unknown" ? "—" : esc(fmtEpoch(since));
    const valText = s.value == null ? "—" : esc(s.value);
    sections.push(
      `<section><h3>Current value</h3>` +
        `<table><tbody>` +
        `<tr><th>value</th><td>${valText}</td></tr>` +
        `<tr><th>freshness</th><td class="freshness-${esc(String(s.freshness))}">${esc(String(s.freshness))}</td></tr>` +
        `<tr><th>age</th><td>${ageText}</td></tr>` +
        `</tbody></table></section>`,
    );
    sections.push(originSection({
      origin: s.origin,
      identifier: s.identifier,
      cadence_ms: s.cadence_ms as unknown as number,
      staleness_ms: s.staleness_ms as unknown as number,
    }));
    return sections.filter(Boolean).join("");
  }

  const a = sensors[entityId];
  const mm = meta[entityId];

  if (!a) {
    sections.push(`<section><p>no sensor "${esc(entityId)}" in snapshot</p></section>`);
    return sections.filter(Boolean).join("");
  }

  const valText = a.value === null ? "—" : fmtNum(a.value, 2);
  const since = a.since_epoch_ms as unknown as number;
  // PR-DESYN-1: same Unknown guard as the typed-sensor inspector
  // branches; matches the table-row fix at the top of `renderSensors`.
  const ageText =
    a.freshness === "Unknown" ? "—" : esc(fmtEpoch(since));
  sections.push(
    `<section><h3>Current value</h3>` +
      `<table><tbody>` +
      `<tr><th>value</th><td>${valText}</td></tr>` +
      `<tr><th>freshness</th><td class="freshness-${esc(String(a.freshness))}">${esc(String(a.freshness))}</td></tr>` +
      `<tr><th>age</th><td>${ageText}</td></tr>` +
      `</tbody></table></section>`,
  );

  if (mm) {
    sections.push(originSection(mm));
  }
  return sections.filter(Boolean).join("");
}

// Shared "Origin" block for sensor inspector popups. Used by both the
// f64 path (sensors_meta) and typed-sensor branches (eddi.mode/zappi).
// Skips the copy icon when identifier is empty (typed sensors with
// no upstream serial fall through `unwrap_or("")` in convert).
function originSection(m: {
  origin: string;
  identifier: string;
  cadence_ms: number;
  staleness_ms: number;
}): string {
  const identCell =
    m.identifier === ""
      ? "—"
      : `${esc(m.identifier)} ${copyIcon(m.identifier)}`;
  return (
    `<section><h3>Origin</h3>` +
    `<table><tbody>` +
    `<tr><th>origin</th><td>${esc(m.origin)}</td></tr>` +
    `<tr><th>identifier</th><td>${identCell}</td></tr>` +
    `<tr><th>cadence</th><td>${esc(fmtDurationMs(m.cadence_ms))}</td></tr>` +
    `<tr><th>stale after</th><td>${esc(fmtDurationMs(m.staleness_ms))}</td></tr>` +
    `</tbody></table></section>`
  );
}

/// PR-EDDI-SENSORS-1: render the "Raw response" panel for typed
/// sensors that captured the upstream JSON body. Returns an empty
/// string when raw_json is absent — silent absence rather than a
/// "no data" placeholder, per the PR brief.
function rawResponseSection(raw: string | null | undefined): string {
  if (raw == null) return "";
  return (
    `<section><details class="raw-response" open>` +
    `<summary>Raw response <button class="copy-btn icon" data-copy-from-sibling="true" title="Copy JSON">⧉</button></summary>` +
    `<pre><code>${esc(raw)}</code></pre>` +
    `</details></section>`
  );
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
    // PR-baseline-forecast: locally-computed pessimistic baseline,
    // used as a fallback when no cloud provider is fresh.
    ["baseline", (snap.forecasts as unknown as { baseline?: any }).baseline],
  ];
  providers.sort(([a], [b]) => displayNameOfTyped(a, "forecast").localeCompare(displayNameOfTyped(b, "forecast")));
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

// --- PR-WSOC-EDIT-2: weather-SoC 9×6 flat editable lookup-table widget --
//
// PR-WSOC-TABLE-1 introduced this widget read-only on the Detail tab.
// PR-WSOC-EDIT-1 promoted it to the Control tab with a 3-column grouped
// layout + inline boundary-input strip + per-cell 4-field modal.
// PR-WSOC-EDIT-2 flattens the layout to 9 columns × 6 rows: each
// per-field cell, each kWh boundary number embedded in a row label,
// and the shared "12 °C" temperature header are independently
// clickable single-knob-edit targets routed through the
// `single-knob-edit` modal arm. The boundary-input strip is gone —
// its function is absorbed into the row-label kWh anchors.

/// PR-WSOC-TABLE-1: minimal per-cell shape used by the renderer. Mirrors
/// the wire model `WeatherSocCell`; kept structural so the test harness
/// can feed plain JS objects without instantiating the codegen class.
export type WeatherSocCellLike = {
  export_soc_threshold: number;
  battery_soc_target: number;
  discharge_soc_target: number;
  extended: boolean;
};
export type WeatherSocTableLike = Record<string, WeatherSocCellLike>;

/// PR-WSOC-EDIT-2: live boundary numbers piped into bucket row labels.
/// Read from `snap.knobs.weathersoc_*` snake names by
/// `renderWeatherSocTable`; KNOB_SPEC defaults are fallback only. Each
/// kWh number in the row label renders as an `entity-link` anchor that
/// opens the single-knob-edit modal for the matching boundary knob.
export type WeatherSocBoundariesLike = {
  low: number;
  ok: number;
  high: number;
  tooMuch: number;
  verySunny: number;
};

/// PR-WSOC-EDIT-2: per-field metadata, one entry per of the 4 cell
/// fields. Drives both the row builder (cell index 1..4 / 5..8) and
/// the dispatched dotted knob_name suffix.
const WEATHER_SOC_CELL_FIELDS: ReadonlyArray<{
  snake: keyof WeatherSocCellLike;
  kebab: "export-soc-threshold" | "battery-soc-target" | "discharge-soc-target" | "extended";
  kind: "float" | "bool";
}> = [
  { snake: "export_soc_threshold", kebab: "export-soc-threshold", kind: "float" },
  { snake: "battery_soc_target", kebab: "battery-soc-target", kind: "float" },
  { snake: "discharge_soc_target", kebab: "discharge-soc-target", kind: "float" },
  { snake: "extended", kebab: "extended", kind: "bool" },
];

/// PR-WSOC-EDIT-2: format a single numeric cell field (`exp`, `bat`,
/// `dis`). Integer if it round-trips cleanly, else 1-decimal. Bool is
/// formatted via the caller (✓ / —).
function fmtCellField(value: number, isBool: false): string;
function fmtCellField(value: boolean, isBool: true): string;
function fmtCellField(value: number | boolean, isBool: boolean): string {
  if (isBool) return value ? "✓" : "—";
  const v = value as number;
  if (Number.isFinite(v) && Math.abs(v - Math.round(v)) < 1e-9) {
    return String(Math.round(v));
  }
  return v.toFixed(1);
}

/// PR-WSOC-EDIT-2: format a boundary kWh number for a row label —
/// integer if clean, else 1-decimal. Same rule as `fmtCellField`,
/// pulled out so the row-label and cell-content code can share it.
function fmtBoundary(v: number): string {
  if (Number.isFinite(v) && Math.abs(v - Math.round(v)) < 1e-9) {
    return String(Math.round(v));
  }
  return v.toFixed(1);
}

/// PR-WSOC-EDIT-2: build the bucket-label HTML (cell[0]) for a row.
/// Each kWh number is wrapped as an `entity-link` anchor with
/// `data-entity-type="single-knob-edit"` pointing at the matching
/// boundary knob. `>` / `≤` glyphs flow through `esc(...)`.
function bucketLabelHtml(
  bucketLabel: string,
  inner: string,
): string {
  return `${bucketLabel} (${inner})`;
}

/// Anchor for a boundary kWh number inside a bucket label.
function boundaryAnchor(dotted: string, value: number): string {
  return `<a class="entity-link mono" data-entity-id="${esc(dotted)}" data-entity-type="single-knob-edit">${esc(fmtBoundary(value))}</a>`;
}

/// Anchor for a per-field cell (cells 1..4 / 5..8) — wraps a single
/// formatted value as a single-knob-edit click target. `active=true`
/// adds the `weather-soc-active` class so the dashboard widget can
/// highlight the cell-group the WeatherSocPlanner is currently driving
/// from (PR-WSOC-ACTIVE-1).
function cellAnchor(dotted: string, formatted: string, active: boolean): string {
  const extra = active ? " weather-soc-active" : "";
  return `<a class="entity-link mono${extra}" data-entity-id="${esc(dotted)}" data-entity-type="single-knob-edit">${esc(formatted)}</a>`;
}

/// PR-WSOC-ACTIVE-1: the (bucket, temp) cell the WeatherSocPlanner is
/// currently active in. Sourced from `snap.weather_soc_active` (which
/// comes from `world.weather_soc_active` cached in core after each
/// successful `evaluate_weather_soc`). `bucket` is kebab-case;
/// `cold=true` matches the cold temperature column.
export interface WeatherSocActiveLike {
  bucket: string;
  cold: boolean;
}

/// PR-WSOC-EDIT-2: pure row-builder, factored out so it can be unit-
/// tested without a DOM. Returns one `KeyedRow` per bucket (6 rows,
/// 9 cells each: bucket label + 4 warm-cell anchors + 4 cold-cell
/// anchors). Each cell 1..8 is its own clickable single-knob-edit
/// target keyed by the dotted KNOB_SPEC id
/// `weathersoc.table.<bucket-kebab>.<warm|cold>.<field-kebab>`.
export function buildWeatherSocTableRows(
  table: WeatherSocTableLike,
  boundaries: WeatherSocBoundariesLike,
  active?: WeatherSocActiveLike | null,
): KeyedRow[] {
  // PR-WSOC-EDIT-2: row-label kWh anchors per the locked design:
  //   VerySunny (>67.5)
  //   Sunny     (45–67.5)
  //   Mid       (30–45)
  //   Low       (15–30)
  //   Dim       (8–15)
  //   VeryDim   (≤8)
  // dotted ids:
  //   .energy.low / .ok / .high / .too-much / .very-sunny
  const aLow = boundaryAnchor("weathersoc.threshold.energy.low", boundaries.low);
  const aOk = boundaryAnchor("weathersoc.threshold.energy.ok", boundaries.ok);
  const aHigh = boundaryAnchor("weathersoc.threshold.energy.high", boundaries.high);
  const aTooMuch = boundaryAnchor("weathersoc.threshold.energy.too-much", boundaries.tooMuch);
  const aVerySunny = boundaryAnchor("weathersoc.threshold.energy.very-sunny", boundaries.verySunny);
  const labels: ReadonlyArray<{ html: string; key: string; kebab: string; warm: string; cold: string }> = [
    { key: "very_sunny", kebab: "very-sunny", warm: "very_sunny_warm", cold: "very_sunny_cold",
      html: bucketLabelHtml("VerySunny", `${esc(">")}${aVerySunny}`) },
    { key: "sunny", kebab: "sunny", warm: "sunny_warm", cold: "sunny_cold",
      html: bucketLabelHtml("Sunny", `${aTooMuch}–${aVerySunny}`) },
    { key: "mid", kebab: "mid", warm: "mid_warm", cold: "mid_cold",
      html: bucketLabelHtml("Mid", `${aHigh}–${aTooMuch}`) },
    { key: "low", kebab: "low", warm: "low_warm", cold: "low_cold",
      html: bucketLabelHtml("Low", `${aOk}–${aHigh}`) },
    { key: "dim", kebab: "dim", warm: "dim_warm", cold: "dim_cold",
      html: bucketLabelHtml("Dim", `${aLow}–${aOk}`) },
    { key: "very_dim", kebab: "very-dim", warm: "very_dim_warm", cold: "very_dim_cold",
      html: bucketLabelHtml("VeryDim", `${esc("≤")}${aLow}`) },
  ];
  return labels.map((b) => {
    const warm = table[b.warm];
    const cold = table[b.cold];
    // PR-WSOC-ACTIVE-1: the planner reports its active classification
    // as { bucket: kebab, cold: bool }. A row is "warm-active" when
    // `active.bucket === b.kebab && !active.cold`; "cold-active" when
    // `active.bucket === b.kebab && active.cold`. Each individual cell
    // 1..4 / 5..8 gets the highlight class only on the matching side.
    const warmActive = !!active && active.bucket === b.kebab && !active.cold;
    const coldActive = !!active && active.bucket === b.kebab && active.cold;
    const fieldCells = (cell: WeatherSocCellLike, temp: "warm" | "cold", isActive: boolean) =>
      WEATHER_SOC_CELL_FIELDS.map((f) => {
        const dotted = `weathersoc.table.${b.kebab}.${temp}.${f.kebab}`;
        const formatted = f.kind === "bool"
          ? fmtCellField(cell[f.snake] as boolean, true)
          : fmtCellField(cell[f.snake] as number, false);
        return { cls: "mono", html: cellAnchor(dotted, formatted, isActive) };
      });
    return {
      key: b.key,
      cells: [
        { cls: "mono", html: b.html },
        ...fieldCells(warm, "warm", warmActive),
        ...fieldCells(cold, "cold", coldActive),
      ],
    };
  });
}

// --- PR-WSOC-EDIT-2: single-knob-edit modal dispatcher ------------------

/// PR-WSOC-EDIT-2: module-level dispatcher reference, set by
/// `renderWeatherSocTable`. The single-knob-edit modal handler (bound
/// once on `bodyEl`) reads it at click-time so a stale closure can't
/// fire after a new `sendCommand` has been seeded. Use sites must
/// null-check; bail when unset.
let singleKnobSendCommand: ((cmd: unknown) => void) | null = null;

/// PR-WSOC-EDIT-2: pull the live current value for a dotted knob name
/// out of the snapshot. Cell knobs ride inside `weather_soc_table`
/// (look up via the `<bucket>_<temp>` snake key); flat boundary /
/// winter-temp knobs live as scalar fields on `snap.knobs[<snake>]`.
/// Returns `undefined` when nothing in the snapshot matches the id.
function currentValueFor(
  dotted: string,
  snap: WorldSnapshot,
): number | boolean | undefined {
  const knobs = toPlain(snap.knobs);
  // Cell-knob form: `weathersoc.table.<bucket>.<warm|cold>.<field>`.
  const cellMatch = dotted.match(/^weathersoc\.table\.([a-z-]+)\.(warm|cold)\.([a-z-]+)$/);
  if (cellMatch) {
    const tableRaw = knobs["weather_soc_table"] as unknown;
    if (!tableRaw || typeof tableRaw !== "object") return undefined;
    const tablePlain = toPlain(tableRaw) as Record<string, unknown>;
    const bucketSnake = cellMatch[1].replace(/-/g, "_");
    const cellKey = `${bucketSnake}_${cellMatch[2]}`;
    const cellRaw = tablePlain[cellKey];
    if (!cellRaw || typeof cellRaw !== "object") return undefined;
    const cell = toPlain(cellRaw) as Record<string, unknown>;
    const fieldKey = cellMatch[3].replace(/-/g, "_");
    const v = cell[fieldKey];
    if (typeof v === "number" || typeof v === "boolean") return v;
    return undefined;
  }
  // Boundary / winter-temp knobs: dotted → snake_case scalar field.
  const snake = dottedToSnake(dotted);
  const v = (knobs as Record<string, unknown>)[snake];
  if (typeof v === "number" || typeof v === "boolean") return v;
  return undefined;
}

/// PR-WSOC-EDIT-2: dotted boundary knob name → snake_case `snap.knobs`
/// key. Limited to the boundary + winter-temperature knobs that can
/// open the single-knob-edit modal alongside the 48 cell knobs.
function dottedToSnake(dotted: string): string {
  switch (dotted) {
    case "weathersoc.threshold.energy.low": return "weathersoc_low_energy_threshold";
    case "weathersoc.threshold.energy.ok": return "weathersoc_ok_energy_threshold";
    case "weathersoc.threshold.energy.high": return "weathersoc_high_energy_threshold";
    case "weathersoc.threshold.energy.too-much": return "weathersoc_too_much_energy_threshold";
    case "weathersoc.threshold.energy.very-sunny": return "weathersoc_very_sunny_threshold";
    case "weathersoc.threshold.winter-temperature": return "weathersoc_winter_temperature_threshold";
    default: return "";
  }
}

/// PR-WSOC-EDIT-2 / D08: per-cell knob description fallback. Cell
/// knobs (`weathersoc.table.<bucket>.<temp>.<field>`) reuse the
/// column-header surrogate description (`weathersoc.table.<field>`).
/// Boundary / winter-temp knobs read their description directly.
function descriptionForCellKnob(dotted: string): string {
  const m = dotted.match(/^weathersoc\.table\.[a-z-]+\.(?:warm|cold)\.(.+)$/);
  if (m) {
    return (entityDescriptions as Record<string, string>)[`weathersoc.table.${m[1]}`] ?? "";
  }
  return (entityDescriptions as Record<string, string>)[dotted] ?? "";
}

/// PR-WSOC-EDIT-2: read the per-knob boundaries off the snapshot.
/// Falls back to `KNOB_SPEC` defaults so a snapshot missing a field
/// still renders something sensible. Used by `renderWeatherSocTable`
/// and exposed for tests via `buildWeatherSocTableRows`.
function readBoundaries(snap: WorldSnapshot): WeatherSocBoundariesLike {
  const knobs = toPlain(snap.knobs);
  const read = (snake: string, dotted: string): number => {
    const v = (knobs as Record<string, unknown>)[snake];
    if (typeof v === "number") return v;
    const spec = KNOB_SPEC[dotted];
    if (spec && spec.kind !== "bool" && spec.kind !== "enum") return spec.default;
    return 0;
  };
  return {
    low: read("weathersoc_low_energy_threshold", "weathersoc.threshold.energy.low"),
    ok: read("weathersoc_ok_energy_threshold", "weathersoc.threshold.energy.ok"),
    high: read("weathersoc_high_energy_threshold", "weathersoc.threshold.energy.high"),
    tooMuch: read("weathersoc_too_much_energy_threshold", "weathersoc.threshold.energy.too-much"),
    verySunny: read("weathersoc_very_sunny_threshold", "weathersoc.threshold.energy.very-sunny"),
  };
}

/// PR-WSOC-EDIT-2: DOM-mutating renderer. Takes `sendCommand` so it
/// can stash the dispatcher for the single-knob-edit modal handler.
/// The flat 9-column layout reads boundaries off `snap.knobs.*` so
/// the row labels track operator edits next snapshot.
export function renderWeatherSocTable(
  snap: WorldSnapshot,
  sendCommand?: (cmd: unknown) => void,
): void {
  if (sendCommand) singleKnobSendCommand = sendCommand;
  const tbody = document.querySelector("#weather-soc-table-table tbody") as HTMLElement | null;
  if (!tbody) return;
  const k = toPlain(snap.knobs);
  const tableRaw = k["weather_soc_table"] as unknown;
  if (!tableRaw || typeof tableRaw !== "object") return;
  const tablePlain = toPlain(tableRaw) as Record<string, unknown>;
  // Each cell is itself a class instance with private getters; flatten
  // through toPlain so the renderer reads `export_soc_threshold` etc.
  const tableFlat: WeatherSocTableLike = {};
  for (const k of Object.keys(tablePlain)) {
    const cell = tablePlain[k];
    if (cell && typeof cell === "object") {
      tableFlat[k] = toPlain(cell) as unknown as WeatherSocCellLike;
    }
  }
  const boundaries = readBoundaries(snap);
  // PR-WSOC-ACTIVE-1: read the planner's active classification off the
  // snapshot. The wire field is `WeatherSocActive { bucket, cold }`;
  // class instances flatten through toPlain. Null when the planner
  // skipped (no fresh forecast / unusable temp) — no group highlighted.
  const activeRaw = (snap as unknown as { weather_soc_active?: unknown }).weather_soc_active;
  let active: WeatherSocActiveLike | null = null;
  if (activeRaw && typeof activeRaw === "object") {
    const flat = toPlain(activeRaw) as Record<string, unknown>;
    if (typeof flat.bucket === "string" && typeof flat.cold === "boolean") {
      active = { bucket: flat.bucket, cold: flat.cold };
    }
  }
  updateKeyedRows(tbody, buildWeatherSocTableRows(tableFlat, boundaries, active));
}

// --- PR-WSOC-EDIT-2: single-knob-edit modal body ------------------------

/// PR-WSOC-EDIT-2: render the single-knob-edit modal body into
/// `bodyEl`. The body contains description prose, one input (number or
/// checkbox), a default hint, a revert button, and Save/Cancel.
/// Subsequent calls from `applySnapshot` only update the input when
/// it is NOT focused and still matches the previously-stamped snapshot
/// value (mirrors the `renderKnobs` focus-preservation discipline so
/// in-progress edits aren't clobbered by snapshot ticks).
function renderSingleKnobEditModalBody(
  dotted: string,
  snap: WorldSnapshot,
  bodyEl: HTMLElement,
): void {
  const spec = KNOB_SPEC[dotted];
  if (!spec) {
    bodyEl.innerHTML = `<section><p>Unknown knob <code>${esc(dotted)}</code>.</p></section>`;
    return;
  }
  const cur = currentValueFor(dotted, snap);
  const desc = descriptionForCellKnob(dotted);

  installSingleKnobEditHandlers(bodyEl);

  // D01: the `singleknobKnob` dataset stamp can survive a non-single-knob
  // re-render (e.g. `renderKnobBody` overwriting `innerHTML` while the
  // dataset entries persist). Require BOTH the dataset match AND the input
  // still being present in the DOM — otherwise fall through to the full
  // rebuild branch so we don't wedge on stale knob-body content.
  const inputStillPresent = !!bodyEl.querySelector("[data-singleknob-field]");
  const alreadyOpen = bodyEl.dataset.singleknobKnob === dotted && inputStillPresent;
  if (!alreadyOpen) {
    bodyEl.dataset.singleknobKnob = dotted;
    const descSection = desc
      ? `<section><p style="color:var(--muted);margin:0">${esc(desc)}</p></section>`
      : "";
    let inputHtml: string;
    let defStr: string;
    if (spec.kind === "bool") {
      defStr = spec.default ? "true" : "false";
      const checked = (cur === undefined ? spec.default : (cur as boolean)) ? " checked" : "";
      inputHtml = `<input type="checkbox" data-singleknob-field${checked}>`;
    } else if (spec.kind === "enum") {
      // single-knob-edit isn't currently aimed at enum knobs (the 56
      // click-targets are float/bool only). Defensive fallback.
      defStr = spec.default;
      const v = cur === undefined ? spec.default : String(cur);
      inputHtml = `<input type="text" data-singleknob-field value="${esc(v)}">`;
    } else {
      // float / int — both ride `SetFloatKnob` per the existing convention.
      defStr = String(spec.default);
      const v = cur === undefined ? spec.default : (cur as number);
      inputHtml = `<input type="number" data-singleknob-field min="${spec.min}" max="${spec.max}" step="${spec.step}" value="${v}">`;
    }
    const escDef = esc(defStr);
    bodyEl.innerHTML =
      `${descSection}` +
      `<section><table class="single-knob-edit-grid"><tbody>` +
        `<tr>` +
          `<td class="mono">${esc(dotted)}</td>` +
          `<td>${inputHtml}</td>` +
          `<td class="dim">[default: ${escDef}]</td>` +
          `<td><button class="copy-btn icon" data-singleknob-revert data-singleknob-default="${escDef}" title="Reset to default (${escDef})">↺</button></td>` +
        `</tr>` +
      `</tbody></table>` +
      `<footer style="margin-top: 8px; display: flex; gap: 8px; justify-content: flex-end;">` +
        `<button id="single-knob-cancel">Cancel</button>` +
        `<button id="single-knob-save">Save</button>` +
      `</footer></section>`;
    stampSingleKnobLastSnap(bodyEl, spec);
    return;
  }
  // Live-refresh branch: update the input only when not focused AND it
  // still matches the previously-stamped snapshot value.
  const input = bodyEl.querySelector("[data-singleknob-field]") as HTMLInputElement | null;
  if (!input) return;
  if (document.activeElement === input) return;
  if (spec.kind === "bool") {
    const live = (cur === undefined ? spec.default : (cur as boolean));
    const last = input.dataset.singleknobLastSnap;
    if (last !== undefined && (input.checked ? "true" : "false") !== last) {
      input.dataset.singleknobLastSnap = String(live);
      return;
    }
    if (input.checked !== live) input.checked = live;
    input.dataset.singleknobLastSnap = String(live);
  } else if (spec.kind === "enum") {
    const live = cur === undefined ? spec.default : String(cur);
    const last = input.dataset.singleknobLastSnap;
    if (last !== undefined && input.value !== last) {
      input.dataset.singleknobLastSnap = live;
      return;
    }
    if (input.value !== live) input.value = live;
    input.dataset.singleknobLastSnap = live;
  } else {
    const live = cur === undefined ? spec.default : (cur as number);
    const last = input.dataset.singleknobLastSnap;
    if (last !== undefined && Number(input.value) !== Number(last)) {
      input.dataset.singleknobLastSnap = String(live);
      return;
    }
    if (Number(input.value) !== live) input.value = String(live);
    input.dataset.singleknobLastSnap = String(live);
  }
}

/// Stamp last-snap baseline on the single input. Pulled out of the
/// install path so a rebuild can re-baseline without re-binding the
/// click handler.
function stampSingleKnobLastSnap(bodyEl: HTMLElement, spec: KnobSpec): void {
  const input = bodyEl.querySelector("[data-singleknob-field]") as HTMLInputElement | null;
  if (!input) return;
  if (spec.kind === "bool") {
    input.dataset.singleknobLastSnap = input.checked ? "true" : "false";
  } else {
    input.dataset.singleknobLastSnap = input.value;
  }
}

/// PR-WSOC-EDIT-2: bind the single-knob-edit click handler ONCE per
/// `bodyEl`. The handler reads `bodyEl.dataset.singleknobKnob` at
/// dispatch time so it always targets the currently-open knob, never
/// a stale closure-captured one. Latched on
/// `bodyEl.dataset.singleknobHandlersInstalled` so a fresh `bodyEl`
/// (HMR / DOM replacement) gets a clean install.
function installSingleKnobEditHandlers(bodyEl: HTMLElement): void {
  if (bodyEl.dataset.singleknobHandlersInstalled === "1") return;
  bodyEl.dataset.singleknobHandlersInstalled = "1";
  bodyEl.addEventListener("click", (ev) => {
    const target = ev.target as HTMLElement | null;
    if (!target) return;
    const dotted = bodyEl.dataset.singleknobKnob;
    if (!dotted) return;
    const revertBtn = target.closest("button[data-singleknob-revert]") as HTMLButtonElement | null;
    if (revertBtn) {
      const def = revertBtn.getAttribute("data-singleknob-default") || "";
      const input = bodyEl.querySelector("[data-singleknob-field]") as HTMLInputElement | null;
      if (!input) return;
      if (input.type === "checkbox") {
        input.checked = def === "true";
      } else {
        input.value = def;
      }
      return;
    }
    if (target.id === "single-knob-cancel") {
      const closeBtn = document.getElementById("entity-modal-close");
      if (closeBtn) (closeBtn as HTMLButtonElement).click();
      return;
    }
    if (target.id === "single-knob-save") {
      saveSingleKnobEdit(bodyEl);
      const closeBtn = document.getElementById("entity-modal-close");
      if (closeBtn) (closeBtn as HTMLButtonElement).click();
      return;
    }
  });
}

/// PR-WSOC-EDIT-2: clear modal state so a subsequent open of the same
/// knob triggers a full rebuild rather than a live-refresh that leaves
/// stale unsaved input values in place. Exported and called from
/// `closeEntityInspector` in `index.ts`. Idempotent.
export function clearSingleKnobEditModal(): void {
  const bodyEl = document.getElementById("entity-modal-body") as HTMLElement | null;
  if (!bodyEl) return;
  if (!bodyEl.dataset.singleknobKnob) return;
  bodyEl.dataset.singleknobKnob = "";
  bodyEl.innerHTML = "";
}

/// PR-WSOC-EDIT-2: dispatch ONE Set{Float,Bool}Knob command IFF the
/// input value differs from the previously-stamped snapshot value.
/// Avoids publishing untouched values back to retained MQTT.
/// `SetFloatKnob` covers `int` knobs too per the existing convention.
function saveSingleKnobEdit(bodyEl: HTMLElement): void {
  const send = singleKnobSendCommand;
  if (!send) return;
  const dotted = bodyEl.dataset.singleknobKnob;
  if (!dotted) return;
  const spec = KNOB_SPEC[dotted];
  if (!spec) return;
  const input = bodyEl.querySelector("[data-singleknob-field]") as HTMLInputElement | null;
  if (!input) return;
  if (spec.kind === "bool") {
    const v = input.checked;
    const last = input.dataset.singleknobLastSnap;
    if (last !== undefined && (last === "true") === v) return;
    send({ SetBoolKnob: { knob_name: dotted, value: v } });
  } else if (spec.kind === "enum") {
    const v = input.value;
    const last = input.dataset.singleknobLastSnap;
    if (last !== undefined && last === v) return;
    // single-knob-edit currently doesn't open enum knobs; defensive
    // path emits the same shape as the renderKnobs enum dispatcher.
    if (spec.cmdVariant === "SetMode") {
      send({ SetMode: { knob_name: dotted, value: v } });
    } else {
      // D02: hard fail rather than dispatch a payload missing
      // `knob_name`. single-knob-edit currently only opens float/bool
      // knobs; the SetMode path above is the sole defensive enum case.
      // A future enum cmdVariant added to single-knob-edit MUST be wired
      // explicitly here, not silently round-tripped through a malformed
      // command.
      throw new Error(
        `single-knob-edit doesn't support cmdVariant ${spec.cmdVariant} for knob ${dotted}`,
      );
    }
  } else {
    const v = parseFloat(input.value);
    if (!isFinite(v)) return;
    const last = input.dataset.singleknobLastSnap;
    if (last !== undefined && Number(last) === v) return;
    send({ SetFloatKnob: { knob_name: dotted, value: v } });
  }
}

// --- copy-button handler (delegated, installed once) --------------------

let copyHandlerInstalled = false;
export function installCopyHandler() {
  if (copyHandlerInstalled) return;
  copyHandlerInstalled = true;
  document.addEventListener("click", (ev) => {
    const el = (ev.target as HTMLElement).closest(".copy-btn") as HTMLButtonElement | null;
    if (!el) return;
    // Multi-line JSON cannot round-trip through a `data-copy` attribute
    // (the first `"` terminates the attribute); the raw-response button
    // opts into reading its sibling <pre><code> textContent instead.
    const value = el.hasAttribute("data-copy-from-sibling")
      ? (el.closest("details")?.querySelector("pre code")?.textContent ?? "")
      : (el.getAttribute("data-copy") ?? "");
    doCopy(value).then(
      (ok) => flashButton(el, ok ? "copied" : "failed", ok),
    );
  });
}

// --- bookkeeping-edit handler (PR-bookkeeping-edit) ---------------------
//
// Delegated click handler on `#bk-table` that turns a pencil-icon click
// into an inline editor for the row's value cell. On Save it calls
// `sendCommand` with a `SetBookkeeping` payload; on Cancel it just
// removes the inline editor and lets the next snapshot tick redraw.
//
// Focus preservation: while the user is editing, `updateKeyedRows`
// won't touch any cell that contains `document.activeElement` — so a
// focused `<input>` survives snapshot ticks unmolested. The Save/Cancel
// buttons live in the same cell, so the cell as a whole is shielded for
// as long as focus is anywhere inside it.

let bkEditHandlerInstalled = false;
let bkEditSend: ((cmd: unknown) => void) | null = null;

export function installBookkeepingEditHandler(
  sendCommand: (cmd: unknown) => void,
): void {
  bkEditSend = sendCommand;
  if (bkEditHandlerInstalled) return;
  bkEditHandlerInstalled = true;
  const tbody = document.querySelector("#bk-table tbody") as HTMLElement | null;
  if (!tbody) return;
  tbody.addEventListener("click", (ev) => {
    const target = ev.target as HTMLElement | null;
    if (!target) return;

    const editBtn = target.closest("button[data-edit-bk]") as HTMLButtonElement | null;
    if (editBtn) {
      enterBookkeepingEdit(editBtn);
      return;
    }
    const saveBtn = target.closest("button[data-save-bk]") as HTMLButtonElement | null;
    if (saveBtn) {
      saveBookkeepingEdit(saveBtn);
      return;
    }
    const cancelBtn = target.closest("button[data-cancel-bk]") as HTMLButtonElement | null;
    if (cancelBtn) {
      cancelBookkeepingEdit(cancelBtn);
      return;
    }
    const clearBtn = target.closest("button[data-clear-bk]") as HTMLButtonElement | null;
    if (clearBtn) {
      clearBookkeepingEdit(clearBtn);
      return;
    }
  });
}

function clearBookkeepingEdit(btn: HTMLButtonElement): void {
  if (!bkEditSend) return;
  const key = btn.getAttribute("data-clear-bk") ?? "";
  const cmdKey = bookkeepingKeyToWire(key);
  if (!cmdKey) return;
  bkEditSend({
    SetBookkeeping: {
      key: cmdKey,
      value: { Cleared: {} },
    },
  });
  const td = btn.closest("td") as HTMLTableCellElement | null;
  const focused = document.activeElement as HTMLElement | null;
  focused?.blur();
  if (td) td.innerHTML = "";
}

function enterBookkeepingEdit(btn: HTMLButtonElement): void {
  const td = btn.closest("td") as HTMLTableCellElement | null;
  if (!td) return;
  // The pencil button carries the *canonical* bookkeeping key
  // (e.g. "next_full_charge"), not the snapshot field name
  // (e.g. "next_full_charge_iso"). All save/cancel/clear data-*
  // attributes mirror the canonical key so `bookkeepingKeyToWire` can
  // map it to the baboon BookkeepingKey variant.
  const key = btn.getAttribute("data-edit-bk") ?? "";
  // The current value lives in the row's text content excluding the
  // button. Use `td.textContent` minus the button text.
  const currentText = (td.firstChild?.textContent ?? "").trim();
  // Convert "YYYY-MM-DD HH:MM:SS" → "YYYY-MM-DDTHH:MM" (datetime-local
  // format). Fall back to empty string if the cell shows "—".
  const inputValue = toDatetimeLocalValue(currentText);
  td.innerHTML =
    `<input type="datetime-local" data-edit-bk-input="${esc(key)}" value="${esc(inputValue)}" />` +
    ` <button data-save-bk="${esc(key)}">Save</button>` +
    ` <button data-cancel-bk="${esc(key)}">Cancel</button>` +
    ` <button data-clear-bk="${esc(key)}" title="Clear (set to none)">&#10005; clear</button>`;
  const input = td.querySelector("input") as HTMLInputElement | null;
  input?.focus();
}

function saveBookkeepingEdit(btn: HTMLButtonElement): void {
  if (!bkEditSend) return;
  const td = btn.closest("td") as HTMLTableCellElement | null;
  if (!td) return;
  const key = btn.getAttribute("data-save-bk") ?? "";
  const input = td.querySelector("input") as HTMLInputElement | null;
  if (!input) return;
  const raw = input.value;
  if (!raw) return;
  // HTML5 datetime-local emits "YYYY-MM-DDTHH:MM" (sometimes with seconds).
  // The Rust decoder accepts both forms.
  const iso = raw;
  const cmdKey = bookkeepingKeyToWire(key);
  if (!cmdKey) return;
  bkEditSend({
    SetBookkeeping: {
      key: cmdKey,
      value: { NaiveDateTime: { iso } },
    },
  });
  // Drop focus so the next snapshot tick is free to repaint the cell
  // with the new value.
  input.blur();
}

function cancelBookkeepingEdit(btn: HTMLButtonElement): void {
  // Just blur — the next snapshot tick (or any non-focused state) will
  // redraw the cell from the current snapshot.
  const td = btn.closest("td") as HTMLTableCellElement | null;
  const focused = document.activeElement as HTMLElement | null;
  focused?.blur();
  // Force-clear the cell so the row repaints on the next snapshot tick.
  if (td) td.innerHTML = "";
}

function toDatetimeLocalValue(s: string): string {
  if (!s || s === "—") return "";
  // chrono Display: "YYYY-MM-DD HH:MM:SS" or "YYYY-MM-DDTHH:MM:SS".
  const m = s.match(/^(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2})(?::\d{2})?$/);
  if (!m) return "";
  return `${m[1]}T${m[2]}`;
}

function bookkeepingKeyToWire(name: string): string | null {
  switch (name) {
    case "next_full_charge":
      return "NextFullCharge";
    case "above_soc_date":
      return "AboveSocDate";
    default:
      return null;
  }
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

// --- PR-ZDO-4: Zappi compensated-drain section ---------------------------
//
// Two exported functions: `renderZappiDrainSummary` (big-number widgets)
// and `renderZappiDrainChart` (30-min sparkline). Both are called from
// `applySnapshot` in `index.ts` whenever `zappi_drain_state` changes.

export const BRANCH_COLOR: Record<ZappiDrainBranch, string> = {
  [ZappiDrainBranch.Tighten]: "#d33",
  [ZappiDrainBranch.Relax]: "#3a3",
  [ZappiDrainBranch.Bypass]: "#888",
  [ZappiDrainBranch.Disabled]: "#555",
};

export const BRANCH_LABEL: Record<ZappiDrainBranch, string> = {
  [ZappiDrainBranch.Tighten]: "Tighten",
  [ZappiDrainBranch.Relax]: "Relax",
  [ZappiDrainBranch.Bypass]: "Bypass",
  [ZappiDrainBranch.Disabled]: "Disabled",
};

export const BRANCH_CSS_CLASS: Record<ZappiDrainBranch, string> = {
  [ZappiDrainBranch.Tighten]: "branch-tighten",
  [ZappiDrainBranch.Relax]: "branch-relax",
  [ZappiDrainBranch.Bypass]: "branch-bypass",
  [ZappiDrainBranch.Disabled]: "branch-disabled",
};

export type ZappiDrainSummaryDisplay = {
  /** "1500 W" or "—" when latest is null/Disabled. */
  compensatedText: string;
  /** "Tighten" / "Relax" / "Bypass" / "Disabled" / "—". */
  branchText: string;
  /** "Engaged" / "Disengaged" / "—". */
  hardClampText: string;
  /** CSS class for the compensated big-number ("big-number" + maybe branch-*). */
  compensatedClass: string;
  /** CSS class for the branch big-number. */
  branchClass: string;
  /** CSS class for the hard-clamp big-number. */
  hardClampClass: string;
};

/**
 * Pure decision logic for the Zappi drain summary widgets. Given a wire
 * snapshot, returns the text and CSS classes the three big-number
 * widgets should display. Pulled out of `renderZappiDrainSummary` so
 * it can be unit-tested without a DOM.
 */
export function summaryFor(latest: ZappiDrainSnapshotWire | undefined): ZappiDrainSummaryDisplay {
  if (!latest) {
    return {
      compensatedText: "—",
      branchText: "—",
      hardClampText: "—",
      compensatedClass: "big-number",
      branchClass: "big-number",
      hardClampClass: "big-number",
    };
  }
  const branchClass = `big-number ${BRANCH_CSS_CLASS[latest.branch]}`;
  // Show "—" for Disabled branch per PR-ZDO-1-D05:
  // compensated_drain_w=0.0 is a meaningless placeholder.
  const compensatedText = latest.branch === ZappiDrainBranch.Disabled
    ? "—"
    : `${Math.round(latest.compensated_drain_w)} W`;
  const branchText = BRANCH_LABEL[latest.branch];
  const hardClampText = latest.hard_clamp_engaged ? "Engaged" : "Disengaged";
  const hardClampClass = latest.hard_clamp_engaged
    ? "big-number hard-clamp-engaged"
    : "big-number hard-clamp-disengaged";
  return {
    compensatedText,
    branchText,
    hardClampText,
    compensatedClass: branchClass,
    branchClass,
    hardClampClass,
  };
}

export function renderZappiDrainSummary(state: ZappiDrainState): void {
  const compEl = document.querySelector<HTMLDivElement>("#zd-compensated-w");
  const branchEl = document.querySelector<HTMLDivElement>("#zd-branch");
  const hcEl = document.querySelector<HTMLDivElement>("#zd-hard-clamp");
  if (!compEl || !branchEl || !hcEl) return;

  const display = summaryFor(state.latest as ZappiDrainSnapshotWire | undefined);

  const setBigNumber = (el: HTMLDivElement, text: string, cls: string) => {
    const v = el.querySelector<HTMLDivElement>(".big-number-value");
    if (v) v.textContent = text;
    el.className = cls;
  };

  setBigNumber(compEl, display.compensatedText, display.compensatedClass);
  setBigNumber(branchEl, display.branchText, display.branchClass);
  setBigNumber(hcEl, display.hardClampText, display.hardClampClass);
}

// SVG layout constants for the zappi-drain sparkline. Mirrors chart.ts.
const ZD_VB_W_FALLBACK = 800;
const ZD_VB_H = 160;
const ZD_PAD_L = 48;
const ZD_PAD_R = 8;
const ZD_PAD_T = 8;
const ZD_PAD_B = 22;
const ZD_PLOT_H = ZD_VB_H - ZD_PAD_T - ZD_PAD_B;
const ZD_WINDOW_MS = 30 * 60 * 1000; // 30 minutes

export function renderZappiDrainChart(state: ZappiDrainState): void {
  const container = document.querySelector<HTMLDivElement>("#zappi-drain-chart");
  if (!container) return;

  const latest = state.latest as ZappiDrainSnapshotWire | undefined;

  // Sort samples by timestamp; GX clock can jump backwards (PR-ZDO-1 risk note).
  const samples = (state.samples as ZappiDrainSample[])
    .slice()
    .sort((a, b) => Number(a.captured_at_epoch_ms - b.captured_at_epoch_ms));

  // Y-axis max. Skip Disabled samples from the max calculation — their
  // compensated_drain_w=0.0 is a placeholder and would yank the scale to 0.
  const nonDisabled = samples.filter((s) => s.branch !== ZappiDrainBranch.Disabled);
  const sampleMax = nonDisabled.length > 0
    ? Math.max(...nonDisabled.map((s) => s.compensated_drain_w))
    : 0;
  const hardClampW = latest ? latest.hard_clamp_w : 0;
  const yMax = Math.max(sampleMax, hardClampW * 1.5, 100);

  const nowMs = Date.now();
  const x0 = nowMs - ZD_WINDOW_MS;
  const x1 = nowMs;

  const vbW = Math.max(320, Math.round(container.clientWidth || ZD_VB_W_FALLBACK));
  const plotW = vbW - ZD_PAD_L - ZD_PAD_R;

  function xToSvg(epochMs: number): number {
    const t = (epochMs - x0) / (x1 - x0);
    return ZD_PAD_L + Math.max(0, Math.min(1, t)) * plotW;
  }

  function yToSvg(w: number): number {
    const t = Math.max(0, Math.min(yMax, w)) / yMax;
    return ZD_PAD_T + (1 - t) * ZD_PLOT_H;
  }

  const parts: string[] = [];
  parts.push(
    `<svg viewBox="0 0 ${vbW} ${ZD_VB_H}" preserveAspectRatio="xMidYMid meet" role="img" aria-label="Zappi compensated drain sparkline">`,
  );

  // Plot background hit area.
  parts.push(
    `<rect x="${ZD_PAD_L}" y="${ZD_PAD_T}" width="${plotW}" height="${ZD_PLOT_H}" fill="transparent" />`,
  );

  // Horizontal gridlines — 0, 25%, 50%, 75%, 100% of yMax.
  const yGridFractions = [0, 0.25, 0.5, 0.75, 1.0];
  for (const frac of yGridFractions) {
    const wVal = yMax * frac;
    const yPx = yToSvg(wVal);
    parts.push(
      `<line class="zd-axis-grid" x1="${ZD_PAD_L}" y1="${yPx.toFixed(1)}" x2="${ZD_PAD_L + plotW}" y2="${yPx.toFixed(1)}" />`,
    );
    if (frac === 0 || frac === 1.0) {
      const label = `${Math.round(wVal)} W`;
      const anchor = "end";
      parts.push(
        `<text class="zd-axis-label" x="${ZD_PAD_L - 4}" y="${(yPx + 3).toFixed(1)}" text-anchor="${anchor}">${label}</text>`,
      );
    }
  }

  // X-axis labels: "-30 min" at left, "0" at right.
  parts.push(
    `<text class="zd-axis-label" x="${ZD_PAD_L}" y="${ZD_PAD_T + ZD_PLOT_H + 14}" text-anchor="start">-30 min</text>`,
  );
  parts.push(
    `<text class="zd-axis-label" x="${ZD_PAD_L + plotW}" y="${ZD_PAD_T + ZD_PLOT_H + 14}" text-anchor="end">0</text>`,
  );
  // Mid-point tick at -15 min.
  const xMid = xToSvg(x0 + ZD_WINDOW_MS / 2);
  parts.push(
    `<line class="zd-axis-grid" x1="${xMid.toFixed(1)}" y1="${ZD_PAD_T + ZD_PLOT_H}" x2="${xMid.toFixed(1)}" y2="${ZD_PAD_T + ZD_PLOT_H + 4}" />`,
  );
  parts.push(
    `<text class="zd-axis-label" x="${xMid.toFixed(1)}" y="${ZD_PAD_T + ZD_PLOT_H + 14}" text-anchor="middle">-15 min</text>`,
  );

  // Dashed reference lines (drawn before polyline so polyline sits on top).
  if (latest) {
    const yThresh = yToSvg(latest.threshold_w);
    parts.push(
      `<line class="zd-threshold-line" x1="${ZD_PAD_L}" y1="${yThresh.toFixed(1)}" x2="${ZD_PAD_L + plotW}" y2="${yThresh.toFixed(1)}" />`,
    );
    parts.push(
      `<text class="zd-axis-label" x="${ZD_PAD_L + plotW - 2}" y="${(yThresh - 3).toFixed(1)}" text-anchor="end" style="fill:#d97706">threshold</text>`,
    );

    const yHardClamp = yToSvg(latest.hard_clamp_w);
    parts.push(
      `<line class="zd-hard-clamp-line" x1="${ZD_PAD_L}" y1="${yHardClamp.toFixed(1)}" x2="${ZD_PAD_L + plotW}" y2="${yHardClamp.toFixed(1)}" />`,
    );
    parts.push(
      `<text class="zd-axis-label" x="${ZD_PAD_L + plotW - 2}" y="${(yHardClamp - 3).toFixed(1)}" text-anchor="end" style="fill:#dc2626">hard clamp</text>`,
    );
  }

  // Polyline: one coloured <line> per adjacent pair of samples. The
  // segment colour is that of the *later* sample's branch (PR-ZDO-4
  // locked convention). Segments where either endpoint is Disabled are
  // rendered in neutral grey at half opacity.
  if (samples.length >= 2) {
    for (let i = 0; i < samples.length - 1; i++) {
      const sa = samples[i];
      const sb = samples[i + 1];
      const xa = xToSvg(Number(sa.captured_at_epoch_ms));
      const ya = yToSvg(sa.branch === ZappiDrainBranch.Disabled ? 0 : sa.compensated_drain_w);
      const xb = xToSvg(Number(sb.captured_at_epoch_ms));
      const yb = yToSvg(sb.branch === ZappiDrainBranch.Disabled ? 0 : sb.compensated_drain_w);
      const isDisabled =
        sa.branch === ZappiDrainBranch.Disabled || sb.branch === ZappiDrainBranch.Disabled;
      const color = isDisabled ? "#555" : BRANCH_COLOR[sb.branch];
      const opacity = isDisabled ? "0.35" : "1";
      parts.push(
        `<line x1="${xa.toFixed(2)}" y1="${ya.toFixed(2)}" x2="${xb.toFixed(2)}" y2="${yb.toFixed(2)}" stroke="${color}" stroke-width="2" stroke-linecap="round" opacity="${opacity}" />`,
      );
    }
  } else if (samples.length === 0 && !latest) {
    // Empty state: show placeholder text.
    parts.push(
      `<text class="zd-axis-label" x="${ZD_PAD_L + plotW / 2}" y="${ZD_PAD_T + ZD_PLOT_H / 2}" text-anchor="middle">no data yet</text>`,
    );
  }

  parts.push(`</svg>`);
  container.innerHTML = parts.join("");
}
