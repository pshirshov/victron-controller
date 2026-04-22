// ManagedConnection: a single WebSocket wrapped with a state machine
// (NEW → ALIVE → STALE → DEAD), per-nonce ping/pong liveness, and
// RTT tracking.
//
// Adapted from pshirshov/ws-reconnect-demo's src/client/connection.ts
// (the demo uses a generic ping/pong protocol; here we carry our
// baboon-typed WsPing / WsPong messages inside the WsClientMessage /
// WsServerMessage ADTs).

export type ConnState = "NEW" | "ALIVE" | "STALE" | "DEAD";

export interface ConnectionConfig {
  url: string;
  pingIntervalMs: number;
  pongTimeoutMs: number;
  connectTimeoutMs: number;
  staleGracePeriodMs: number;
}

export interface PingBody { nonce: string; client_ts_ms: number; }
export interface PongBody { nonce: string; client_ts_ms: number; server_ts_ms: number; }

export type Listener = (conn: ManagedConnection) => void;

let nextId = 1;

export class ManagedConnection {
  readonly id = nextId++;
  state: ConnState = "NEW";
  createdAt = Date.now();
  closedAt: number | null = null;
  closeCode: number | null = null;
  closeReason: string = "";

  // Pending pings keyed by nonce.
  private pending = new Map<string, { sentAt: number; timer: number }>();
  private pingTimer: number | null = null;
  private connectTimer: number | null = null;
  private staleTimer: number | null = null;

  private ws: WebSocket | null = null;
  private listeners = new Set<Listener>();

  // RTT samples (ms).
  rttLast = 0;
  rttMin = Infinity;
  rttMax = 0;
  rttSum = 0;
  rttCount = 0;

  pings = 0;
  pongs = 0;

  constructor(
    private readonly config: ConnectionConfig,
    private readonly onServerMessage: (raw: unknown) => void,
  ) {
    try {
      this.ws = new WebSocket(config.url);
    } catch (_e) {
      this.transition("DEAD", "constructor-threw");
      return;
    }
    this.ws.onopen = () => this.onOpen();
    this.ws.onclose = (e) => this.onClose(e.code, e.reason);
    this.ws.onerror = () => {}; // onclose always fires after
    this.ws.onmessage = (e) => this.onMessage(e);

    this.connectTimer = window.setTimeout(() => {
      if (this.state === "NEW" || (this.ws && this.ws.readyState === WebSocket.CONNECTING)) {
        this.close(4000, "connect-timeout");
      }
    }, config.connectTimeoutMs);
  }

  addListener(fn: Listener) { this.listeners.add(fn); }
  removeListener(fn: Listener) { this.listeners.delete(fn); }

  send(raw: string): boolean {
    if (this.state !== "ALIVE" || !this.ws || this.ws.readyState !== WebSocket.OPEN) return false;
    this.ws.send(raw);
    return true;
  }

  close(code = 1000, reason = "") {
    if (this.state === "DEAD") return;
    this.closeCode = code;
    this.closeReason = reason;
    try { this.ws?.close(code, reason); } catch (_e) { /* ignore */ }
    this.transition("DEAD", reason);
  }

  private onOpen() {
    if (this.connectTimer !== null) { window.clearTimeout(this.connectTimer); this.connectTimer = null; }
    this.transition("ALIVE", "open");
    this.scheduleNextPing();
  }

  private onClose(code: number, reason: string) {
    if (this.connectTimer !== null) { window.clearTimeout(this.connectTimer); this.connectTimer = null; }
    this.closeCode = code;
    this.closeReason = reason || "";
    this.transition("DEAD", `close ${code}`);
  }

  private onMessage(ev: MessageEvent) {
    if (this.state === "DEAD") return;
    let parsed: unknown;
    try { parsed = JSON.parse(ev.data); } catch { return; }
    this.onServerMessage(parsed);
  }

  /// Call when a Pong for a known nonce arrives.
  handlePong(body: PongBody) {
    const entry = this.pending.get(body.nonce);
    if (!entry) return; // unknown / already timed out
    window.clearTimeout(entry.timer);
    this.pending.delete(body.nonce);
    this.pongs++;
    const rtt = Date.now() - entry.sentAt;
    this.rttLast = rtt;
    this.rttMin = Math.min(this.rttMin, rtt);
    this.rttMax = Math.max(this.rttMax, rtt);
    this.rttSum += rtt;
    this.rttCount++;
    // A pong during STALE revives us to ALIVE.
    if (this.state === "STALE") {
      this.transition("ALIVE", "pong-after-stale");
      if (this.staleTimer !== null) { window.clearTimeout(this.staleTimer); this.staleTimer = null; }
    }
  }

  private scheduleNextPing() {
    if (this.state === "DEAD") return;
    if (this.pingTimer !== null) window.clearTimeout(this.pingTimer);
    this.pingTimer = window.setTimeout(() => this.sendPing(), this.config.pingIntervalMs);
  }

  private sendPing() {
    if (this.state === "DEAD" || !this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    const nonce = Math.random().toString(36).slice(2) + Math.random().toString(36).slice(2);
    const clientTs = Date.now();
    const body: PingBody = { nonce, client_ts_ms: clientTs };
    const timer = window.setTimeout(() => this.onPongTimeout(nonce), this.config.pongTimeoutMs);
    this.pending.set(nonce, { sentAt: clientTs, timer });
    this.pings++;
    try {
      this.ws.send(JSON.stringify({ Ping: { body } }));
    } catch (_e) {
      this.close(4001, "send-failed");
      return;
    }
    this.scheduleNextPing();
  }

  private onPongTimeout(nonce: string) {
    const entry = this.pending.get(nonce);
    if (!entry) return; // pong arrived just in time
    this.pending.delete(nonce);
    if (this.state === "ALIVE") {
      this.transition("STALE", "pong-timeout");
      // Start the stale grace period; if we stay STALE past it, die.
      this.staleTimer = window.setTimeout(() => {
        if (this.state === "STALE") this.close(4002, "stale-grace-expired");
      }, this.config.staleGracePeriodMs);
    }
  }

  private transition(to: ConnState, reason: string) {
    if (this.state === to) return;
    this.state = to;
    if (to === "DEAD") {
      this.closedAt = Date.now();
      for (const [_nonce, e] of this.pending) window.clearTimeout(e.timer);
      this.pending.clear();
      if (this.pingTimer !== null) { window.clearTimeout(this.pingTimer); this.pingTimer = null; }
      if (this.staleTimer !== null) { window.clearTimeout(this.staleTimer); this.staleTimer = null; }
    }
    for (const l of this.listeners) l(this);
    console.debug(`[ws ${this.id}] → ${to} (${reason})`);
  }

  pendingCount() { return this.pending.size; }
  avgRtt() { return this.rttCount > 0 ? Math.round(this.rttSum / this.rttCount) : 0; }
  lossPct() {
    if (this.pings === 0) return 0;
    return Math.round(100 * (this.pings - this.pongs) / this.pings);
  }
}
