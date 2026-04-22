// ConnectionManager: keeps a pool of ManagedConnection instances with
// overlapping-connection failover, full-jitter backoff, page-lifecycle
// wiring, time-jump detection, defer-while-hidden, terminal state.
//
// Adapted from pshirshov/ws-reconnect-demo (src/client/manager.ts).

import {
  ConnectionConfig,
  ConnectionState,
  ConnectionStats,
  ManagedConnection,
  ServerMessageCallback,
} from "./connection.js";

export interface ManagerConfig extends ConnectionConfig {
  maxReconnectAttempts: number;
  baseBackoffMs: number;
  maxBackoffMs: number;
  maxLiveConnections: number;
  /// When the event loop stalled for longer than this, assume the
  /// socket is probably dead and proactively reconnect.
  timeJumpThresholdMs: number;
}

export const DEFAULT_CONFIG: ManagerConfig = {
  url: "",
  pingIntervalMs: 5_000,
  pongTimeoutMs: 3_000,
  staleGracePeriodMs: 15_000,
  connectTimeoutMs: 10_000,
  maxReconnectAttempts: 15,
  baseBackoffMs: 1_000,
  maxBackoffMs: 30_000,
  maxLiveConnections: 3,
  timeJumpThresholdMs: 3_000,
};

// R7 — close codes we should NOT reconnect on.
const NON_RETRIABLE_CODES = new Set<number>([
  1002, // Protocol Error
  1003, // Unsupported Data
  1007, // Invalid Payload
  1009, // Message Too Big
  1010, // Mandatory Extension
  1015, // TLS Failure
]);

const TICK_MS = 1_000;

export type LogEntry = {
  at: number;
  kind: "state" | "lifecycle" | "reconnect" | "info" | "error";
  message: string;
};

export interface ManagerStats {
  connections: ConnectionStats[];
  activeConnectionId: string | null;
  reconnectAttempt: number;
  reconnectScheduledAt: number | null;
  reconnectDelayMs: number | null;
  reconnectDeferredUntilVisible: boolean;
  isTerminal: boolean;
  terminalReason: string | null;
  frozen: boolean;
}

export type UpdateCallback = (stats: ManagerStats) => void;

export class ConnectionManager {
  private connections: ManagedConnection[] = [];
  private deadConnections: ConnectionStats[] = [];
  private config: ManagerConfig;

  private reconnectAttempt = 0;
  private reconnectTimeoutId: ReturnType<typeof setTimeout> | null = null;
  private reconnectScheduledAt: number | null = null;
  private reconnectDelayMs: number | null = null;
  private reconnectDeferredUntilVisible = false;

  private isTerminal = false;
  private terminalReason: string | null = null;
  private destroyed = false;
  private frozen = false;

  private tickIntervalId: ReturnType<typeof setInterval> | null = null;
  private lastTickAt = Date.now();

  private log: LogEntry[] = [];
  private static readonly MAX_LOG = 500;

  private readonly onUpdate: UpdateCallback;
  private readonly onServerMessage: ServerMessageCallback;

  private lifecycleHandlers: Array<{ target: EventTarget; type: string; fn: EventListener }> = [];

  constructor(
    config: ManagerConfig,
    onServerMessage: ServerMessageCallback,
    onUpdate: UpdateCallback,
  ) {
    this.config = config;
    this.onServerMessage = onServerMessage;
    this.onUpdate = onUpdate;
  }

  start(): void {
    this.destroyed = false;
    this.setupLifecycleListeners();
    this.startTimeJumpDetector();
    this.openNew();
    this.notify();
  }

  destroy(): void {
    this.destroyed = true;
    this.teardownLifecycleListeners();
    this.stopTimeJumpDetector();
    this.cancelScheduledReconnect();
    for (const c of this.connections) c.close("manager-destroyed");
    this.connections = [];
    this.notify();
  }

  /// Send an application command via the active connection. Returns true
  /// if handed to a live socket.
  sendCommand(cmd: unknown): boolean {
    const active = this.connections.find((c) => c.currentState === ConnectionState.ALIVE);
    if (!active) return false;
    return active.sendCommand(cmd);
  }

  getStats(): ManagerStats {
    const connStats = this.connections.map((c) => c.getStats());
    const active = connStats.find((s) => s.state === ConnectionState.ALIVE);
    return {
      connections: [...connStats, ...this.deadConnections],
      activeConnectionId: active?.id ?? null,
      reconnectAttempt: this.reconnectAttempt,
      reconnectScheduledAt: this.reconnectScheduledAt,
      reconnectDelayMs: this.reconnectDelayMs,
      reconnectDeferredUntilVisible: this.reconnectDeferredUntilVisible,
      isTerminal: this.isTerminal,
      terminalReason: this.terminalReason,
      frozen: this.frozen,
    };
  }

  getLog(): ReadonlyArray<LogEntry> { return this.log; }

  /// "Force a reconnect now" UI affordance (R13 terminal-state escape).
  retryNow(): void {
    if (this.destroyed) return;
    this.cancelScheduledReconnect();
    this.isTerminal = false;
    this.terminalReason = null;
    this.reconnectAttempt = 0;
    this.reconnectDeferredUntilVisible = false;
    this.openNew();
    this.logEntry("reconnect", "manual retry");
    this.notify();
  }

  // --- private: lifecycle ---

  private setupLifecycleListeners(): void {
    const on = (target: EventTarget, type: string, fn: EventListener) => {
      target.addEventListener(type, fn);
      this.lifecycleHandlers.push({ target, type, fn });
    };

    on(document, "visibilitychange", () => {
      if (document.visibilityState === "visible") {
        this.logEntry("lifecycle", "visible");
        this.checkAllConnections();
        if (this.reconnectDeferredUntilVisible) {
          this.reconnectDeferredUntilVisible = false;
          this.scheduleReconnect();
        }
      } else {
        this.logEntry("lifecycle", "hidden");
      }
      this.notify();
    });

    on(window, "online", () => {
      this.logEntry("lifecycle", "online");
      this.checkAllConnections();
      if (this.reconnectDeferredUntilVisible) {
        this.reconnectDeferredUntilVisible = false;
        this.scheduleReconnect();
      }
      this.notify();
    });
    on(window, "offline", () => {
      this.logEntry("lifecycle", "offline");
      this.notify();
    });

    // BFCache: close sockets on pagehide(persisted=true), reopen on pageshow(persisted=true).
    on(window, "pagehide", (e: Event) => {
      const pe = e as PageTransitionEvent;
      if (pe.persisted) {
        this.logEntry("lifecycle", "pagehide persisted (bfcache)");
        for (const c of this.connections) c.close("bfcache");
      }
    });
    on(window, "pageshow", (e: Event) => {
      const pe = e as PageTransitionEvent;
      if (pe.persisted) {
        this.logEntry("lifecycle", "pageshow persisted (bfcache)");
        // Reset backoff and open fresh.
        this.reconnectAttempt = 0;
        this.isTerminal = false;
        this.openNew();
      }
    });

    // Chrome-only.
    on(document, "freeze", () => {
      this.logEntry("lifecycle", "freeze");
    });
    on(document, "resume", () => {
      this.logEntry("lifecycle", "resume (proactive reconnect)");
      // Treat as a long gap — NAT tables/TCP state are probably gone.
      this.proactiveReplace();
    });

    // Network Information API.
    const nav = navigator as Navigator & { connection?: EventTarget };
    if (nav.connection) {
      on(nav.connection, "change", () => {
        this.logEntry("lifecycle", "netinfo change");
        this.checkAllConnections();
      });
    }
  }

  private teardownLifecycleListeners(): void {
    for (const { target, type, fn } of this.lifecycleHandlers) {
      target.removeEventListener(type, fn);
    }
    this.lifecycleHandlers = [];
  }

  // --- private: time-jump ---

  private startTimeJumpDetector(): void {
    this.lastTickAt = Date.now();
    this.tickIntervalId = setInterval(() => {
      if (this.destroyed) return;
      const now = Date.now();
      const elapsed = now - this.lastTickAt;
      this.lastTickAt = now;
      if (elapsed > TICK_MS + this.config.timeJumpThresholdMs) {
        this.handleResume(elapsed);
      }
    }, TICK_MS);
  }

  private stopTimeJumpDetector(): void {
    if (this.tickIntervalId !== null) {
      clearInterval(this.tickIntervalId);
      this.tickIntervalId = null;
    }
  }

  private handleResume(elapsedMs: number): void {
    this.logEntry("info", `time jump detected: ${elapsedMs}ms`);
    if (elapsedMs >= this.config.pongTimeoutMs) {
      this.proactiveReplace();
    } else {
      this.checkAllConnections();
    }
  }

  // --- private: connection factory ---

  private openNew(): void {
    if (this.destroyed || this.isTerminal) return;
    if (this.connections.length >= this.config.maxLiveConnections) {
      this.logEntry("error", `pool cap (${this.config.maxLiveConnections}) reached; not opening`);
      return;
    }
    const conn = new ManagedConnection(
      this.config,
      (c, old, n) => this.handleStateChange(c, old, n),
      this.onServerMessage,
    );
    this.connections.push(conn);
    this.logEntry("state", `${conn.id}: NEW`);
  }

  private proactiveReplace(): void {
    if (this.destroyed || this.isTerminal) return;
    // Kill everything except NEW ones (they might still succeed), open a
    // replacement. Pool cap enforced by openNew.
    for (const c of this.connections) {
      if (c.currentState === ConnectionState.ALIVE || c.currentState === ConnectionState.STALE) {
        c.close("proactive-replace");
      }
    }
    this.openNew();
  }

  private checkAllConnections(): void {
    for (const c of this.connections) c.checkAlive();
  }

  private handleStateChange(
    conn: ManagedConnection,
    _old: ConnectionState,
    newState: ConnectionState,
  ): void {
    this.logEntry("state", `${conn.id}: → ${newState}`);
    if (newState === ConnectionState.ALIVE) this.handleAlive(conn);
    else if (newState === ConnectionState.STALE) this.handleStale();
    else if (newState === ConnectionState.DEAD) this.handleDead(conn);
    this.notify();
  }

  private handleAlive(alive: ManagedConnection): void {
    this.reconnectAttempt = 0;
    this.cancelScheduledReconnect();
    // Supersede any older non-DEAD connections.
    for (const other of this.connections) {
      if (other !== alive && other.currentState !== ConnectionState.DEAD) {
        other.close("superseded");
      }
    }
  }

  private handleStale(): void {
    // R6 — spin up a replacement immediately if we don't have one in flight.
    const hasSpare = this.connections.some(
      (o) => o.currentState === ConnectionState.NEW || o.currentState === ConnectionState.ALIVE,
    );
    if (!hasSpare) this.openNew();
  }

  private handleDead(conn: ManagedConnection): void {
    // Retain stats for the UI briefly.
    this.deadConnections.push(conn.getStats());
    this.deadConnections = this.deadConnections.filter(
      (s) => s.deadAt !== null && Date.now() - s.deadAt < 60_000,
    );
    this.connections = this.connections.filter((c) => c !== conn);

    const code = conn.getStats().closeCode;
    if (code !== null && NON_RETRIABLE_CODES.has(code)) {
      this.setTerminal(`permanent close code ${code}`);
      return;
    }

    // If no live/connecting connection exists, schedule a reconnect.
    const hasLive = this.connections.some(
      (c) => c.currentState === ConnectionState.ALIVE || c.currentState === ConnectionState.NEW,
    );
    if (!hasLive && !this.isTerminal) this.scheduleReconnect();
  }

  private setTerminal(reason: string): void {
    this.isTerminal = true;
    this.terminalReason = reason;
    this.cancelScheduledReconnect();
    this.logEntry("error", `terminal: ${reason}`);
  }

  // --- private: reconnect scheduling ---

  private scheduleReconnect(): void {
    if (this.destroyed || this.isTerminal) return;
    if (this.reconnectTimeoutId !== null) return;

    if (document.visibilityState === "hidden") {
      this.logEntry("reconnect", "deferred (tab hidden)");
      this.reconnectDeferredUntilVisible = true;
      return;
    }

    if (this.reconnectAttempt >= this.config.maxReconnectAttempts) {
      this.setTerminal(`max reconnect attempts (${this.config.maxReconnectAttempts}) reached`);
      return;
    }

    const delay = this.getBackoffDelay();
    this.reconnectAttempt++;
    this.reconnectDelayMs = delay;
    this.reconnectScheduledAt = Date.now() + delay;
    this.logEntry(
      "reconnect",
      `attempt ${this.reconnectAttempt}/${this.config.maxReconnectAttempts} in ${delay}ms`,
    );

    this.reconnectTimeoutId = setTimeout(() => {
      this.reconnectTimeoutId = null;
      this.reconnectScheduledAt = null;
      this.reconnectDelayMs = null;
      this.openNew();
      this.notify();
    }, delay);
  }

  private cancelScheduledReconnect(): void {
    if (this.reconnectTimeoutId !== null) {
      clearTimeout(this.reconnectTimeoutId);
      this.reconnectTimeoutId = null;
    }
    this.reconnectScheduledAt = null;
    this.reconnectDelayMs = null;
  }

  /// R5 — exponential backoff with full jitter and a cap.
  private getBackoffDelay(): number {
    const base = this.config.baseBackoffMs;
    const cap = this.config.maxBackoffMs;
    const computed = Math.min(cap, base * 2 ** this.reconnectAttempt);
    const jitter = 0.5 + Math.random() * 0.5;
    return Math.floor(computed * jitter);
  }

  // --- private: logging ---

  private logEntry(kind: LogEntry["kind"], message: string): void {
    this.log.unshift({ at: Date.now(), kind, message });
    if (this.log.length > ConnectionManager.MAX_LOG) {
      this.log.length = ConnectionManager.MAX_LOG;
    }
  }

  private notify(): void {
    try { this.onUpdate(this.getStats()); } catch (e) { console.error(e); }
  }
}
