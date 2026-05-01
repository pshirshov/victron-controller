//! myenergi cloud API client for Zappi + Eddi.
//!
//! Two directions:
//!
//! - [`Poller`] — periodic background task that calls `cgi-jstatus-Z`
//!   and `cgi-jstatus-E`, parses the JSON, and emits `TypedReading`
//!   events into the core event channel.
//! - [`Writer`] — accepts `Effect::CallMyenergi(SetZappiMode | SetEddiMode)`
//!   and turns them into `cgi-zappi-mode-Z...` / `cgi-eddi-mode-E...`
//!   POST-style GET requests.
//!
//! Authentication is HTTP Digest (RFC 7616) per the myenergi API spec.
//! We use the [`diqwest`] crate which decorates a [`reqwest::Client`]
//! with the challenge/response handshake.
//!
//! When no credentials are configured (e.g. during early bring-up),
//! both the poller and writer become no-ops that log once at startup
//! and never touch the network.

mod types;

pub use types::{parse_eddi, parse_zappi, parse_zappi_signature, ZappiChangeTracker};

use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use diqwest::WithDigestAuth;
use reqwest::Client as HttpClient;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use victron_controller_core::myenergi::{EddiMode, ZappiMode, ZappiPlugState, ZappiStatus};
use victron_controller_core::types::{
    Event, MyenergiAction, SensorId, SensorReading, TimerId, TimerStatus, TypedReading,
};

use crate::config::MyenergiConfig;

/// Client shared by poller + writer. Cheap to clone — wraps an
/// `Arc<reqwest::Client>` internally.
#[derive(Debug, Clone)]
pub struct Client {
    http: HttpClient,
    config: MyenergiConfig,
}

impl Client {
    #[must_use]
    pub fn new(config: MyenergiConfig) -> Self {
        Self {
            http: HttpClient::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("reqwest client build"),
            config,
        }
    }

    #[must_use]
    pub fn has_credentials(&self) -> bool {
        !self.config.username.is_empty() && !self.config.password.is_empty()
    }

    fn url(&self, path: &str) -> String {
        let base = self.config.director_url.trim_end_matches('/');
        format!("{base}{path}")
    }

    /// GET with HTTP Digest authentication. Callers get the response
    /// as a JSON blob.
    ///
    /// A-28 (myenergi side): non-2xx responses are classified into
    /// [`MyenergiHttpFailure`] so the Poller can back off on 429 /
    /// disable the task on 401-403 rather than hammering the director
    /// at full cadence.
    async fn get_json(&self, path: &str) -> Result<serde_json::Value> {
        let url = self.url(path);
        debug!(%url, "myenergi GET");
        let resp = self
            .http
            .get(&url)
            .send_with_digest_auth(&self.config.username, &self.config.password)
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        let body = resp.text().await.context("read body")?;
        if !status.is_success() {
            let ctx = anyhow::anyhow!("myenergi {url} returned {status}: {body}");
            let code = status.as_u16();
            let failure = if code == 401 || code == 403 {
                MyenergiHttpFailure::AuthFailed(ctx)
            } else if code == 429 {
                MyenergiHttpFailure::RateLimited(ctx)
            } else if status.is_server_error() {
                MyenergiHttpFailure::ServerError(ctx)
            } else {
                MyenergiHttpFailure::Other(ctx)
            };
            return Err(anyhow::Error::from(failure));
        }
        serde_json::from_str(&body)
            .with_context(|| format!("parse myenergi JSON from {url}"))
    }

    // --- Polls ---

    /// Raw body fetch — the caller (the Poller) owns the change tracker
    /// and supplies the latched `Instant` before building a
    /// `ZappiObservation`. Returns `None` when there's no configured
    /// Zappi serial.
    pub async fn poll_zappi_raw(&self) -> Result<Option<serde_json::Value>> {
        if !self.has_credentials() {
            return Ok(None);
        }
        let Some(serial) = self.config.zappi_serial.as_deref() else {
            return Ok(None);
        };
        let body = self.get_json(&format!("/cgi-jstatus-Z{serial}")).await?;
        Ok(Some(body))
    }

    /// PR-EDDI-SENSORS-1: returns the parsed `EddiMode` *and* the raw
    /// JSON body so the poller can stamp the body onto the typed
    /// reading. The caller pretty-prints the body for the dashboard's
    /// raw-response panel.
    pub async fn poll_eddi(&self) -> Result<Option<(EddiMode, serde_json::Value)>> {
        if !self.has_credentials() {
            return Ok(None);
        }
        let Some(serial) = self.config.eddi_serial.as_deref() else {
            return Ok(None);
        };
        let body = self.get_json(&format!("/cgi-jstatus-E{serial}")).await?;
        Ok(parse_eddi(&body).map(|m| (m, body)))
    }

    // --- Writes ---

    pub async fn set_zappi_mode(&self, mode: ZappiMode) -> Result<()> {
        if !self.has_credentials() {
            return Err(anyhow!("myenergi not configured (missing credentials)"));
        }
        let Some(serial) = self.config.zappi_serial.as_deref() else {
            return Err(anyhow!("myenergi not configured (missing zappi serial)"));
        };
        let code = zappi_mode_code(mode);
        // myenergi mode-change endpoint: /cgi-zappi-mode-Z<serial>-<mode>-<boost>-<kwh>-<timeto>
        // Pass zeros for unused positional params.
        let path = format!("/cgi-zappi-mode-Z{serial}-{code}-0-0-0000");
        let body = self
            .get_json(&path)
            .await
            .with_context(|| format!("set_zappi_mode {mode:?}"))?;
        interpret_zappi_mode_response(&body)
            .with_context(|| format!("set_zappi_mode {mode:?}"))
    }

    pub async fn set_eddi_mode(&self, mode: EddiMode) -> Result<()> {
        if !self.has_credentials() {
            return Err(anyhow!("myenergi not configured (missing credentials)"));
        }
        let Some(serial) = self.config.eddi_serial.as_deref() else {
            return Err(anyhow!("myenergi not configured (missing eddi serial)"));
        };
        let code = eddi_mode_code(mode);
        let path = format!("/cgi-eddi-mode-E{serial}-{code}");
        let body = self
            .get_json(&path)
            .await
            .with_context(|| format!("set_eddi_mode {mode:?}"))?;
        interpret_eddi_mode_response(&body)
            .with_context(|| format!("set_eddi_mode {mode:?}"))
    }
}

/// Inspect a myenergi zappi mode-set HTTP response body.
///
/// Protocol: successful commands return a JSON object containing `"zsh": 0`.
/// A non-zero `zsh` is a rejection code (e.g. device busy, invalid mode).
/// A missing `zsh` is also treated as a rejection — we never saw a success
/// acknowledgement. On rejection we log the full body at `warn!` so the
/// operator can diagnose, then return `Err`.
fn interpret_zappi_mode_response(body: &serde_json::Value) -> Result<()> {
    match body.get("zsh").and_then(serde_json::Value::as_i64) {
        Some(0) => Ok(()),
        Some(code) => {
            warn!(body = %body, code, "myenergi rejected zappi mode-set");
            Err(anyhow!("myenergi rejected zappi mode-set: zsh={code}"))
        }
        None => {
            warn!(body = %body, "myenergi zappi mode-set response missing/non-numeric zsh");
            Err(anyhow!(
                "myenergi zappi mode-set response missing/non-numeric zsh: {body}"
            ))
        }
    }
}

/// Inspect a myenergi eddi mode-set HTTP response body. Same shape as zappi
/// but the success flag is `"esh"` (0 = ok, non-zero = rejection code).
fn interpret_eddi_mode_response(body: &serde_json::Value) -> Result<()> {
    match body.get("esh").and_then(serde_json::Value::as_i64) {
        Some(0) => Ok(()),
        Some(code) => {
            warn!(body = %body, code, "myenergi rejected eddi mode-set");
            Err(anyhow!("myenergi rejected eddi mode-set: esh={code}"))
        }
        None => {
            warn!(body = %body, "myenergi eddi mode-set response missing/non-numeric esh");
            Err(anyhow!(
                "myenergi eddi mode-set response missing/non-numeric esh: {body}"
            ))
        }
    }
}

/// Classification of myenergi-HTTP failures. Downcast-able from the
/// anyhow chain in the Poller's error handler so we back off correctly
/// rather than hammering a rate-limited / auth-failed director.
#[derive(Debug)]
pub enum MyenergiHttpFailure {
    AuthFailed(anyhow::Error),
    RateLimited(anyhow::Error),
    ServerError(anyhow::Error),
    Other(anyhow::Error),
}

impl std::fmt::Display for MyenergiHttpFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MyenergiHttpFailure::AuthFailed(e) => write!(f, "AuthFailed: {e:#}"),
            MyenergiHttpFailure::RateLimited(e) => write!(f, "RateLimited: {e:#}"),
            MyenergiHttpFailure::ServerError(e) => write!(f, "ServerError: {e:#}"),
            MyenergiHttpFailure::Other(e) => write!(f, "Other: {e:#}"),
        }
    }
}

impl std::error::Error for MyenergiHttpFailure {}

/// PR-timers-section: wall-clock epoch-ms helper used for the per-poll
/// `Event::TimerState` emit.
fn myenergi_epoch_ms_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| {
            i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
        })
}

/// Myenergi Zappi mode codes from their API spec.
const fn zappi_mode_code(m: ZappiMode) -> u8 {
    match m {
        ZappiMode::Fast => 1,
        ZappiMode::Eco => 2,
        ZappiMode::EcoPlus => 3,
        ZappiMode::Off => 4,
    }
}

/// Myenergi Eddi mode codes. 0 = Stopped (don't divert), 1 = Normal.
const fn eddi_mode_code(m: EddiMode) -> u8 {
    match m {
        EddiMode::Stopped => 0,
        EddiMode::Normal => 1,
    }
}

// -----------------------------------------------------------------------------
// Poller
// -----------------------------------------------------------------------------

/// The worst-case classification of a single poll cycle (zappi + eddi).
/// The Poller uses this to drive adaptive backoff (A-28).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PollOutcome {
    Ok,
    Other,
    ServerError,
    RateLimited,
    AuthFailed,
}

impl PollOutcome {
    fn severity(self) -> u8 {
        match self {
            PollOutcome::Ok => 0,
            PollOutcome::Other => 1,
            PollOutcome::ServerError => 2,
            PollOutcome::RateLimited => 3,
            PollOutcome::AuthFailed => 4,
        }
    }

    fn worst(self, other: Self) -> Self {
        if other.severity() > self.severity() { other } else { self }
    }

    fn from_error(e: &anyhow::Error) -> Self {
        match e.downcast_ref::<MyenergiHttpFailure>() {
            Some(MyenergiHttpFailure::AuthFailed(_)) => PollOutcome::AuthFailed,
            Some(MyenergiHttpFailure::RateLimited(_)) => PollOutcome::RateLimited,
            Some(MyenergiHttpFailure::ServerError(_)) => PollOutcome::ServerError,
            Some(MyenergiHttpFailure::Other(_)) | None => PollOutcome::Other,
        }
    }
}

#[derive(Debug)]
pub struct Poller {
    client: Client,
    poll_period: Duration,
    /// Latches a monotonic `Instant` on every observed
    /// `(zmo, sta, pst)` change; supplies
    /// `ZappiState::zappi_last_change_signature`. `None` until the
    /// first successful poll — see [`ZappiChangeTracker`].
    zappi_tracker: Option<ZappiChangeTracker>,
}

impl Poller {
    #[must_use]
    pub const fn new(client: Client, poll_period: Duration) -> Self {
        Self {
            client,
            poll_period,
            zappi_tracker: None,
        }
    }

    pub async fn run(mut self, tx: mpsc::Sender<Event>) -> Result<()> {
        if !self.client.has_credentials() {
            info!("myenergi poller disabled (no credentials configured)");
            return Ok(());
        }
        info!(
            period_s = self.poll_period.as_secs(),
            "myenergi poller started"
        );
        let mut ticker = interval(self.poll_period);
        // A-28 (myenergi side): extra backoff added on top of
        // `poll_period` when the director signals rate-limit or server
        // error. Reset on a successful poll.
        let mut extra_backoff = Duration::ZERO;
        const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(15 * 60);
        const SERVER_ERROR_BACKOFF_START: Duration = Duration::from_secs(30);
        const SERVER_ERROR_BACKOFF_MAX: Duration = Duration::from_secs(10 * 60);
        loop {
            ticker.tick().await;
            if extra_backoff > Duration::ZERO {
                info!(sleep_s = extra_backoff.as_secs(), "myenergi poller backoff");
                tokio::time::sleep(extra_backoff).await;
            }
            let had_failure = self.poll_once(&tx).await;
            // PR-timers-section: emit a TimerState per cycle. Auth fail
            // surfaces as `failed_last_run` with no next fire (we return).
            let timer_status = match had_failure {
                PollOutcome::Ok => TimerStatus::Idle,
                PollOutcome::AuthFailed | PollOutcome::Other => TimerStatus::FailedLastRun,
                PollOutcome::RateLimited | PollOutcome::ServerError => {
                    TimerStatus::RetryBackoff
                }
            };
            // The next-fire projection includes whatever extra backoff
            // the failure classifier *will* apply for this fire.
            let prospective_extra = match had_failure {
                PollOutcome::RateLimited => RATE_LIMIT_BACKOFF,
                PollOutcome::ServerError => {
                    if extra_backoff == Duration::ZERO {
                        SERVER_ERROR_BACKOFF_START
                    } else {
                        (extra_backoff * 2).min(SERVER_ERROR_BACKOFF_MAX)
                    }
                }
                PollOutcome::Ok | PollOutcome::AuthFailed | PollOutcome::Other => {
                    Duration::ZERO
                }
            };
            let last_fire_ms = myenergi_epoch_ms_now();
            let next_fire_ms = if matches!(had_failure, PollOutcome::AuthFailed) {
                None
            } else {
                let interval_ms =
                    i64::try_from((self.poll_period + prospective_extra).as_millis())
                        .unwrap_or(i64::MAX);
                Some(last_fire_ms + interval_ms)
            };
            if tx
                .send(Event::TimerState {
                    id: TimerId::MyenergiPoller,
                    last_fire_epoch_ms: last_fire_ms,
                    next_fire_epoch_ms: next_fire_ms,
                    status: timer_status,
                    at: Instant::now(),
                })
                .await
                .is_err()
            {
                return Ok(());
            }
            match had_failure {
                PollOutcome::Ok => extra_backoff = Duration::ZERO,
                PollOutcome::AuthFailed => {
                    error!(
                        "myenergi poller: auth failed (401/403); disabling — check credentials"
                    );
                    return Ok(());
                }
                PollOutcome::RateLimited => {
                    warn!(
                        backoff_s = RATE_LIMIT_BACKOFF.as_secs(),
                        "myenergi rate-limited (429); long backoff"
                    );
                    extra_backoff = RATE_LIMIT_BACKOFF;
                }
                PollOutcome::ServerError => {
                    extra_backoff = if extra_backoff == Duration::ZERO {
                        SERVER_ERROR_BACKOFF_START
                    } else {
                        (extra_backoff * 2).min(SERVER_ERROR_BACKOFF_MAX)
                    };
                    warn!(backoff_s = extra_backoff.as_secs(), "myenergi 5xx; exponential backoff");
                }
                PollOutcome::Other => { /* normal cadence */ }
            }
        }
    }

    async fn poll_once(&mut self, tx: &mpsc::Sender<Event>) -> PollOutcome {
        let now = Instant::now();
        let mut outcome = PollOutcome::Ok;
        let mut upgrade = |o: PollOutcome| {
            // Keep the "worst" outcome across zappi + eddi polls, per
            // severity order Auth > RateLimit > ServerErr > Other > Ok.
            outcome = outcome.worst(o);
        };
        match self.client.poll_zappi_raw().await {
            Ok(Some(body)) => match parse_zappi_signature(&body) {
                Some(tuple) => {
                    let stamp = self.stamp_zappi_change(tuple, now);
                    if let Some(obs) = parse_zappi(&body, stamp) {
                        let session_kwh = obs.state.session_kwh;
                        let raw = pretty_json(&body);
                        if tx
                            .send(Event::TypedSensor(TypedReading::Zappi {
                                state: obs.state,
                                at: now,
                                raw_json: Some(raw),
                            }))
                            .await
                            .is_err()
                        {
                            return outcome;
                        }
                        // Surface the cumulative session kWh as a
                        // first-class scalar sensor so the dashboard's
                        // sensor row picks it up — same pattern as
                        // other myenergi-derived signals. See
                        // PR-session-kwh-sensor / A-13/A-14.
                        if tx
                            .send(Event::Sensor(SensorReading {
                                id: SensorId::SessionKwh,
                                value: session_kwh,
                                at: now,
                            }))
                            .await
                            .is_err()
                        {
                            return outcome;
                        }
                    } else {
                        warn!("zappi poll: signature parsed but observation did not");
                    }
                }
                None => debug!("zappi poll: no zappi entry in body"),
            },
            Ok(None) => debug!("zappi poll: no credentials/serial"),
            Err(e) => {
                upgrade(PollOutcome::from_error(&e));
                warn!(error = %e, "zappi poll failed");
            }
        }

        match self.client.poll_eddi().await {
            Ok(Some((mode, body))) => {
                let raw = pretty_json(&body);
                if tx
                    .send(Event::TypedSensor(TypedReading::Eddi {
                        mode,
                        at: now,
                        raw_json: Some(raw),
                    }))
                    .await
                    .is_err()
                {}
            }
            Ok(None) => debug!("eddi poll: no credentials/serial"),
            Err(e) => {
                upgrade(PollOutcome::from_error(&e));
                warn!(error = %e, "eddi poll failed");
            }
        }
        outcome
    }

    /// Latches the zappi change-detection tracker and returns the
    /// appropriate `Instant` for `zappi_last_change_signature`. On the
    /// very first poll, stamps `now` as the initial signature — the
    /// classifier's `WAIT_TIMEOUT_MIN` branch then waits the full
    /// 5 min before firing, which is correct: we can't assume anything
    /// about zappi state age at startup.
    fn stamp_zappi_change(
        &mut self,
        tuple: (ZappiMode, ZappiStatus, ZappiPlugState),
        now: Instant,
    ) -> Instant {
        match self.zappi_tracker.as_mut() {
            Some(tr) => tr.observe(tuple, now),
            None => {
                self.zappi_tracker = Some(ZappiChangeTracker::new(tuple, now));
                now
            }
        }
    }
}

/// PR-EDDI-SENSORS-1: pretty-print a myenergi response body for the
/// dashboard's raw-response panel. Falls back to the default
/// `Display` if `to_string_pretty` somehow fails (it shouldn't for a
/// `serde_json::Value`).
fn pretty_json(body: &serde_json::Value) -> String {
    serde_json::to_string_pretty(body).unwrap_or_else(|_| body.to_string())
}

// -----------------------------------------------------------------------------
// Writer — executes CallMyenergi effects
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Writer {
    client: Client,
    /// When false, actions are logged but not executed. Honours the
    /// config-file `[myenergi] writes_enabled` gate, independent of the
    /// core's `knobs.writes_enabled` kill switch.
    dry_run: bool,
}

impl Writer {
    #[must_use]
    pub const fn new(client: Client, dry_run: bool) -> Self {
        Self { client, dry_run }
    }

    pub async fn execute(&self, action: MyenergiAction) {
        if self.dry_run {
            info!(?action, "myenergi action (dry-run; writes_enabled=false, not sent)");
            return;
        }
        let res = match action {
            MyenergiAction::SetZappiMode(m) => self.client.set_zappi_mode(m).await,
            MyenergiAction::SetEddiMode(m) => self.client.set_eddi_mode(m).await,
        };
        match res {
            Ok(()) => info!(?action, "myenergi action confirmed (zsh=0 or esh=0)"),
            Err(e) => warn!(?action, error = %e, "myenergi action failed"),
        }
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{interpret_eddi_mode_response, interpret_zappi_mode_response};
    use serde_json::json;

    #[test]
    fn test_zappi_mode_success_on_zsh_zero() {
        let body = json!({"zsh": 0});
        assert!(interpret_zappi_mode_response(&body).is_ok());
    }

    #[test]
    fn test_zappi_mode_rejected_on_nonzero_zsh() {
        let body = json!({"zsh": 3});
        let err = interpret_zappi_mode_response(&body).unwrap_err();
        assert!(
            err.to_string().contains("zsh=3"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_zappi_mode_rejected_on_missing_zsh() {
        let body = json!({});
        let err = interpret_zappi_mode_response(&body).unwrap_err();
        assert!(
            err.to_string().contains("missing/non-numeric zsh"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_zappi_mode_rejected_on_non_numeric_zsh() {
        let body = json!({"zsh": "ok"});
        let err = interpret_zappi_mode_response(&body).unwrap_err();
        assert!(
            err.to_string().contains("missing/non-numeric zsh"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_eddi_mode_success_on_esh_zero() {
        let body = json!({"esh": 0});
        assert!(interpret_eddi_mode_response(&body).is_ok());
    }

    #[test]
    fn test_eddi_mode_rejected_on_nonzero_esh() {
        let body = json!({"esh": 5});
        let err = interpret_eddi_mode_response(&body).unwrap_err();
        assert!(
            err.to_string().contains("esh=5"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_eddi_mode_rejected_on_missing_esh() {
        let body = json!({});
        let err = interpret_eddi_mode_response(&body).unwrap_err();
        assert!(
            err.to_string().contains("missing/non-numeric esh"),
            "unexpected error: {err}"
        );
    }
}
