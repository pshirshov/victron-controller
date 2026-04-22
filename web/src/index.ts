// Dashboard entry point. Wires manager → widget + snapshot renderers +
// knob controls.

import { ConnectionManager, DEFAULT_CONFIG } from "./manager.js";
import {
  renderActuated,
  renderBookkeeping,
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
  (document.getElementById("captured-at") as HTMLElement).textContent = String(snap.captured_at_naive_iso);
  const writesBadge = document.getElementById("writes-badge") as HTMLElement;
  if (snap.knobs.writes_enabled) {
    writesBadge.textContent = "WRITES ON";
    writesBadge.className = "badge on";
  } else {
    writesBadge.textContent = "OBSERVER";
    writesBadge.className = "badge off";
  }
  renderSensors(snap);
  renderActuated(snap);
  renderBookkeeping(snap);
  renderForecasts(snap);
  renderKnobs(snap, sendCommand);
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

  manager.start();
}

init();
