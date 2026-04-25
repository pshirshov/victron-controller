// Dashboard entry point. Wires manager → widget + snapshot renderers +
// knob controls.

import { ConnectionManager, DEFAULT_CONFIG } from "./manager.js";
import {
  installCopyHandler,
  renderActuated,
  renderBookkeeping,
  renderCoreModal,
  renderCoresState,
  renderDecisions,
  renderForecasts,
  renderSensors,
} from "./render.js";
import { renderKnobs } from "./knobs.js";
import { WsWidget } from "./ws-widget.js";
import type { WorldSnapshot } from "./model/victron_controller/dashboard/WorldSnapshot.js";

function wsUrl(): string {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/ws`;
}

let managerRef: ConnectionManager | null = null;
let widgetRef: WsWidget | null = null;
// TASS core inspector: id of the currently-open core, or null when
// closed. On every snapshot we re-render the modal body so the user
// sees live updates.
let openCoreId: string | null = null;
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
  }
  // Hello / Pong / Log are handled inside the connection/widget.
}

function applySnapshot(snap: WorldSnapshot): void {
  const writesBadge = document.getElementById("writes-badge") as HTMLElement;
  if (snap.knobs.writes_enabled) {
    writesBadge.textContent = "WRITES ON";
    writesBadge.className = "badge on";
  } else {
    writesBadge.textContent = "OBSERVER";
    writesBadge.className = "badge off";
  }
  renderSensors(snap);
  renderDecisions(snap);
  renderActuated(snap);
  renderCoresState(snap);
  renderBookkeeping(snap);
  renderForecasts(snap);
  renderKnobs(snap, sendCommand);

  // Live-refresh the TASS core inspector if it's open.
  lastSnapshot = snap;
  if (openCoreId !== null) {
    renderCoreModal(openCoreId, snap);
  }
}

function openCoreInspector(coreId: string): void {
  openCoreId = coreId;
  const modal = document.getElementById("core-modal");
  if (!modal) return;
  modal.removeAttribute("hidden");
  if (lastSnapshot) {
    renderCoreModal(coreId, lastSnapshot);
  }
}

function closeCoreInspector(): void {
  openCoreId = null;
  const modal = document.getElementById("core-modal");
  if (modal) modal.setAttribute("hidden", "");
}

function installCoreInspectorHandlers(): void {
  // Click on a `.core-link` (the rendered core-name anchor) to open.
  document.body.addEventListener("click", (ev) => {
    const target = ev.target as HTMLElement | null;
    if (!target) return;
    const link = target.closest(".core-link") as HTMLElement | null;
    if (link?.dataset.coreId) {
      ev.preventDefault();
      openCoreInspector(link.dataset.coreId);
      return;
    }
    if (target.id === "core-modal-close" || target.classList.contains("modal-backdrop")) {
      closeCoreInspector();
    }
  });
  document.addEventListener("keydown", (ev) => {
    if (ev.key === "Escape" && openCoreId !== null) {
      closeCoreInspector();
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
  installCoreInspectorHandlers();

  manager.start();
}

init();
