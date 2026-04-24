# Plan — A-56: D-Bus writer reconnect, retry, and bounded calls

## 1. Module structure (`writer.rs`)

Replace `Writer { conn, services, dry_run }` with:

- `pub struct Writer { services: DbusServices, dry_run: bool, inner: tokio::sync::Mutex<WriterInner> }`
- `struct WriterInner { conn: Option<Connection>, next_reconnect_earliest: Instant, backoff: Duration, last_healthy_at: Option<Instant>, last_warn_at: Option<Instant> }`

Constants (mirror subscriber names where applicable):
- `RECONNECT_BACKOFF_INITIAL = Duration::from_millis(500)`
- `RECONNECT_BACKOFF_MAX = Duration::from_secs(30)`
- `HEALTHY_THRESHOLD = Duration::from_secs(60)` — once `last_healthy_at` is older than this and a fresh write succeeds, reset backoff to initial.
- `SET_VALUE_TIMEOUT = Duration::from_secs(2)` — matches subscriber's `GET_ITEMS_TIMEOUT`.
- `THROTTLED_WARN_DEDUP = Duration::from_secs(5)` — don't spam the log with "throttled" warns on every controller re-proposal.

`zbus::Connection` is internally `Arc`-cloned. Mutex is held only across `conn.clone()` / state mutation, never across the `SetValue` call itself.

## 2. Lifecycle

`pub fn new(services: DbusServices, dry_run: bool) -> Self` — pure, infallible. Sets `conn: None`, `backoff: INITIAL`, `next_reconnect_earliest: Instant::now()`, `last_healthy_at: None`, `last_warn_at: None`. No I/O. (Lazy-connect rationale: §6.)

`pub async fn write(&self, target, value)` — signature unchanged. Flow:

1. Resolve `(svc, path)` from `target`; warn-and-return on `None`. If `dry_run`, debug-log and return (unchanged).
2. **Acquire connection** via private `async fn connection(&self) -> Option<Connection>`:
   - Lock `inner`.
   - If `inner.conn.is_some()`, clone the handle and return it (drop lock).
   - Else, if `Instant::now() < next_reconnect_earliest`, throttle. Log `warn!` with `throttle_remaining_ms` only if `last_warn_at` is `None` or older than `THROTTLED_WARN_DEDUP`; update `last_warn_at`. Return `None`.
   - Else attempt `tokio::time::timeout(SET_VALUE_TIMEOUT, Connection::system()).await`. On success: store, set `last_healthy_at = Some(now)`, `info!("dbus writer connected")`, return clone. On failure: leave `conn = None`, `next_reconnect_earliest = now + backoff`, `backoff = (backoff*2).min(MAX)`, `warn!` once per window with the error + next-retry duration, return `None`.
3. If `connection()` returned `None`, return from `write` (no panic, no further work, no `ActuatedPhase` emission — see §3).
4. Run `tokio::time::timeout(SET_VALUE_TIMEOUT, self.set_value(&conn, &svc, &path, value)).await`.
5. **Mark healthy / unhealthy**:
   - On `Ok(Ok(()))`: lock `inner`; if `last_healthy_at.map_or(true, |t| now.duration_since(t) > HEALTHY_THRESHOLD)` and `backoff > INITIAL`, reset `backoff = INITIAL` and log `info!("dbus writer backoff reset after {hold} healthy")`. Always update `last_healthy_at = Some(now)`. Drop lock. `debug!("WriteDbus ok")`.
   - On `Err(_elapsed)` or `Ok(Err(_))`: lock `inner`; `inner.conn = None`; `next_reconnect_earliest = now + backoff`; `backoff = (backoff * 2).min(MAX)`; drop lock. `error!` with the original error string. The next `write` hits the throttle gate.

`set_value` retains its body (build `Proxy`, encode value, call `SetValue`, check `status == 0`), but takes `&Connection` by reference.

## 3. Phase semantics

Writer does NOT publish `ActuatedPhase{Unset}`. Reasons:
- Writer's input is `(DbusTarget, DbusValue)` — no handle to `World` or phase store.
- Phase transitions are a core/runtime concern. Adding a side-channel from writer back to core would invert the dispatch direction.

Behavioural consequence on sustained outage: TASS stays in `Commanded` indefinitely because no readback arrives (subscriber is dead too in that scenario). This matches "fail-closed for device state"; once the bus comes back, subscriber reseed delivers fresh readbacks and TASS advances. Follow-up ticket: have core demote phases to `Unset` on `last_readback_at` staleness. That belongs in core, not here.

## 4. Backoff invariants (precise)

- Initial: `backoff = 500ms`, `next_reconnect_earliest = now`, `last_healthy_at = None`, `last_warn_at = None`.
- Each connect failure or write failure: `next_reconnect_earliest = now + backoff; backoff = min(backoff*2, 30s)`.
- Each successful write updates `last_healthy_at = now`. Reset rule fires when a successful write arrives with `last_healthy_at` older than `HEALTHY_THRESHOLD` (60 s) — we have evidence the new connection has been usable for that long. Matches subscriber's "session-age > threshold ⇒ reset" semantics adapted for request-driven traffic.
- Throttle window applies only when `conn.is_none()`. While `conn.is_some()`, writes go through until they fail.

## 5. `main.rs` callsite (line ~137)

`Writer::connect(services, dry_run).await?` → `Writer::new(services, dry_run)` — drops `.await?` and the `.context(...)`. Info log reworded to `"creating D-Bus writer (dry_run={}, lazy connect)"`. Making `new` infallible removes the "Venus down at boot ⇒ controller crashloops" failure mode. Controller already tolerates a dead bus at runtime via subscriber reconnect (PR-URGENT-20); symmetrising the writer means a Venus reboot during shell startup no longer unit-fails.

## 6. Eager vs lazy tradeoff

| Aspect | Eager (status quo) | Lazy (this plan) |
|---|---|---|
| Boot-time signal of broken bus | systemd unit fails fast | Visible via warn logs on first write |
| Resilience to bus-down-at-boot | Crashloops | Recovers without restart |
| Symmetry with subscriber | No (subscriber lazy post-PR-URGENT-20) | Yes |
| Test surface | Hard to test constructor | `Writer::new` pure → trivially testable |

Choice: **lazy**. Fast-fail diagnostic replaceable by a one-shot reachability log at startup if operators miss it; resilience is the bigger win.

## 7. Test plan

Cannot test (no bus in CI):
- Real `Connection::system()` success/failure.
- Real `SetValue` round-trip including timeout.

Can test (added in `writer.rs` under `#[cfg(test)]`):
1. `new_is_infallible` — construct Writer with test services, no panic, no `Result`.
2. `dry_run_skips_dispatch` — construct in dry-run, call `write`, confirm no panic / no connection attempt.
3. `resolve_covers_every_target` — iterate all `DbusTarget` variants (incl. every `ScheduleField`), assert `resolve` returns `Some(_)`.
4. `next_backoff_doubles_capped` — extract the arithmetic into `pub(crate) fn next_backoff(current: Duration) -> Duration { (current * 2).min(RECONNECT_BACKOFF_MAX) }`; test doubling + cap + identity-at-cap.

Runtime/bus-level testing deferred to live-Venus field verification.

## 8. Risks / open questions

- **Throttle-and-drop vs queue.** Queue rejected: TASS re-proposes on every tick (controllers are idempotent), so a queue duplicates work and risks stale replay. Drop is correct.
- **Disconnected log burst.** Dedup via `last_warn_at` + `THROTTLED_WARN_DEDUP`. First failure logs `error!`; subsequent throttled-skip `warn!`s collapse.
- **Mutex contention.** Dispatch is serial in `Runtime::dispatch`. Lock held for microseconds.
- **Zbus error classification.** Treating any `set_value` error as "drop the connection" is more aggressive than necessary (e.g. `SetValue` returning status=1 is application-level, not bus). First cut: drop on every failure — over-reconnect is cheap (one `Connection::system()` call) and avoids a brittle error-kind allowlist. Refinement ticket later if noisy.

## 9. Verification commands

```
nix develop --command cargo test --all
nix develop --command cargo clippy --all-targets -- -D warnings
nix develop --command cargo build --target armv7-unknown-linux-gnueabihf --release
```

## Critical files

- `crates/shell/src/dbus/writer.rs` — main change.
- `crates/shell/src/main.rs` — callsite adjust.
- `crates/shell/src/dbus/subscriber.rs` — reference (backoff constants, naming).
- `crates/shell/src/runtime.rs` — callsite verification (`writer.write(...)` signature unchanged).
- `crates/shell/src/dbus/mod.rs` — re-export unchanged.
