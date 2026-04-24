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

pub use types::{parse_eddi, parse_zappi};

use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use diqwest::WithDigestAuth;
use reqwest::Client as HttpClient;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info, warn};

use victron_controller_core::myenergi::{EddiMode, ZappiMode};
use victron_controller_core::types::{Event, MyenergiAction, TypedReading};

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
            return Err(anyhow::anyhow!(
                "myenergi {url} returned {status}: {body}"
            ));
        }
        serde_json::from_str(&body)
            .with_context(|| format!("parse myenergi JSON from {url}"))
    }

    // --- Polls ---

    pub async fn poll_zappi(&self) -> Result<Option<types::ZappiObservation>> {
        if !self.has_credentials() {
            return Ok(None);
        }
        let Some(serial) = self.config.zappi_serial.as_deref() else {
            return Ok(None);
        };
        let body = self.get_json(&format!("/cgi-jstatus-Z{serial}")).await?;
        Ok(parse_zappi(&body))
    }

    pub async fn poll_eddi(&self) -> Result<Option<EddiMode>> {
        if !self.has_credentials() {
            return Ok(None);
        }
        let Some(serial) = self.config.eddi_serial.as_deref() else {
            return Ok(None);
        };
        let body = self.get_json(&format!("/cgi-jstatus-E{serial}")).await?;
        Ok(parse_eddi(&body))
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

#[derive(Debug)]
pub struct Poller {
    client: Client,
    poll_period: Duration,
}

impl Poller {
    #[must_use]
    pub const fn new(client: Client, poll_period: Duration) -> Self {
        Self {
            client,
            poll_period,
        }
    }

    pub async fn run(self, tx: mpsc::Sender<Event>) -> Result<()> {
        if !self.client.has_credentials() {
            info!("myenergi poller disabled (no credentials configured)");
            return Ok(());
        }
        info!(
            period_s = self.poll_period.as_secs(),
            "myenergi poller started"
        );
        let mut ticker = interval(self.poll_period);
        loop {
            ticker.tick().await;
            self.poll_once(&tx).await;
        }
    }

    async fn poll_once(&self, tx: &mpsc::Sender<Event>) {
        let now = Instant::now();
        match self.client.poll_zappi().await {
            Ok(Some(obs)) => {
                if tx
                    .send(Event::TypedSensor(TypedReading::Zappi {
                        state: obs.state,
                        at: now,
                    }))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            Ok(None) => debug!("zappi poll: no credentials/serial"),
            Err(e) => warn!(error = %e, "zappi poll failed"),
        }

        match self.client.poll_eddi().await {
            Ok(Some(mode)) => {
                if tx
                    .send(Event::TypedSensor(TypedReading::Eddi { mode, at: now }))
                    .await
                    .is_err()
                {}
            }
            Ok(None) => debug!("eddi poll: no credentials/serial"),
            Err(e) => warn!(error = %e, "eddi poll failed"),
        }
    }
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
