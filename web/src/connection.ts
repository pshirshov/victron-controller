// ManagedConnection: WebSocket + four-state machine (NEW/ALIVE/STALE/DEAD),
// per-ping nonce timeout, RTT sampling, connect-timeout guard.
//
// Adapted from pshirshov/ws-reconnect-demo (src/client/connection.ts),
// with our baboon-typed WsClientMessage / WsServerMessage envelope.

import {
  BaboonBinReader,
  BaboonCodecContext,
} from "./model/BaboonSharedRuntime.js";
import { WorldSnapshot_UEBACodec } from "./model/victron_controller/dashboard/WorldSnapshot.js";

/// PR-tier-3-ueba: decode a baboon-UEBA-encoded `WorldSnapshot` from a
/// binary WS frame. Returns the decoded tree with `bigint` (i64) fields
/// converted to `number` in place. We do NOT rebuild the tree — that
/// would re-allocate every object the decoder just allocated and erase
/// the GC win we're trying to achieve.
///
/// The decoder produces class instances (e.g. `WorldSnapshot`,
/// `Sensors`, …) instead of plain objects; downstream renderers only
/// dot-access fields, so the class prototype is harmless. Only the i64
/// → number coercion is needed so arithmetic works (`Date.now() - ms`,
/// rounding, etc.). Epoch-millisecond values fit comfortably below
/// `Number.MAX_SAFE_INTEGER` (~285 ky from epoch).
function decodeSnapshotBinary(buf: ArrayBuffer): unknown {
  const reader = new BaboonBinReader(new Uint8Array(buf));
  const decoded = WorldSnapshot_UEBACodec.instance.decode(
    BaboonCodecContext.Default,
    reader,
  );
  normalizeBigintsInPlace(decoded as unknown as Record<string, unknown>);
  return decoded;
}

/// Walk an object tree mutating `bigint` properties to `number`. Skips
/// primitives, recurses into arrays + nested objects. Allocates only
/// the per-call `Object.keys` array — no new wrapper objects.
function normalizeBigintsInPlace(v: unknown): void {
  if (Array.isArray(v)) {
    for (let i = 0; i < v.length; i++) {
      const x = v[i];
      if (typeof x === "bigint") v[i] = Number(x);
      else if (x !== null && typeof x === "object") normalizeBigintsInPlace(x);
    }
    return;
  }
  if (v === null || typeof v !== "object") return;
  const obj = v as Record<string, unknown>;
  const keys = Object.keys(obj);
  for (let i = 0; i < keys.length; i++) {
    const k = keys[i];
    const x = obj[k];
    if (typeof x === "bigint") obj[k] = Number(x);
    else if (x !== null && typeof x === "object") normalizeBigintsInPlace(x);
  }
}

// Generate a ping nonce. `crypto.randomUUID()` is only available on
// secure contexts (HTTPS or localhost). When the dashboard is served
// over plain HTTP to a LAN IP — our default — Chrome/Firefox throw
// from randomUUID, which happens *before* our ping-sent bookkeeping
// increments and silently kills all ping traffic. Fall back to a
// timestamp + Math.random nonce; uniqueness only needs to hold across
// the few pings in flight at once.
function makeNonce(): string {
  const g = (globalThis as { crypto?: { randomUUID?: () => string } });
  if (g.crypto && typeof g.crypto.randomUUID === "function") {
    try { return g.crypto.randomUUID(); } catch { /* secure-context error */ }
  }
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

export enum ConnectionState {
  NEW = "NEW",
  ALIVE = "ALIVE",
  STALE = "STALE",
  DEAD = "DEAD",
}

export interface ConnectionConfig {
  url: string;
  pingIntervalMs: number;
  pongTimeoutMs: number;
  staleGracePeriodMs: number;
  connectTimeoutMs: number;
}

export interface RttSample {
  rtt: number;
  receivedAt: number;
}

export interface ConnectionStats {
  id: string;
  state: ConnectionState;
  createdAt: number;
  lastPingSentAt: number | null;
  lastPongReceivedAt: number | null;
  lastRtt: number | null;
  avgRtt: number | null;
  minRtt: number | null;
  maxRtt: number | null;
  pendingPingCount: number;
  earliestPendingPingSentAt: number | null;
  totalPingsSent: number;
  totalPongsReceived: number;
  rttSamples: ReadonlyArray<RttSample>;
  staleAt: number | null;
  deadAt: number | null;
  closeCode: number | null;
  closeReason: string | null;
}

export type StateChangeCallback = (
  conn: ManagedConnection,
  oldState: ConnectionState,
  newState: ConnectionState,
) => void;

export type ServerMessageCallback = (raw: unknown) => void;

let connectionCounter = 0;

export class ManagedConnection {
  readonly id: string;
  private ws: WebSocket;
  private state: ConnectionState = ConnectionState.NEW;
  private readonly createdAt: number = Date.now();

  private pendingPings = new Map<string, number>(); // nonce → sentAt
  private lastPingSentAt: number | null = null;
  private lastPongReceivedAt: number | null = null;
  private lastRtt: number | null = null;
  private rttSamples: RttSample[] = [];
  private totalPingsSent = 0;
  private totalPongsReceived = 0;
  private static readonly MAX_RTT_SAMPLES = 500;

  private staleAt: number | null = null;
  private deadAt: number | null = null;
  private closeCode: number | null = null;
  private closeReason: string | null = null;

  private pingIntervalId: ReturnType<typeof setInterval> | null = null;
  private staleTimeoutId: ReturnType<typeof setTimeout> | null = null;
  private connectTimeoutId: ReturnType<typeof setTimeout> | null = null;

  private readonly config: ConnectionConfig;
  private readonly onStateChange: StateChangeCallback;
  private readonly onServerMessage: ServerMessageCallback;
  private frozen = false;

  constructor(
    config: ConnectionConfig,
    onStateChange: StateChangeCallback,
    onServerMessage: ServerMessageCallback,
  ) {
    this.id = `conn-${++connectionCounter}`;
    this.config = config;
    this.onStateChange = onStateChange;
    this.onServerMessage = onServerMessage;

    this.ws = new WebSocket(config.url);
    // PR-tier-3-ueba: snapshots arrive as Binary frames carrying baboon
    // UEBA bytes. Switch to ArrayBuffer (default is Blob, which would
    // force an async `await blob.arrayBuffer()` on every snapshot).
    this.ws.binaryType = "arraybuffer";
    this.ws.onopen = this.handleOpen.bind(this);
    this.ws.onclose = this.handleClose.bind(this);
    this.ws.onerror = () => {}; // onclose always follows
    this.ws.onmessage = this.handleMessage.bind(this);

    // R4 — abort if TCP/TLS handshake hangs.
    this.connectTimeoutId = setTimeout(() => {
      if (
        this.state === ConnectionState.NEW &&
        this.ws.readyState === WebSocket.CONNECTING
      ) {
        this.close(`connect timeout after ${config.connectTimeoutMs}ms`);
      }
    }, config.connectTimeoutMs);
  }

  get currentState(): ConnectionState { return this.state; }

  getStats(): ConnectionStats {
    const rtt = this.rttSamples;
    let min = Infinity, max = -Infinity, sum = 0;
    for (const s of rtt) {
      if (s.rtt < min) min = s.rtt;
      if (s.rtt > max) max = s.rtt;
      sum += s.rtt;
    }
    let earliestPending: number | null = null;
    for (const sentAt of this.pendingPings.values()) {
      if (earliestPending === null || sentAt < earliestPending) earliestPending = sentAt;
    }
    return {
      id: this.id,
      state: this.state,
      createdAt: this.createdAt,
      lastPingSentAt: this.lastPingSentAt,
      lastPongReceivedAt: this.lastPongReceivedAt,
      lastRtt: this.lastRtt,
      avgRtt: rtt.length > 0 ? Math.round(sum / rtt.length) : null,
      minRtt: rtt.length > 0 ? min : null,
      maxRtt: rtt.length > 0 ? max : null,
      pendingPingCount: this.pendingPings.size,
      earliestPendingPingSentAt: earliestPending,
      totalPingsSent: this.totalPingsSent,
      totalPongsReceived: this.totalPongsReceived,
      rttSamples: rtt,
      staleAt: this.staleAt,
      deadAt: this.deadAt,
      closeCode: this.closeCode,
      closeReason: this.closeReason,
    };
  }

  /// Send an application command. Returns true if handed to a live socket.
  sendCommand(cmd: unknown): boolean {
    if (this.state !== ConnectionState.ALIVE) return false;
    if (this.ws.readyState !== WebSocket.OPEN) return false;
    try {
      this.ws.send(JSON.stringify({ SendCommand: { body: cmd } }));
      return true;
    } catch {
      return false;
    }
  }

  sendPing(): void {
    if (this.frozen) return;
    if (this.state === ConnectionState.DEAD) return;
    if (this.ws.readyState !== WebSocket.OPEN) return;

    const nonce = makeNonce();
    const now = Date.now();
    this.pendingPings.set(nonce, now);
    this.lastPingSentAt = now;
    this.totalPingsSent++;

    try {
      this.ws.send(JSON.stringify({ Ping: { body: { nonce, client_ts_ms: now } } }));
    } catch {
      this.close("send-failed");
      return;
    }

    // R3 — per-ping timeout.
    setTimeout(() => {
      if (
        this.pendingPings.has(nonce) &&
        this.state !== ConnectionState.DEAD &&
        !this.frozen
      ) {
        this.markStale();
      }
    }, this.config.pongTimeoutMs);
  }

  close(reason: string): void {
    if (this.state === ConnectionState.DEAD) return;
    this.closeCode = 1000;
    this.closeReason = reason;
    this.clearAllTimers();
    if (
      this.ws.readyState === WebSocket.OPEN ||
      this.ws.readyState === WebSocket.CONNECTING
    ) {
      this.ws.close(1000, reason.slice(0, 123));
    }
    this.transitionTo(ConnectionState.DEAD);
  }

  /// Fire an immediate ping to verify liveness — used by the time-jump
  /// detector and page-lifecycle handlers (R8, R9).
  checkAlive(): void {
    if (this.state !== ConnectionState.ALIVE && this.state !== ConnectionState.STALE) return;
    // Drop pending pings that can't possibly resolve (e.g. after a freeze).
    const now = Date.now();
    for (const [nonce, sentAt] of this.pendingPings) {
      if (now - sentAt > this.config.pongTimeoutMs * 3) this.pendingPings.delete(nonce);
    }
    this.sendPing();
  }

  setFrozen(frozen: boolean): void { this.frozen = frozen; }

  // --- private ---

  private handleOpen(): void {
    if (this.connectTimeoutId !== null) {
      clearTimeout(this.connectTimeoutId);
      this.connectTimeoutId = null;
    }
    this.transitionTo(ConnectionState.ALIVE);
    this.pingIntervalId = setInterval(() => this.sendPing(), this.config.pingIntervalMs);
    this.sendPing();
  }

  private handleClose(event: CloseEvent): void {
    if (this.state === ConnectionState.DEAD) return;
    this.closeCode = event.code;
    this.closeReason ??= `code=${event.code} reason=${event.reason || "(none)"}`;
    this.clearAllTimers();
    this.transitionTo(ConnectionState.DEAD);
  }

  private handleMessage(event: MessageEvent): void {
    if (this.frozen) return;

    // PR-tier-3-ueba: Binary frames carry the UEBA-encoded WorldSnapshot.
    // Decoded once here, wrapped to look like the previous JSON Snapshot
    // envelope so downstream consumers (`onServerMessage` in index.ts)
    // don't change.
    if (event.data instanceof ArrayBuffer) {
      let snap: unknown;
      try {
        snap = decodeSnapshotBinary(event.data);
      } catch (e) {
        // Malformed frame — log to console (no in-band channel for
        // structured warnings here) and drop. The next frame will
        // attempt a fresh decode.
        // eslint-disable-next-line no-console
        console.warn("ws ueba decode failed", e);
        return;
      }
      this.onServerMessage({ Snapshot: { body: snap } });
      return;
    }

    let msg: unknown;
    try { msg = JSON.parse(event.data as string); } catch { return; }
    if (typeof msg !== "object" || msg === null) return;
    const obj = msg as Record<string, unknown>;

    // Pong routed to our per-nonce bookkeeping.
    if ("Pong" in obj) {
      const body = (obj.Pong as { body: { nonce: string; client_ts_ms: number; server_ts_ms: number } }).body;
      this.handlePong(body.nonce);
      this.onServerMessage(msg);
      return;
    }
    this.onServerMessage(msg);
  }

  private handlePong(nonce: string): void {
    const sentAt = this.pendingPings.get(nonce);
    if (sentAt === undefined) return; // R11 equivalent: drop unsolicited pongs
    this.pendingPings.delete(nonce);

    const now = Date.now();
    const rtt = now - sentAt;
    this.lastPongReceivedAt = now;
    this.lastRtt = rtt;
    this.totalPongsReceived++;

    this.rttSamples.push({ rtt, receivedAt: now });
    if (this.rttSamples.length > ManagedConnection.MAX_RTT_SAMPLES) this.rttSamples.shift();

    if (this.state === ConnectionState.STALE) {
      if (this.staleTimeoutId !== null) {
        clearTimeout(this.staleTimeoutId);
        this.staleTimeoutId = null;
      }
      this.staleAt = null;
      this.transitionTo(ConnectionState.ALIVE);
    }
  }

  private markStale(): void {
    if (this.frozen) return;
    if (this.state !== ConnectionState.ALIVE) return;
    this.staleAt = Date.now();
    this.transitionTo(ConnectionState.STALE);
    this.staleTimeoutId = setTimeout(() => {
      if (this.state === ConnectionState.STALE) {
        this.close(`stale for ${this.config.staleGracePeriodMs}ms without recovery`);
      }
    }, this.config.staleGracePeriodMs);
  }

  private clearAllTimers(): void {
    if (this.connectTimeoutId !== null) { clearTimeout(this.connectTimeoutId); this.connectTimeoutId = null; }
    if (this.pingIntervalId !== null) { clearInterval(this.pingIntervalId); this.pingIntervalId = null; }
    if (this.staleTimeoutId !== null) { clearTimeout(this.staleTimeoutId); this.staleTimeoutId = null; }
  }

  private transitionTo(newState: ConnectionState): void {
    const oldState = this.state;
    if (oldState === newState) return;
    if (oldState === ConnectionState.DEAD) return; // terminal
    this.state = newState;
    if (newState === ConnectionState.DEAD) {
      this.deadAt = Date.now();
      this.clearAllTimers();
    }
    this.onStateChange(this, oldState, newState);
  }
}
