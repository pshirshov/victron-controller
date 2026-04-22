// Resilient-WS connection widget. Renders a 32×32 SVG with:
//   - colored dot (state channel)
//   - ring (countdown channel) that depletes during timed waits
//   - hover tooltip with pool / RTT windows / loss / backoff / close
//   - document.title reflection
//   - bounded event log
//   - rAF-throttled render (~10 Hz) + event-driven immediate refresh
//
// Adapted from pshirshov/ws-reconnect-demo/src/client/ui.ts.

import { ConnectionState, ConnectionStats, RttSample } from "./connection.js";
import { ConnectionManager, LogEntry, ManagerStats } from "./manager.js";

export type WidgetState =
  | "alive"
  | "stale"
  | "connecting"
  | "dead"
  | "terminal"
  | "frozen";

const RING_RADIUS = 13;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

export class WsWidget {
  private manager: ConnectionManager;
  private indicator: HTMLElement;
  private arc: SVGCircleElement;
  private tooltip: HTMLElement;
  private logList: HTMLElement;
  private rafScheduled = false;

  constructor(manager: ConnectionManager, root: HTMLElement) {
    this.manager = manager;
    root.innerHTML = markup();
    this.indicator = root.querySelector(".ws-indicator") as HTMLElement;
    this.arc = this.indicator.querySelector(".ws-indicator-arc") as SVGCircleElement;
    this.tooltip = this.indicator.querySelector(".ws-indicator-tooltip") as HTMLElement;
    this.logList = root.querySelector(".ws-widget-log") as HTMLElement;
    // Retry button (visible only in terminal state) wired once.
    const retry = root.querySelector(".ws-retry") as HTMLButtonElement;
    retry.addEventListener("click", () => this.manager.retryNow());

    this.arc.setAttribute("stroke-dasharray", String(RING_CIRCUMFERENCE));

    // Kick the rAF loop.
    this.scheduleRaf();
  }

  /// Event-driven refresh (called from the manager's onUpdate).
  refresh(): void { this.render(); }

  private scheduleRaf(): void {
    if (this.rafScheduled) return;
    this.rafScheduled = true;
    requestAnimationFrame(() => {
      this.rafScheduled = false;
      this.render();
      setTimeout(() => this.scheduleRaf(), 100); // throttle to ~10 Hz
    });
  }

  private render(): void {
    const stats = this.manager.getStats();
    const widgetState = deriveWidgetState(stats);
    this.indicator.setAttribute("data-state", widgetState);
    this.indicator.setAttribute(
      "aria-label",
      `Connection: ${widgetState}${stats.isTerminal ? " (stopped)" : ""}`,
    );

    // Countdown ring.
    const remaining = computeRingRemaining(widgetState, pickActive(stats), stats, this.manager);
    if (remaining === null) {
      this.arc.style.opacity = "0";
    } else {
      this.arc.style.opacity = "1";
      const dashOffset = RING_CIRCUMFERENCE * (1 - remaining);
      this.arc.style.strokeDashoffset = String(dashOffset);
    }

    // Tooltip.
    this.tooltip.innerHTML = renderTooltipHtml(stats, widgetState);

    // Retry button visibility.
    const retry = this.indicator.querySelector(".ws-retry") as HTMLElement;
    retry.style.display = stats.isTerminal ? "inline-block" : "none";

    // Event log.
    this.logList.innerHTML = renderLog(this.manager.getLog());

    // document.title mirror (V7).
    const alive = stats.connections.filter((c) => c.state === ConnectionState.ALIVE).length;
    const total = stats.connections.filter((c) => c.state !== ConnectionState.DEAD).length;
    const suffix = stats.isTerminal ? " [STOPPED]" : stats.frozen ? " [FROZEN]" : "";
    document.title = `(${alive}/${total}) victron-controller${suffix}`;
  }
}

// --- markup + derive helpers ---

function markup(): string {
  return `
    <div id="ws-indicator" class="ws-indicator" data-state="dead" tabindex="0"
         aria-label="Connection status">
      <svg class="ws-indicator-svg" viewBox="0 0 32 32" width="32" height="32" aria-hidden="true">
        <circle class="ws-indicator-arc" cx="16" cy="16" r="${RING_RADIUS}"
                fill="none" stroke-width="2"/>
      </svg>
      <div class="ws-indicator-dot"></div>
      <button class="ws-retry" title="Retry now">retry</button>
      <div class="ws-indicator-tooltip" role="tooltip"></div>
    </div>
    <ul class="ws-widget-log"></ul>
  `;
}

function pickActive(stats: ManagerStats): ConnectionStats | null {
  return stats.connections.find((c) => c.id === stats.activeConnectionId) ?? null;
}

function deriveWidgetState(stats: ManagerStats): WidgetState {
  if (stats.frozen) return "frozen";
  const seen = new Set(stats.connections.map((c) => c.state));
  if (seen.has(ConnectionState.ALIVE)) return "alive";
  if (seen.has(ConnectionState.STALE)) return "stale";
  if (seen.has(ConnectionState.NEW)) return "connecting";
  if (stats.isTerminal) return "terminal";
  if (stats.reconnectScheduledAt !== null || stats.reconnectDeferredUntilVisible) return "connecting";
  return "dead";
}

function clamp01(n: number): number { return n < 0 ? 0 : n > 1 ? 1 : n; }

function computeRingRemaining(
  widgetState: WidgetState,
  active: ConnectionStats | null,
  stats: ManagerStats,
  manager: ConnectionManager,
): number | null {
  const cfg = (manager as unknown as { config: { pongTimeoutMs: number; staleGracePeriodMs: number; connectTimeoutMs: number } }).config;
  if (widgetState === "alive") {
    if (active === null || active.earliestPendingPingSentAt === null) return 1;
    const elapsed = Date.now() - active.earliestPendingPingSentAt;
    return clamp01(1 - elapsed / cfg.pongTimeoutMs);
  }
  if (widgetState === "stale") {
    if (active === null || active.staleAt === null) return 1;
    const elapsed = Date.now() - active.staleAt;
    return clamp01(1 - elapsed / cfg.staleGracePeriodMs);
  }
  if (widgetState === "connecting") {
    if (stats.reconnectScheduledAt !== null && stats.reconnectDelayMs !== null && stats.reconnectDelayMs > 0) {
      const remaining = stats.reconnectScheduledAt - Date.now();
      return clamp01(remaining / stats.reconnectDelayMs);
    }
    const newest = stats.connections.find((c) => c.state === ConnectionState.NEW);
    if (newest) {
      const elapsed = Date.now() - newest.createdAt;
      return clamp01(1 - elapsed / cfg.connectTimeoutMs);
    }
    return null;
  }
  return null; // dead / terminal / frozen: ring hidden (terminal uses its own CSS)
}

// --- tooltip ---

function renderTooltipHtml(stats: ManagerStats, widgetState: WidgetState): string {
  const active = pickActive(stats);
  const parts: string[] = [];

  parts.push(`<div class="ws-tt-title">${widgetState.toUpperCase()}</div>`);

  const alive = stats.connections.filter((c) => c.state === ConnectionState.ALIVE).length;
  const stale = stats.connections.filter((c) => c.state === ConnectionState.STALE).length;
  const newcount = stats.connections.filter((c) => c.state === ConnectionState.NEW).length;
  const dead = stats.connections.filter((c) => c.state === ConnectionState.DEAD).length;
  parts.push(
    `<div>pool: <b>${alive}</b> alive / <b>${stale}</b> stale / <b>${newcount}</b> new / <b>${dead}</b> dead</div>`,
  );

  if (active) {
    parts.push(`<div>active: <code>${active.id}</code> · uptime ${formatUptime(active.createdAt)}</div>`);
    parts.push(
      `<div>pings sent ${active.totalPingsSent} · pongs ${active.totalPongsReceived} · pending ${active.pendingPingCount}</div>`,
    );
    if (active.totalPingsSent > 0) {
      const loss = Math.round(100 * (active.totalPingsSent - active.totalPongsReceived) / active.totalPingsSent);
      parts.push(`<div>loss: <b>${loss}%</b></div>`);
    }
    const w30 = rttWindow(active.rttSamples, 30_000);
    const w1m = rttWindow(active.rttSamples, 60_000);
    const w5m = rttWindow(active.rttSamples, 300_000);
    parts.push(
      `<div>RTT 30s: ${rttStr(w30)}<br>RTT 1m:&nbsp; ${rttStr(w1m)}<br>RTT 5m:&nbsp; ${rttStr(w5m)}</div>`,
    );
  }

  if (stats.reconnectScheduledAt !== null && stats.reconnectDelayMs !== null) {
    const remaining = Math.max(0, stats.reconnectScheduledAt - Date.now());
    parts.push(
      `<div>reconnect: attempt ${stats.reconnectAttempt} in ${Math.ceil(remaining / 1000)}s</div>`,
    );
  } else if (stats.reconnectDeferredUntilVisible) {
    parts.push(`<div>reconnect: deferred (tab hidden)</div>`);
  } else if (stats.isTerminal) {
    parts.push(`<div class="ws-tt-error">stopped: ${esc(stats.terminalReason ?? "")}</div>`);
  }

  // Last close for the most recent dead connection.
  const lastDead = stats.connections.find((c) => c.state === ConnectionState.DEAD);
  if (lastDead && (lastDead.closeCode !== null || lastDead.closeReason !== null)) {
    parts.push(
      `<div>last close: <code>${lastDead.closeCode ?? "?"}</code> ${esc(lastDead.closeReason ?? "")}</div>`,
    );
  }

  return parts.join("");
}

function rttWindow(samples: ReadonlyArray<RttSample>, withinMs: number):
  { min: number; median: number; max: number; count: number } | null
{
  const cutoff = Date.now() - withinMs;
  const vals: number[] = [];
  for (const s of samples) if (s.receivedAt >= cutoff) vals.push(s.rtt);
  if (vals.length === 0) return null;
  vals.sort((a, b) => a - b);
  return {
    min: vals[0] as number,
    max: vals[vals.length - 1] as number,
    median: vals[Math.floor(vals.length / 2)] as number,
    count: vals.length,
  };
}

function rttStr(w: { min: number; median: number; max: number; count: number } | null): string {
  if (w === null) return "<span class='dim'>—</span>";
  return `${w.min}/${w.median}/${w.max}ms (${w.count})`;
}

function formatUptime(since: number): string {
  const d = Date.now() - since;
  if (d < 1_000) return "<1s";
  if (d < 60_000) return `${Math.floor(d / 1_000)}s`;
  if (d < 3_600_000) return `${Math.floor(d / 60_000)}m ${Math.floor((d % 60_000) / 1_000)}s`;
  return `${Math.floor(d / 3_600_000)}h ${Math.floor((d % 3_600_000) / 60_000)}m`;
}

// --- event log ---

function renderLog(entries: ReadonlyArray<LogEntry>): string {
  const display = entries.slice(0, 100);
  return display
    .map((e) => {
      const t = new Date(e.at).toLocaleTimeString();
      return `<li class="ws-log-${e.kind}"><span class="dim">${t}</span> ${esc(e.message)}</li>`;
    })
    .join("");
}

function esc(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}
