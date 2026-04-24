//! Executes `Effect::WriteDbus` effects.
//!
//! Each write is a `SetValue` call on `com.victronenergy.BusItem`.
//! Integer paths get signed `i32`, float paths get `f64`. Errors are
//! logged but not retried — the controller will re-propose on its
//! next tick.
//!
//! The writer connects lazily and reconnects with bounded exponential
//! backoff on failure. Constructor is infallible so a Venus reboot
//! during shell startup does not crash-loop the unit (A-56).

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};
use zbus::zvariant::Value;
use zbus::{Connection, Proxy};

use victron_controller_core::types::{DbusTarget, DbusValue, ScheduleField};

use crate::config::DbusServices;

/// Initial reconnect backoff after a failed connect or write.
const RECONNECT_BACKOFF_INITIAL: Duration = Duration::from_millis(500);
/// Cap on the reconnect backoff.
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(30);
/// Once `last_healthy_at` is older than this and a fresh write
/// succeeds, reset backoff to the initial value.
const HEALTHY_THRESHOLD: Duration = Duration::from_secs(60);
/// Hard timeout on `Connection::system()` and on each `SetValue` call.
const SET_VALUE_TIMEOUT: Duration = Duration::from_secs(2);
/// Minimum gap between consecutive "throttled" warn lines while the
/// writer is in its disconnected/backoff state.
const THROTTLED_WARN_DEDUP: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct Writer {
    services: DbusServices,
    /// When false, writes are logged but not emitted. Honours the
    /// config-file `[dbus] writes_enabled` knob *in addition* to the
    /// runtime kill switch.
    dry_run: bool,
    inner: tokio::sync::Mutex<WriterInner>,
}

#[derive(Debug)]
struct WriterInner {
    conn: Option<Connection>,
    /// Writes are dropped (with throttled warn) until `Instant::now()`
    /// reaches this point.
    next_reconnect_earliest: Instant,
    /// Current backoff window. Doubles on each failure, capped at
    /// `RECONNECT_BACKOFF_MAX`.
    backoff: Duration,
    /// Set on every successful write. Used for the "long-healthy ⇒
    /// reset backoff" rule.
    last_healthy_at: Option<Instant>,
    /// Last time we emitted the throttled-skip warn. Dedup window is
    /// `THROTTLED_WARN_DEDUP`.
    last_warn_at: Option<Instant>,
    /// Last time we emitted a `WriteDbus failed` error line. Dedup
    /// window is `THROTTLED_WARN_DEDUP`. Cleared on a successful
    /// write so the first error of a fresh outage fires immediately.
    last_error_at: Option<Instant>,
}

impl Writer {
    /// Construct a writer. Pure, infallible — no I/O. The system bus
    /// connection is opened lazily on the first `write` call and
    /// re-opened on demand after a failure.
    #[must_use]
    pub fn new(services: DbusServices, dry_run: bool) -> Self {
        Self {
            services,
            dry_run,
            inner: tokio::sync::Mutex::new(WriterInner {
                conn: None,
                next_reconnect_earliest: Instant::now(),
                backoff: RECONNECT_BACKOFF_INITIAL,
                last_healthy_at: None,
                last_warn_at: None,
                last_error_at: None,
            }),
        }
    }

    pub async fn write(&self, target: DbusTarget, value: DbusValue) {
        let (svc, path) = match self.resolve(target) {
            Some(v) => v,
            None => {
                warn!(?target, "no resolved (service, path) for DbusTarget");
                return;
            }
        };
        if self.dry_run {
            debug!(%svc, %path, ?value, "DRY-RUN WriteDbus (dbus.writes_enabled=false)");
            return;
        }

        let Some(conn) = self.connection().await else {
            return;
        };

        let result = tokio::time::timeout(
            SET_VALUE_TIMEOUT,
            set_value(&conn, &svc, &path, value),
        )
        .await;

        match result {
            Ok(Ok(())) => {
                self.mark_healthy().await;
                debug!(%svc, %path, ?value, "WriteDbus ok");
            }
            Ok(Err(e)) => {
                let should_log = self.mark_failed().await;
                if should_log {
                    error!(%svc, %path, ?value, error = %e, "WriteDbus failed");
                }
            }
            Err(_elapsed) => {
                let should_log = self.mark_failed().await;
                if should_log {
                    error!(
                        %svc,
                        %path,
                        ?value,
                        timeout_ms = SET_VALUE_TIMEOUT.as_millis() as u64,
                        "WriteDbus failed"
                    );
                }
            }
        }
    }

    /// Acquire (or lazily open) the system bus connection. Returns
    /// `None` if currently throttled or if the connect attempt fails;
    /// the caller drops the write in either case.
    ///
    /// The lock is **released** across the `Connection::system()` await
    /// so a slow connect on a dead bus does not serialise unrelated
    /// writers behind a single 2 s timeout. If two callers race into
    /// the connect path concurrently, the second to finish discards
    /// its own connection (the first one wins).
    async fn connection(&self) -> Option<Connection> {
        // Phase 1: snapshot under lock — decide what to do.
        let now = Instant::now();
        {
            let mut inner = self.inner.lock().await;
            if let Some(c) = &inner.conn {
                return Some(c.clone());
            }
            if now < inner.next_reconnect_earliest {
                let remaining = inner.next_reconnect_earliest.saturating_duration_since(now);
                let should_warn = inner
                    .last_warn_at
                    .is_none_or(|t| now.duration_since(t) >= THROTTLED_WARN_DEDUP);
                if should_warn {
                    warn!(
                        throttle_remaining_ms = remaining.as_millis() as u64,
                        "dbus writer throttled; dropping write"
                    );
                    inner.last_warn_at = Some(now);
                }
                return None;
            }
            // Fall through to the connect attempt with the lock dropped.
        }

        // Phase 2: connect attempt — lock is released here so other
        // callers can short-circuit if a peer wins the race.
        let connect_result = tokio::time::timeout(SET_VALUE_TIMEOUT, Connection::system()).await;
        let after = Instant::now();

        // Phase 3: re-acquire the lock and commit the result. If a
        // peer already populated `conn`, drop our freshly-built one
        // and reuse theirs.
        let mut inner = self.inner.lock().await;
        if let Some(existing) = &inner.conn {
            return Some(existing.clone());
        }
        match connect_result {
            Ok(Ok(c)) => {
                inner.conn = Some(c.clone());
                // Note: `last_healthy_at` is *not* set here — it is
                // seeded only by a successful write in `mark_healthy`,
                // which represents real evidence of a usable bus.
                inner.last_warn_at = None;
                info!("dbus writer connected");
                Some(c)
            }
            Ok(Err(e)) => {
                let backoff = inner.backoff;
                inner.next_reconnect_earliest = after + backoff;
                inner.backoff = next_backoff(backoff);
                let should_warn = inner
                    .last_warn_at
                    .is_none_or(|t| after.duration_since(t) >= THROTTLED_WARN_DEDUP);
                if should_warn {
                    warn!(
                        error = %e,
                        next_retry_ms = backoff.as_millis() as u64,
                        "dbus writer connect failed"
                    );
                    inner.last_warn_at = Some(after);
                }
                None
            }
            Err(_elapsed) => {
                let backoff = inner.backoff;
                inner.next_reconnect_earliest = after + backoff;
                inner.backoff = next_backoff(backoff);
                let should_warn = inner
                    .last_warn_at
                    .is_none_or(|t| after.duration_since(t) >= THROTTLED_WARN_DEDUP);
                if should_warn {
                    warn!(
                        timeout_ms = SET_VALUE_TIMEOUT.as_millis() as u64,
                        next_retry_ms = backoff.as_millis() as u64,
                        "dbus writer connect timed out"
                    );
                    inner.last_warn_at = Some(after);
                }
                None
            }
        }
    }

    async fn mark_healthy(&self) {
        let now = Instant::now();
        let mut inner = self.inner.lock().await;
        if should_reset_backoff(inner.last_healthy_at, now, HEALTHY_THRESHOLD)
            && inner.backoff > RECONNECT_BACKOFF_INITIAL
        {
            let elapsed = inner
                .last_healthy_at
                .map(|t| now.duration_since(t))
                .unwrap_or_default();
            inner.backoff = RECONNECT_BACKOFF_INITIAL;
            info!(?elapsed, "dbus writer backoff reset after {elapsed:?} healthy");
        }
        inner.last_healthy_at = Some(now);
        // A successful write clears all dedup state so the first
        // warn/error of the *next* outage fires immediately.
        inner.last_warn_at = None;
        inner.last_error_at = None;
    }

    /// Mark the in-flight write as failed. Returns `true` if the
    /// caller should emit an `error!` line; `false` if the dedup
    /// window suppresses it.
    async fn mark_failed(&self) -> bool {
        let now = Instant::now();
        let mut inner = self.inner.lock().await;
        inner.conn = None;
        // Clear `last_healthy_at` so the *first* successful write
        // after the next reconnect re-seeds it instead of triggering
        // a premature "long-healthy ⇒ reset backoff" against a stale
        // pre-outage timestamp.
        inner.last_healthy_at = None;
        let backoff = inner.backoff;
        inner.next_reconnect_earliest = now + backoff;
        inner.backoff = next_backoff(backoff);
        let should_log = inner
            .last_error_at
            .is_none_or(|t| now.duration_since(t) >= THROTTLED_WARN_DEDUP);
        if should_log {
            inner.last_error_at = Some(now);
        }
        should_log
    }

    fn resolve(&self, target: DbusTarget) -> Option<(String, String)> {
        let s = &self.services;
        match target {
            DbusTarget::GridSetpoint => Some((
                s.settings.clone(),
                "/Settings/CGwacs/AcPowerSetPoint".to_string(),
            )),
            DbusTarget::InputCurrentLimit => Some((
                s.vebus.clone(),
                "/Ac/In/1/CurrentLimit".to_string(),
            )),
            DbusTarget::Schedule { index, field } => {
                let field_name = match field {
                    ScheduleField::Start => "Start",
                    ScheduleField::Duration => "Duration",
                    ScheduleField::Soc => "Soc",
                    ScheduleField::Days => "Day",
                    ScheduleField::AllowDischarge => "AllowDischarge",
                };
                Some((
                    s.settings.clone(),
                    format!(
                        "/Settings/CGwacs/BatteryLife/Schedule/Charge/{index}/{field_name}"
                    ),
                ))
            }
        }
    }
}

async fn set_value(
    conn: &Connection,
    service: &str,
    path: &str,
    value: DbusValue,
) -> Result<()> {
    let proxy = Proxy::new(conn, service, path, "com.victronenergy.BusItem")
        .await
        .context("building SetValue proxy")?;
    let v: Value<'_> = match value {
        DbusValue::Int(i) => Value::I32(i),
        DbusValue::Float(f) => Value::F64(f),
    };
    // SetValue returns an i32 status code; 0 = success.
    let status: i32 = proxy
        .call("SetValue", &(v,))
        .await
        .context("SetValue call")?;
    if status == 0 {
        Ok(())
    } else {
        Err(anyhow::anyhow!("SetValue returned status {status}"))
    }
}

/// Compute the next backoff window: double, capped at
/// `RECONNECT_BACKOFF_MAX`. Extracted as `pub(crate)` for unit
/// testability.
pub(crate) fn next_backoff(current: Duration) -> Duration {
    (current * 2).min(RECONNECT_BACKOFF_MAX)
}

/// True when the writer has been visibly healthy for longer than
/// `threshold` and a fresh successful write should reset the backoff
/// to its initial value. Crucially returns `false` when
/// `last_healthy_at` is `None` (the post-outage state, immediately
/// after a reconnect): the *first* successful write only re-seeds
/// the timestamp, it does not yet count as evidence of long health.
pub(crate) fn should_reset_backoff(
    last_healthy_at: Option<Instant>,
    now: Instant,
    threshold: Duration,
) -> bool {
    last_healthy_at.is_some_and(|t| now.duration_since(t) > threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_services() -> DbusServices {
        DbusServices::default_venus_3_70()
    }

    /// Compile-time guard: `Writer::new` is a pure, infallible
    /// `(DbusServices, bool) -> Writer`. If anyone changes the
    /// signature to return `Result`, take additional parameters, or
    /// become async, this line stops compiling.
    const _NEW_IS_INFALLIBLE: fn(DbusServices, bool) -> Writer = Writer::new;

    #[tokio::test]
    async fn dry_run_skips_dispatch() {
        // In dry-run we must not attempt any connection; the call
        // returns silently. Without a system bus available the live
        // path would either time out or fail; reaching this point at
        // all proves we short-circuited before `connection()`.
        let w = Writer::new(test_services(), true);
        w.write(DbusTarget::GridSetpoint, DbusValue::Int(0)).await;
        // Inner state must be untouched: no conn opened, backoff still
        // at initial, no warn timestamp.
        let inner = w.inner.lock().await;
        assert!(inner.conn.is_none());
        assert_eq!(inner.backoff, RECONNECT_BACKOFF_INITIAL);
        assert!(inner.last_healthy_at.is_none());
        assert!(inner.last_warn_at.is_none());
        assert!(inner.last_error_at.is_none());
    }

    #[test]
    fn resolve_covers_every_target() {
        let w = Writer::new(test_services(), true);
        assert!(w.resolve(DbusTarget::GridSetpoint).is_some());
        assert!(w.resolve(DbusTarget::InputCurrentLimit).is_some());
        for index in [0u8, 1u8] {
            for field in [
                ScheduleField::Start,
                ScheduleField::Duration,
                ScheduleField::Soc,
                ScheduleField::Days,
                ScheduleField::AllowDischarge,
            ] {
                assert!(
                    w.resolve(DbusTarget::Schedule { index, field }).is_some(),
                    "resolve returned None for index={index} field={field:?}"
                );
            }
        }
    }

    #[test]
    fn should_reset_backoff_handles_post_reconnect_state() {
        let now = Instant::now();
        let threshold = HEALTHY_THRESHOLD;

        // Post-outage / post-reconnect state: `last_healthy_at` was
        // cleared by `mark_failed`. The first successful write must
        // *not* trigger a backoff reset — it only re-seeds the
        // timestamp. (D02)
        assert!(!should_reset_backoff(None, now, threshold));

        // Recently seeded — within the threshold — also no reset.
        let just_now = now.checked_sub(Duration::from_secs(5)).unwrap();
        assert!(!should_reset_backoff(Some(just_now), now, threshold));

        // Long-healthy — reset.
        let long_ago = now
            .checked_sub(threshold)
            .unwrap()
            .checked_sub(Duration::from_secs(1))
            .unwrap();
        assert!(should_reset_backoff(Some(long_ago), now, threshold));
    }

    #[tokio::test]
    async fn mark_failed_clears_last_healthy_at() {
        // Reproduction for D02: pretend we've had a long-healthy
        // session, the bus dies, and we record a failure. The stale
        // pre-outage `last_healthy_at` must be cleared so the *next*
        // successful write does not falsely reset the backoff after
        // a single ok call.
        let w = Writer::new(test_services(), false);
        {
            let mut inner = w.inner.lock().await;
            inner.last_healthy_at = Some(
                Instant::now()
                    .checked_sub(Duration::from_secs(120))
                    .unwrap(),
            );
            inner.backoff = Duration::from_millis(500);
            inner.conn = None;
            inner.next_reconnect_earliest = Instant::now();
        }

        let _ = w.mark_failed().await;

        let inner = w.inner.lock().await;
        // After a failure, the stale healthy timestamp is cleared so
        // the next mark_healthy seeds a fresh anchor instead of
        // resetting backoff on a single write.
        assert!(inner.last_healthy_at.is_none());
        // Backoff must have advanced (was 500ms initial, now 1s).
        assert_eq!(inner.backoff, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn mark_failed_throttles_consecutive_errors() {
        // D03: the first error of a fresh outage logs immediately;
        // a follow-up failure within `THROTTLED_WARN_DEDUP` does
        // not. A successful write clears the dedup state so the
        // next outage's first error fires immediately again.
        let w = Writer::new(test_services(), false);

        // First failure of the outage: log.
        assert!(w.mark_failed().await);
        // Second failure right after: suppressed.
        assert!(!w.mark_failed().await);

        // Recovery clears the dedup state.
        w.mark_healthy().await;
        {
            let inner = w.inner.lock().await;
            assert!(inner.last_error_at.is_none());
            assert!(inner.last_warn_at.is_none());
        }

        // First failure of the next outage: log again.
        assert!(w.mark_failed().await);
    }

    #[test]
    fn next_backoff_doubles_capped() {
        // Doubling.
        assert_eq!(
            next_backoff(Duration::from_millis(500)),
            Duration::from_secs(1)
        );
        assert_eq!(
            next_backoff(Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(
            next_backoff(Duration::from_secs(8)),
            Duration::from_secs(16)
        );
        // Cap.
        assert_eq!(
            next_backoff(Duration::from_secs(20)),
            RECONNECT_BACKOFF_MAX
        );
        // Identity at cap.
        assert_eq!(next_backoff(RECONNECT_BACKOFF_MAX), RECONNECT_BACKOFF_MAX);
    }
}
