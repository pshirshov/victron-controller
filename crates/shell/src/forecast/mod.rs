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
use tracing::{debug, info, warn};

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
    loop {
        ticker.tick().await;
        match fetcher.fetch().await {
            Ok(totals) => {
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
            Err(e) => warn!(?provider, error = %e, "forecast fetch failed"),
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
pub(crate) fn as_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

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
        return Err(anyhow::anyhow!("{url} returned {status}: {body}"));
    }
    serde_json::from_str(&body).with_context(|| format!("parse {url}"))
}
