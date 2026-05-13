// Dashboard entry point. Wires manager → widget + snapshot renderers +
// knob controls.

import { ConnectionManager, DEFAULT_CONFIG } from "./manager.js";
import {
  clearSingleKnobEditModal,
  installBookkeepingEditHandler,
  installCopyHandler,
  renderActuated,
  renderBookkeeping,
  renderCoresState,
  renderDecisions,
  renderDiagnostics,
  renderEntityModal,
  renderForecasts,
  renderPinnedRegisters,
  renderSchedule,
  renderSensors,
  renderTimers,
  renderWeatherSocTable,
  renderHeatingCurveTable,
  renderZappiDrainSummary,
  renderZappiDrainChart,
  type EntityType,
} from "./render.js";
import { renderKnobs } from "./knobs.js";
import { renderSocChart } from "./chart.js";
import { WsWidget } from "./ws-widget.js";
import type { WorldSnapshot } from "./model/victron_controller/dashboard/WorldSnapshot.js";

function wsUrl(): string {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/ws`;
}

let managerRef: ConnectionManager | null = null;
let widgetRef: WsWidget | null = null;
// Entity inspector: id + type of the currently-open entity, or null
// when closed. On every snapshot we re-render the modal body so the
// user sees live updates.
let openEntityId: string | null = null;
let openEntityType: EntityType | null = null;
let lastSnapshot: WorldSnapshot | null = null;

function sendCommand(cmd: unknown): void {
  managerRef?.sendCommand(cmd);
}

function onServerMessage(raw: unknown): void {
  if (typeof raw !== "object" || raw === null) return;
  const obj = raw as Record<string, unknown>;
  if ("Snapshot" in obj) {
    const snap = (obj.Snapshot as { body: WorldSnapshot }).body;
    applySnapshot(snap);
  } else if ("Ack" in obj) {
    const ack = (obj.Ack as { body: { accepted: boolean; error_message: string | null } }).body;
    const err = document.getElementById("last-error") as HTMLElement;
    err.textContent = ack.accepted ? "" : `REJECTED: ${ack.error_message ?? "(unknown)"}`;
  } else if ("Hello" in obj) {
    const hello = (obj.Hello as {
      server_version?: string;
      server_git_sha?: string | null;
    });
    handleHello(hello.server_git_sha ?? null);
  }
  // Pong / Log are handled inside the connection/widget.
}

// PR-version-reload: bundle-baked git SHA. `__WEB_GIT_SHA__` is
// substituted by esbuild's `--define`; the declared type below keeps
// the typechecker happy when running `tsc --noEmit`. An empty string
// (no git checkout during build, e.g. Nix sandbox without VCS) means
// "skip the version check" — same fallback as `server_git_sha: None`.
declare const __WEB_GIT_SHA__: string;
const WEB_GIT_SHA: string = typeof __WEB_GIT_SHA__ === "string" ? __WEB_GIT_SHA__ : "";

// `null` = no Hello observed yet. Records the SHA from the very first
// Hello of this page-load; later Hellos (after reconnect) compare
// against it. Comparison runs against the bundle-baked SHA, not the
// first-seen one — that way a reload across a deploy actually picks
// up the new bundle even if the *first* Hello after page-load happens
// to ship the new SHA.
let firstHelloSeen = false;
let reloadingForVersion = false;

function handleHello(serverSha: string | null): void {
  // Strict null/empty check: missing SHA on either side means we
  // can't be sure, so don't reload.
  if (!serverSha || !WEB_GIT_SHA) {
    firstHelloSeen = true;
    return;
  }

  if (!firstHelloSeen) {
    firstHelloSeen = true;
    // First Hello — initial connection. Only reload if the bundle in
    // the browser predates the running server, which is exactly the
    // case we want to handle (stale cached tab opening after a
    // deploy). Use a sessionStorage breadcrumb so we don't loop in
    // the unlikely event the cache returns the same stale bundle.
    if (serverSha !== WEB_GIT_SHA) {
      reloadIfNotAlreadyAttempted(serverSha);
    }
    return;
  }

  // Subsequent Hello — websocket reconnection. A SHA mismatch here
  // strongly indicates an in-place deploy: the operator left a tab
  // open, the shell restarted with a new bundle, and the next
  // reconnect's Hello carries the new SHA. Reload to pick it up.
  if (serverSha !== WEB_GIT_SHA) {
    reloadIfNotAlreadyAttempted(serverSha);
  }
}

function reloadIfNotAlreadyAttempted(serverSha: string): void {
  if (reloadingForVersion) return;
  const breadcrumb = "victron.versionReloadFor";
  let prior: string | null = null;
  try {
    prior = sessionStorage.getItem(breadcrumb);
  } catch {
    // Privacy mode / disabled storage — fall through to reload.
  }
  if (prior === serverSha) {
    // We already reloaded once for this exact server SHA and the
    // bundle is still mismatched. Don't loop — log and stop.
    // eslint-disable-next-line no-console
    console.warn(
      `version-reload: bundle still mismatches server SHA ${serverSha} after a reload; ` +
      `cached bundle may be sticky. Check Cache-Control on /bundle.js.`
    );
    return;
  }
  try {
    sessionStorage.setItem(breadcrumb, serverSha);
  } catch {
    /* ignore */
  }
  reloadingForVersion = true;
  // eslint-disable-next-line no-console
  console.info(
    `version-reload: bundle SHA ${WEB_GIT_SHA} ≠ server SHA ${serverSha}; reloading.`
  );
  location.reload();
}

/// Tier 2: structural equality without allocations. Walks plain
/// JSON-shaped values (primitives, arrays, plain objects). Returns
/// early on the first mismatch — no intermediate string keys built up,
/// so the equal-path costs only memory reads. NaN is treated as
/// equal-to-itself (the JSON parser never produces NaN, but explicit
/// safety doesn't hurt).
function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a === null || b === null) return a === b;
  if (typeof a !== "object") {
    // Number NaN === NaN handling: both NaN ⇒ equal.
    return typeof a === "number" && typeof b === "number"
      && Number.isNaN(a) && Number.isNaN(b);
  }
  if (Array.isArray(a)) {
    if (!Array.isArray(b)) return false;
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (!deepEqual(a[i], b[i])) return false;
    }
    return true;
  }
  if (Array.isArray(b)) return false;
  const ao = a as Record<string, unknown>;
  const bo = b as Record<string, unknown>;
  // Same key set, same value at each key. Length check first so we
  // bail on mismatched sizes without iterating either side.
  const ak = Object.keys(ao);
  if (ak.length !== Object.keys(bo).length) return false;
  for (let i = 0; i < ak.length; i++) {
    const k = ak[i];
    if (!Object.prototype.hasOwnProperty.call(bo, k)) return false;
    if (!deepEqual(ao[k], bo[k])) return false;
  }
  return true;
}

/// Tier 2: previous snapshot used by `applySnapshot` to skip renderers
/// whose slice of the world didn't change. Distinct from `lastSnapshot`,
/// which the entity-inspector path uses; the inspector wants the
/// current snapshot regardless of skip decisions.
let prevSnapshot: WorldSnapshot | null = null;
/// Wall-clock second at which we last rendered a time-dependent
/// renderer. Used to force a re-render once per second so "X s ago"
/// cells stay current even when the underlying snapshot hasn't moved.
let lastRenderSecond = 0;

function applySnapshot(snap: WorldSnapshot): void {
  // Writes badge — cheap; only update on transition.
  const writesNow = snap.knobs.writes_enabled;
  const writesPrev = prevSnapshot?.knobs.writes_enabled;
  if (writesPrev !== writesNow) {
    const writesBadge = document.getElementById("writes-badge") as HTMLElement;
    if (writesNow) {
      writesBadge.textContent = "WRITES ON";
      writesBadge.className = "badge on";
    } else {
      writesBadge.textContent = "OBSERVER";
      writesBadge.className = "badge off";
    }
  }

  const nowSec = Math.floor(Date.now() / 1000);
  const tickedSecond = nowSec !== lastRenderSecond;
  lastRenderSecond = nowSec;

  // Per-slice change detection. Equal-path is allocation-free; only
  // renderers whose inputs actually changed (or whose output is
  // time-dependent and a wall-clock second has passed) re-run. On the
  // first call `prevSnapshot` is null and every renderer fires.
  const prev = prevSnapshot;
  // Sensors table surfaces typed-sensor rows alongside the f64 ones.
  // PR-DESYN-1: timezone / sunrise / sunset moved off the bare
  // `WorldSnapshot` fields onto `typed_sensors`; the deep-equal on
  // `typed_sensors` covers all five typed rows (eddi.mode, zappi,
  // timezone, sunrise, sunset). Sensor rows render "X s ago" so they
  // are time-dependent.
  const sensorsChanged =
    !prev
    || !deepEqual(prev.sensors, snap.sensors)
    || !deepEqual(prev.sensors_meta, snap.sensors_meta)
    || !deepEqual(
      (prev as unknown as { typed_sensors?: unknown }).typed_sensors,
      (snap as unknown as { typed_sensors?: unknown }).typed_sensors,
    );
  if (sensorsChanged || tickedSecond) renderSensors(snap);

  if (!prev || !deepEqual(prev.decisions, snap.decisions)) renderDecisions(snap);
  // Actuated rows show "since X" age — time-dependent.
  if (!prev || !deepEqual(prev.actuated, snap.actuated) || tickedSecond) renderActuated(snap);
  if (!prev || !deepEqual(prev.cores_state, snap.cores_state)) renderCoresState(snap);
  // Timers: last-fire / next-fire ages — time-dependent.
  if (!prev || !deepEqual(prev.timers, snap.timers) || tickedSecond) renderTimers(snap);
  if (!prev || !deepEqual(prev.bookkeeping, snap.bookkeeping)) renderBookkeeping(snap);
  // Forecasts: last-fetch age — time-dependent.
  if (!prev || !deepEqual(prev.forecasts, snap.forecasts) || tickedSecond) renderForecasts(snap);
  // Knobs: structural only. Pure (sendCommand handler unchanged).
  if (!prev || !deepEqual(prev.knobs, snap.knobs)) renderKnobs(snap, sendCommand);
  // PR-WSOC-EDIT-1: editable 6×2 weather-SoC lookup-table widget.
  // Re-render whenever the table OR any of the 6 boundary knobs
  // change so the inline boundary inputs stay current.
  const wsocBoundariesChanged =
    !prev
    || (prev.knobs.weathersoc_low_energy_threshold
        !== snap.knobs.weathersoc_low_energy_threshold)
    || (prev.knobs.weathersoc_ok_energy_threshold
        !== snap.knobs.weathersoc_ok_energy_threshold)
    || (prev.knobs.weathersoc_high_energy_threshold
        !== snap.knobs.weathersoc_high_energy_threshold)
    || (prev.knobs.weathersoc_too_much_energy_threshold
        !== snap.knobs.weathersoc_too_much_energy_threshold)
    || (prev.knobs.weathersoc_very_sunny_threshold
        !== snap.knobs.weathersoc_very_sunny_threshold)
    || (prev.knobs.weathersoc_winter_temperature_threshold
        !== snap.knobs.weathersoc_winter_temperature_threshold);
  // PR-WSOC-ACTIVE-1: also re-render when the active-cell highlight
  // moves (operator edits a boundary, sensor freshness flips, today's
  // fused forecast crosses a bucket boundary).
  const wsocActiveChanged =
    !prev
    || !deepEqual(
      (prev as unknown as { weather_soc_active?: unknown }).weather_soc_active,
      (snap as unknown as { weather_soc_active?: unknown }).weather_soc_active,
    );
  if (
    !prev
    || wsocBoundariesChanged
    || wsocActiveChanged
    || !deepEqual(
      (prev.knobs as unknown as { weather_soc_table?: unknown }).weather_soc_table,
      (snap.knobs as unknown as { weather_soc_table?: unknown }).weather_soc_table,
    )
  ) {
    renderWeatherSocTable(snap, sendCommand);
  }
  // PR-HEATING-CURVE-1: re-render only when the nested heating_curve
  // struct changed. The widget is otherwise content-static.
  if (
    !prev
    || !deepEqual(
      (prev.knobs as unknown as { heating_curve?: unknown }).heating_curve,
      (snap.knobs as unknown as { heating_curve?: unknown }).heating_curve,
    )
  ) {
    renderHeatingCurveTable(snap, sendCommand);
  }
  // Schedule: "in 4h 23m" → time-dependent.
  if (
    !prev
    || !deepEqual(prev.scheduled_actions, snap.scheduled_actions)
    || tickedSecond
  ) renderSchedule(snap);
  // Pinned registers: last-check / last-drift ages — time-dependent.
  if (!prev || !deepEqual(prev.pinned_registers, snap.pinned_registers) || tickedSecond) {
    renderPinnedRegisters(snap);
  }
  // SoC chart: server stamps `now_epoch_ms` per snapshot, so the
  // chart slice changes every tick anyway; deepEqual short-circuits
  // immediately. We simply forward the snapshot.
  if (!prev || !deepEqual(prev.soc_chart, snap.soc_chart)) renderSocChart(snap);
  // Zappi drain section: re-render whenever the state changes or a second
  // has passed (the sparkline's x-axis scrolls in real time).
  if (!prev || !deepEqual(prev.zappi_drain_state, snap.zappi_drain_state) || tickedSecond) {
    renderZappiDrainSummary(snap.zappi_drain_state);
    renderZappiDrainChart(snap.zappi_drain_state);
  }
  // PR-DIAG-1: process + host memory diagnostics. Refreshes on the
  // sampler cadence (60 s) AND every wall-clock second so the
  // "Last sampled" "X s ago" cell stays current. The diff against
  // `prev.diagnostics` skips the per-second renderer when nothing has
  // moved.
  const prevDiag = (prev as unknown as { diagnostics?: unknown })?.diagnostics;
  const snapDiag = (snap as unknown as { diagnostics?: unknown }).diagnostics;
  if (!prev || !deepEqual(prevDiag, snapDiag) || tickedSecond) {
    renderDiagnostics(snap);
  }

  prevSnapshot = snap;
  lastSnapshot = snap;
  if (openEntityId !== null && openEntityType !== null) {
    renderEntityModal(openEntityId, openEntityType, snap);
  }
}

function openEntityInspector(entityId: string, type: EntityType): void {
  openEntityId = entityId;
  openEntityType = type;
  const modal = document.getElementById("entity-modal");
  if (!modal) return;
  modal.removeAttribute("hidden");
  if (lastSnapshot) {
    renderEntityModal(entityId, type, lastSnapshot);
  }
}

function closeEntityInspector(): void {
  openEntityId = null;
  openEntityType = null;
  const modal = document.getElementById("entity-modal");
  if (modal) modal.setAttribute("hidden", "");
  // PR-WSOC-EDIT-2: clear single-knob-edit modal body state (dataset
  // id + innerHTML) so a subsequent open of the same knob rebuilds
  // from scratch. Without this, the `alreadyOpen` short-circuit takes
  // the live-refresh branch and any stale unsaved input value
  // persists across open/close cycles. Helper is a no-op when no
  // single-knob-edit modal was open.
  clearSingleKnobEditModal();
}

const VALID_TYPES: ReadonlySet<EntityType> = new Set([
  "sensor",
  "knob",
  "actuated",
  "bookkeeping",
  "decision",
  "forecast",
  "core",
  "timer",
  // PR-WSOC-EDIT-2: single-knob-edit modal (covers all 56
  // weather-SoC click-targets — 48 cells + 6 boundary kWh + 1
  // winter-temperature header).
  "single-knob-edit",
]);

function installEntityInspectorHandlers(): void {
  document.body.addEventListener("click", (ev) => {
    const target = ev.target as HTMLElement | null;
    if (!target) return;
    const link = target.closest(".entity-link") as HTMLElement | null;
    if (link?.dataset.entityId && link?.dataset.entityType) {
      const t = link.dataset.entityType as EntityType;
      if (VALID_TYPES.has(t)) {
        ev.preventDefault();
        openEntityInspector(link.dataset.entityId, t);
        return;
      }
    }
    if (target.id === "entity-modal-close" || target.classList.contains("modal-backdrop")) {
      closeEntityInspector();
    }
  });
  document.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape" && openEntityId !== null) {
      closeEntityInspector();
    }
  });
}

function init(): void {
  const manager = new ConnectionManager(
    { ...DEFAULT_CONFIG, url: wsUrl() },
    onServerMessage,
    (_stats) => widgetRef?.refresh(),
  );
  managerRef = manager;

  const widget = new WsWidget(manager, document.getElementById("ws-indicator-slot") as HTMLElement);
  widgetRef = widget;

  installCopyHandler();
  installBookkeepingEditHandler(sendCommand);
  installEntityInspectorHandlers();
  installTabSwitcher();

  manager.start();
}

/// Two-tab layout: Control (knobs / decisions / actuated / cores /
/// forecasts) vs Detail (timers / sensors / bookkeeping). Active tab
/// persists in `location.hash` so a refresh / shared link lands on
/// the same view.
function installTabSwitcher(): void {
  const buttons = Array.from(document.querySelectorAll<HTMLButtonElement>(".tabs .tab"));
  const panels = Array.from(document.querySelectorAll<HTMLElement>(".tab-panel"));
  if (buttons.length === 0 || panels.length === 0) return;

  const apply = (target: string) => {
    for (const btn of buttons) {
      btn.setAttribute("aria-selected", btn.dataset.tab === target ? "true" : "false");
    }
    for (const panel of panels) {
      if (panel.dataset.tab === target) panel.removeAttribute("hidden");
      else panel.setAttribute("hidden", "");
    }
  };

  const initial = (location.hash || "").replace(/^#/, "");
  const valid = buttons.some((b) => b.dataset.tab === initial);
  apply(valid ? initial : "control");

  for (const btn of buttons) {
    btn.addEventListener("click", () => {
      const t = btn.dataset.tab ?? "control";
      apply(t);
      // replaceState (not `location.hash =`) avoids growing a history
      // entry per tab click.
      history.replaceState(null, "", `#${t}`);
    });
  }
}

init();
