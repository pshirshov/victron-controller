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

  // PR-tz-from-victron: synthetic row for the Victron-supplied display
  // timezone. The wire field is a plain string (not an Actual<f64>), so
  // it lives outside `snap.sensors`/`snap.sensors_meta` and is built
  // here. Freshness is "Fresh" if the dashboard's snapshot was captured
  // — `captured_at_epoch_ms` is the live-now stamp, and the controller
  // updates `timezone` every D-Bus reseed (60 s on the settings
  // service); we don't have a per-row timestamp, so we always show
  // Fresh and pin staleness/cadence at static "informational" values.
  const tz = (snap as unknown as { timezone?: string }).timezone ?? "Etc/UTC";
  const tzKey = "system.timezone";
  rows.push({
    key: tzKey,
    cells: [
      { cls: "mono", html: entityLink(tzKey, "sensor") },
      { cls: "mono", html: esc(tz) },
      { cls: "freshness-Fresh", html: "Fresh" },
      { cls: "mono", html: fmtDurationMs(60_000) },
      { cls: "mono", html: fmtDurationMs(120_000) },
      { cls: "mono", html: "D-Bus settings" },
    ],
  });

  // PR-baseline-forecast: synthetic rows for today's sunrise/sunset.
  // Wire fields are `opt[str]` — `null` means "Stale or never observed",
  // rendered as an em-dash row with Stale class. The freshness window
  // (3 h, see core::world::SUNRISE_SUNSET_FRESHNESS) is enforced
  // server-side; the client just reflects what it received.
  const sunriseStr = (snap as unknown as { sunrise_local_iso?: string | null })
    .sunrise_local_iso ?? null;
  const sunsetStr = (snap as unknown as { sunset_local_iso?: string | null })
    .sunset_local_iso ?? null;
  for (const [key, value] of [
    ["solar.sunrise", sunriseStr],
    ["solar.sunset", sunsetStr],
  ] as Array<[string, string | null]>) {
    const fresh = value !== null;
    rows.push({
      key,
      cells: [
        { cls: "mono", html: entityLink(key, "sensor") },
        { cls: "mono", html: fresh ? esc(value as string) : "—" },
        {
          cls: fresh ? "freshness-Fresh" : "freshness-Stale",
          html: fresh ? "Fresh" : "Stale",
        },
        { cls: "mono", html: fmtDurationMs(60 * 60 * 1000) },
        { cls: "mono", html: fmtDurationMs(3 * 60 * 60 * 1000) },
        { cls: "mono", html: "baseline forecast" },
      ],
    });
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
