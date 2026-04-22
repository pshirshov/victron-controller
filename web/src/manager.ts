// ConnectionManager: keeps one live connection, creates a replacement
// the moment the current one goes STALE, tears down the old once the
// new one establishes or the grace period expires. Exponential backoff
// for repeated connect failures; resets to zero on successful ALIVE.
//
// Adapted from pshirshov/ws-reconnect-demo's src/client/manager.ts.

import { ConnState, ConnectionConfig, ManagedConnection, PongBody } from "./connection.js";

export interface ManagerStats {
  active: ManagedConnection | null;
  all: ManagedConnection[];
  backoffMs: number;
  nextAttemptAt: number | null;
}

export type StatsListener = (stats: ManagerStats) => void;

export interface ManagerCallbacks {
  onServerMessage: (raw: unknown) => void;
}

const INITIAL_BACKOFF_MS = 500;
const MAX_BACKOFF_MS = 30_000;

export class ConnectionManager {
  private connections: ManagedConnection[] = [];
  private listeners = new Set<StatsListener>();
  private backoffMs = INITIAL_BACKOFF_MS;
  private nextAttemptTimer: number | null = null;
  private nextAttemptAt: number | null = null;
  private stopped = false;

  constructor(
    private readonly config: ConnectionConfig,
    private readonly cbs: ManagerCallbacks,
  ) {}

  start() {
    this.stopped = false;
    this.openNew();
  }

  stop() {
    this.stopped = true;
    if (this.nextAttemptTimer !== null) {
      window.clearTimeout(this.nextAttemptTimer);
      this.nextAttemptTimer = null;
    }
    for (const c of this.connections) c.close(1000, "manager-stop");
    this.connections = [];
    this.notify();
  }

  addListener(fn: StatsListener) { this.listeners.add(fn); fn(this.stats()); }
  removeListener(fn: StatsListener) { this.listeners.delete(fn); }

  /// Return the live connection, if any.
  active(): ManagedConnection | null {
    return this.connections.find((c) => c.state === "ALIVE") ?? null;
  }

  /// Route an incoming pong to whichever connection issued that nonce.
  deliverPong(body: PongBody) {
    for (const c of this.connections) c.handlePong(body);
  }

  /// Send a raw serialized WsClientMessage; uses the currently-active
  /// connection. Returns true if handed to a socket.
  send(raw: string): boolean {
    const a = this.active();
    if (!a) return false;
    return a.send(raw);
  }

  stats(): ManagerStats {
    return {
      active: this.active(),
      all: this.connections.slice(),
      backoffMs: this.backoffMs,
      nextAttemptAt: this.nextAttemptAt,
    };
  }

  private notify() {
    const s = this.stats();
    for (const l of this.listeners) l(s);
  }

  private openNew() {
    if (this.stopped) return;
    const conn = new ManagedConnection(this.config, this.cbs.onServerMessage);
    conn.addListener((c) => this.onStateChange(c));
    this.connections.push(conn);
    this.notify();
  }

  private onStateChange(c: ManagedConnection) {
    if (c.state === "ALIVE") {
      // Successful connection → reset backoff.
      this.backoffMs = INITIAL_BACKOFF_MS;
      // Supersede any older non-DEAD connections.
      for (const other of this.connections) {
        if (other !== c && other.state !== "DEAD") {
          other.close(4003, "superseded");
        }
      }
    } else if (c.state === "STALE") {
      // Start a replacement in parallel, let the old one try to recover.
      const hasSpare = this.connections.some((other) => other !== c && other.state !== "DEAD");
      if (!hasSpare) this.openNew();
    } else if (c.state === "DEAD") {
      // If no live connection remains, schedule backoff.
      this.cleanupDead();
      if (!this.active() && !this.connections.some((o) => o.state === "NEW" || o.state === "STALE")) {
        this.scheduleReconnect();
      }
    }
    this.notify();
  }

  private cleanupDead() {
    // Keep DEAD connections briefly for UI; drop after 60s.
    const cutoff = Date.now() - 60_000;
    this.connections = this.connections.filter((c) => c.state !== "DEAD" || (c.closedAt ?? 0) > cutoff);
  }

  private scheduleReconnect() {
    if (this.stopped) return;
    if (this.nextAttemptTimer !== null) return;
    const delay = this.backoffMs;
    this.nextAttemptAt = Date.now() + delay;
    console.debug(`[manager] reconnecting in ${delay}ms`);
    this.nextAttemptTimer = window.setTimeout(() => {
      this.nextAttemptTimer = null;
      this.nextAttemptAt = null;
      this.backoffMs = Math.min(MAX_BACKOFF_MS, this.backoffMs * 2);
      this.openNew();
    }, delay);
    this.notify();
  }
}

export { ConnState };
