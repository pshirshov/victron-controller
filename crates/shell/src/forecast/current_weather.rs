//! Current-weather poller: fetches outdoor temperature from Open-Meteo
//! and emits it as a `SensorReading(OutdoorTemperature)` event.
//!
//! Placeholder source while the MQTT weather-sensor binding is being
//! sorted out (SPEC §10.2). Uses the free Open-Meteo
//! `current=temperature_2m` endpoint — no API key, no rate limit at
//! sensible cadences.
//!
//! Runs independently from the forecast scheduler so temperature
//! updates arrive on their own cadence (default: 15 min).

use std::time::{Duration, Instant};

use anyhow::Result;
use chrono_tz::Tz;
use reqwest::Client as HttpClient;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info, warn};

use victron_controller_core::types::{Event, SensorId, SensorReading};

use super::fetch_json;

pub async fn run_open_meteo_temperature(
    http: HttpClient,
    latitude: f64,
    longitude: f64,
    cadence: Duration,
    tz: Tz,
    tx: mpsc::Sender<Event>,
) -> Result<()> {
    info!(
        latitude,
        longitude,
        cadence_s = cadence.as_secs(),
        tz = tz.name(),
        "open-meteo current-temperature poller started"
    );
    let url = "https://api.open-meteo.com/v1/forecast";
    let lat = format!("{latitude}");
    let lon = format!("{longitude}");
    let tz_name = tz.name();
    let mut ticker = interval(cadence);
    loop {
        ticker.tick().await;
        let query = [
            ("latitude", lat.as_str()),
            ("longitude", lon.as_str()),
            ("current", "temperature_2m"),
            // A-50: match the configured site TZ (don't leak machine TZ
            // into the forecast pipeline).
            ("timezone", tz_name),
        ];
        match fetch_json(&http, url, &query).await {
            Ok(body) => {
                let Some(t) = body
                    .pointer("/current/temperature_2m")
                    .and_then(serde_json::Value::as_f64)
                else {
                    warn!("open-meteo response missing /current/temperature_2m");
                    continue;
                };
                debug!(temperature_c = t, "open-meteo outdoor temperature fetched");
                if tx
                    .send(Event::Sensor(SensorReading {
                        id: SensorId::OutdoorTemperature,
                        value: t,
                        at: Instant::now(),
                    }))
                    .await
                    .is_err()
                {
                    info!("runtime receiver closed; temperature poller exiting");
                    return Ok(());
                }
            }
            Err(e) => warn!(error = %e, "open-meteo current-weather fetch failed"),
        }
    }
}
