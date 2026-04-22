// Dashboard entry point. Wires a resilient WebSocket (ConnectionManager)
// to the render/knobs layers. Visible connection indicator + RTT in
// the footer.

import { ConnectionManager, ManagerStats } from "./manager.js";
import { renderActuated, renderBookkeeping, renderForecasts, renderSensors } from "./render.js";
import { renderKnobs } from "./knobs.js";
import type { WorldSnapshot } from "./model/victron_controller/dashboard/WorldSnapshot.js";

const DEFAULT_CONFIG = {
  pingIntervalMs: 5_000,
  pongTimeoutMs: 3_000,
  connectTimeoutMs: 10_000,
  staleGracePeriodMs: 15_000,
};

function wsUrl(): string {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/ws`;
}

let managerRef: ConnectionManager | null = null;

function sendCommand(cmd: unknown) {
  if (!managerRef) return;
  const ok = managerRef.send(JSON.stringify({ SendCommand: { body: cmd } }));
  setConnectionIndicator(null, ok ? "command sent" : "no connection; command dropped");
}

function onServerMessage(raw: unknown) {
  if (typeof raw !== "object" || raw === null) return;
  const obj = raw as Record<string, unknown>;
  if ("Hello" in obj) {
    const body = obj.Hello as { server_version: string; server_ts_ms: number };
    console.info("server hello", body);
  } else if ("Pong" in obj) {
    const body = (obj.Pong as { body: { nonce: string; client_ts_ms: number; server_ts_ms: number } }).body;
    managerRef?.deliverPong(body);
  } else if ("Snapshot" in obj) {
    const snap = (obj.Snapshot as { body: WorldSnapshot }).body;
    applySnapshot(snap);
  } else if ("Log" in obj) {
    appendLog(obj.Log as { body: { at_epoch_ms: number; level: string; source: string; message: string } });
  } else if ("Ack" in obj) {
    const ack = (obj.Ack as { body: { accepted: boolean; error_message: string | null } }).body;
    setConnectionIndicator(null, ack.accepted ? "command accepted" : `REJECTED: ${ack.error_message}`);
  }
}

function applySnapshot(snap: WorldSnapshot) {
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

function appendLog(_line: { body: { at_epoch_ms: number; level: string; source: string; message: string } }) {
  // Placeholder — Log messages will be rendered in a separate panel
  // once the server starts forwarding tracing events over the WS. For
  // now we just log to the browser console.
  console.debug("[log]", _line.body.level, _line.body.source, _line.body.message);
}

function setConnectionIndicator(stats: ManagerStats | null, note?: string) {
  const el = document.getElementById("conn-indicator") as HTMLElement;
  const noteEl = document.getElementById("last-error") as HTMLElement;
  if (note) noteEl.textContent = note;
  if (stats === null) {
    // just a transient note; don't reset the state display.
    return;
  }
  const active = stats.active;
  if (active) {
    el.innerHTML = `<span class="conn-ALIVE">ALIVE</span> <span class="dim">rtt ${active.rttLast}ms (avg ${active.avgRtt()}ms) | pings ${active.pings} pongs ${active.pongs} loss ${active.lossPct()}%</span>`;
  } else {
    const nonDead = stats.all.filter((c) => c.state !== "DEAD");
    const stateLabels = nonDead.map((c) => `<span class="conn-${c.state}">${c.state}</span>`).join(" ");
    const nextIn = stats.nextAttemptAt ? Math.max(0, stats.nextAttemptAt - Date.now()) : null;
    const reconnectNote =
      nextIn !== null ? ` retry in ${(nextIn / 1000).toFixed(1)}s` : "";
    el.innerHTML = `${stateLabels || '<span class="conn-DEAD">DISCONNECTED</span>'}${reconnectNote}`;
  }
}

function init() {
  const manager = new ConnectionManager(
    { url: wsUrl(), ...DEFAULT_CONFIG },
    { onServerMessage },
  );
  managerRef = manager;
  manager.addListener((stats) => setConnectionIndicator(stats));
  manager.start();

  // Keep the stale/DEAD display fresh (needed for "retry in Ns" countdown).
  setInterval(() => setConnectionIndicator(manager.stats()), 500);
}

init();
