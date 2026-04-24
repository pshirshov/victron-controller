//! Forecast HTTP fetchers — Solcast, Forecast.Solar, Open-Meteo.
//!
//! Per SPEC §5.7, each provider runs on its own cadence, fetches
//! today's and tomorrow's kWh totals, and emits a
//! `TypedReading::Forecast` event into the core event channel.
//! Controllers (specifically `run_weather_soc`) consume the fused
//! result via `forecast_fusion::fused_today_kwh`.
//!
//! A provider task is created only when the config gives it meaningful
//! parameters (e.g. Solcast: an API key and ≥1 rooftop site id). Absent
//! config = no task = no events = no problem — the fusion layer
//! tolerates missing providers.

pub mod current_weather;
pub mod forecast_solar;
pub mod open_meteo;
pub mod solcast;

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client as HttpClient;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use victron_controller_core::types::{Event, ForecastProvider, TypedReading};

pub use forecast_solar::ForecastSolarClient;
pub use open_meteo::OpenMeteoClient;
pub use solcast::SolcastClient;

/// Shared HTTP client. Cheap to clone via `Arc`.
#[must_use]
pub fn http_client() -> HttpClient {
    HttpClient::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("victron-controller/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("reqwest client build")
}

/// One fetched snapshot ready to turn into a `TypedReading::Forecast`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForecastTotals {
    pub today_kwh: f64,
    pub tomorrow_kwh: f64,
}

/// Common trait that each provider implementation satisfies.
#[async_trait::async_trait]
pub trait ForecastFetcher: Send + Sync + std::fmt::Debug {
    fn provider(&self) -> ForecastProvider;
    async fn fetch(&self) -> Result<ForecastTotals>;
}

/// Periodic scheduler: ticks on `cadence`, calls `fetcher.fetch()`,
/// sends a `TypedReading::Forecast` event on success.
pub async fn run_scheduler(
    fetcher: Box<dyn ForecastFetcher>,
    cadence: Duration,
    tx: mpsc::Sender<Event>,
) -> Result<()> {
    let provider = fetcher.provider();
    info!(
        ?provider,
        cadence_s = cadence.as_secs(),
        "forecast scheduler started"
    );
    let mut ticker = interval(cadence);
    // Extra backoff applied on top of `cadence` when the endpoint
    // signals rate-limit or server error. Reset to zero on success.
    // See A-28: Solcast free tier is 10 calls/day, so a single 429
    // storm with the default 1 h cadence would burn the day's quota
    // in the next 10 polls.
    let mut extra_backoff = Duration::ZERO;
    const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(6 * 3600); // 6 h
    const SERVER_ERROR_BACKOFF_START: Duration = Duration::from_secs(60);
    const SERVER_ERROR_BACKOFF_MAX: Duration = Duration::from_secs(30 * 60);
    loop {
        ticker.tick().await;
        if extra_backoff > Duration::ZERO {
            info!(?provider, sleep_s = extra_backoff.as_secs(), "forecast scheduler backoff");
            tokio::time::sleep(extra_backoff).await;
        }
        match fetcher.fetch().await {
            Ok(totals) => {
                extra_backoff = Duration::ZERO;
                debug!(
                    ?provider,
                    today_kwh = totals.today_kwh,
                    tomorrow_kwh = totals.tomorrow_kwh,
                    "forecast fetched"
                );
                if tx
                    .send(Event::TypedSensor(TypedReading::Forecast {
                        provider,
                        today_kwh: totals.today_kwh,
                        tomorrow_kwh: totals.tomorrow_kwh,
                        at: std::time::Instant::now(),
                    }))
                    .await
                    .is_err()
                {
                    info!(?provider, "runtime receiver closed; forecast scheduler exiting");
                    return Ok(());
                }
            }
            Err(e) => {
                // A-28: classify HTTP failures so we don't hammer a
                // rate-limited or auth-failed endpoint at full cadence.
                match e.downcast_ref::<FetchFailure>() {
                    Some(FetchFailure::AuthFailed(_)) => {
                        error!(
                            ?provider,
                            error = %e,
                            "forecast auth failed (401/403); disabling fetcher — check api_key"
                        );
                        return Ok(());
                    }
                    Some(FetchFailure::RateLimited(_)) => {
                        warn!(
                            ?provider,
                            error = %e,
                            backoff_s = RATE_LIMIT_BACKOFF.as_secs(),
                            "forecast rate-limited (429); long backoff before next attempt"
                        );
                        extra_backoff = RATE_LIMIT_BACKOFF;
                    }
                    Some(FetchFailure::ServerError(_)) => {
                        extra_backoff = if extra_backoff == Duration::ZERO {
                            SERVER_ERROR_BACKOFF_START
                        } else {
                            (extra_backoff * 2).min(SERVER_ERROR_BACKOFF_MAX)
                        };
                        warn!(
                            ?provider,
                            error = %e,
                            backoff_s = extra_backoff.as_secs(),
                            "forecast 5xx; exponential backoff"
                        );
                    }
                    Some(FetchFailure::Other(_)) | None => {
                        warn!(?provider, error = %e, "forecast fetch failed");
                    }
                }
            }
        }
    }
}

/// A single PV plane (tilt + azimuth + kWp). Reused by Forecast.Solar
/// and Open-Meteo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub tilt_deg: f64,
    pub azimuth_deg: f64,
    pub kwp: f64,
}

/// Helper: extract a number from `serde_json::Value`, coercing
/// common shapes.
///
/// A-48: NaN / ±Inf and out-of-range strings are rejected. Rust's
/// `"NaN".parse::<f64>()` / `"inf".parse::<f64>()` happily succeed,
/// so a provider leaking those strings would previously feed non-
/// finite values into forecast totals and thence into weather_soc.
pub(crate) fn as_f64(v: &serde_json::Value) -> Option<f64> {
    let raw = match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    };
    raw.filter(|f| f.is_finite())
}

/// Classification of forecast-fetch failures. The scheduler uses this
/// to decide whether to retry at the normal cadence, back off
/// exponentially, or abandon the fetcher entirely (A-28).
#[derive(Debug)]
pub enum FetchFailure {
    /// 401 / 403 — credentials are wrong/expired. No amount of
    /// retrying helps; scheduler should shut down this fetcher.
    AuthFailed(anyhow::Error),
    /// 429 Too Many Requests — we've exceeded the per-day quota on
    /// a rate-limited endpoint. Wait a long time before retrying.
    RateLimited(anyhow::Error),
    /// 5xx — transient server error. Normal backoff.
    ServerError(anyhow::Error),
    /// Any other error — network timeout, parse error, etc.
    Other(anyhow::Error),
}

impl std::fmt::Display for FetchFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchFailure::AuthFailed(e) => write!(f, "AuthFailed: {e:#}"),
            FetchFailure::RateLimited(e) => write!(f, "RateLimited: {e:#}"),
            FetchFailure::ServerError(e) => write!(f, "ServerError: {e:#}"),
            FetchFailure::Other(e) => write!(f, "Other: {e:#}"),
        }
    }
}

impl std::error::Error for FetchFailure {}

/// Helper used by all three providers: tag a fetch error with the
/// caller's context for clearer logs.
pub(crate) async fn fetch_json(
    http: &HttpClient,
    url: &str,
    query: &[(&str, &str)],
) -> Result<serde_json::Value> {
    let resp = http
        .get(url)
        .query(query)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    let status = resp.status();
    let body = resp.text().await.with_context(|| format!("body {url}"))?;
    if !status.is_success() {
        let ctx = anyhow::anyhow!("{url} returned {status}: {body}");
        let code = status.as_u16();
        let failure = if code == 401 || code == 403 {
            FetchFailure::AuthFailed(ctx)
        } else if code == 429 {
            FetchFailure::RateLimited(ctx)
        } else if status.is_server_error() {
            FetchFailure::ServerError(ctx)
        } else {
            FetchFailure::Other(ctx)
        };
        return Err(anyhow::Error::from(failure));
    }
    serde_json::from_str(&body).with_context(|| format!("parse {url}"))
}
