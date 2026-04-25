// Dashboard entry point. Wires manager → widget + snapshot renderers +
// knob controls.

import { ConnectionManager, DEFAULT_CONFIG } from "./manager.js";
import {
  installCopyHandler,
  renderActuated,
  renderBookkeeping,
  renderCoresState,
  renderDecisions,
  renderEntityModal,
  renderForecasts,
  renderSensors,
  renderTimers,
  type EntityType,
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
  renderTimers(snap);
  renderBookkeeping(snap);
  renderForecasts(snap);
  renderKnobs(snap, sendCommand);

  // Live-refresh the entity inspector if it's open.
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
  installEntityInspectorHandlers();

  manager.start();
}

init();
