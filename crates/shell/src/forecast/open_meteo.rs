//! Open-Meteo client.
//!
//! Free, no key. We use the "forecast" endpoint with `minutely_15` /
//! `hourly` resolution, requesting `global_tilted_irradiance` per
//! plane so we can multiply by the plane's kWp to estimate AC output.
//!
//! Open-Meteo doesn't have a "site" concept: we loop over the
//! user-configured planes and sum. For each plane:
//!
//! ```text
//! GET https://api.open-meteo.com/v1/forecast
//!     ?latitude=LAT&longitude=LON
//!     &hourly=global_tilted_irradiance
//!     &tilt=TILT&azimuth=AZ
//!     &timezone=auto
//! ```
//!
//! Open-Meteo uses an azimuth convention of S=0, E=-90, W=+90 (same
//! as Forecast.Solar), so we reuse our compass → FS conversion.
//!
//! AC output = irradiance (W/m²) × assumed efficiency × kWp / nominal
//! irradiance (1000 W/m²). We use an effective-area coefficient that
//! captures panel efficiency + inverter efficiency (default 0.75).

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Local;
use reqwest::Client as HttpClient;

use victron_controller_core::types::ForecastProvider;

use super::forecast_solar::forecast_solar_azimuth_pub;
use super::{fetch_json, ForecastFetcher, ForecastTotals, Plane};

/// Combined panel + inverter + BOS efficiency. Used to convert plane
/// irradiance (kW/m²) × kWp → AC kW output.
const SYSTEM_EFFICIENCY: f64 = 0.75;

#[derive(Debug, Clone)]
pub struct OpenMeteoClient {
    http: HttpClient,
    latitude: f64,
    longitude: f64,
    planes: Vec<Plane>,
}

impl OpenMeteoClient {
    #[must_use]
    pub fn new(http: HttpClient, latitude: f64, longitude: f64, planes: Vec<Plane>) -> Self {
        Self {
            http,
            latitude,
            longitude,
            planes,
        }
    }

    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.planes.is_empty()
    }
}

#[async_trait]
impl ForecastFetcher for OpenMeteoClient {
    fn provider(&self) -> ForecastProvider {
        ForecastProvider::OpenMeteo
    }

    async fn fetch(&self) -> Result<ForecastTotals> {
        let today = Local::now().date_naive();
        let tomorrow = today.succ_opt().context("today.succ_opt")?;

        let mut totals_today_kwh = 0.0;
        let mut totals_tomorrow_kwh = 0.0;

        let url = "https://api.open-meteo.com/v1/forecast";
        for plane in &self.planes {
            let tilt = format!("{}", plane.tilt_deg);
            let az = format!("{}", forecast_solar_azimuth_pub(plane.azimuth_deg));
            let lat = format!("{}", self.latitude);
            let lon = format!("{}", self.longitude);
            let body = fetch_json(
                &self.http,
                url,
                &[
                    ("latitude", &lat),
                    ("longitude", &lon),
                    ("hourly", "global_tilted_irradiance"),
                    ("tilt", &tilt),
                    ("azimuth", &az),
                    ("timezone", "auto"),
                    ("forecast_days", "2"),
                ],
            )
            .await?;

            let Some(times) = body.pointer("/hourly/time").and_then(|v| v.as_array()) else {
                continue;
            };
            let Some(irrad) = body
                .pointer("/hourly/global_tilted_irradiance")
                .and_then(|v| v.as_array())
            else {
                continue;
            };

            // Sum hourly irradiance (W/m²) × 1 h × efficiency × kWp/1000
            // into today/tomorrow buckets, using the time string's date.
            for (t, w) in times.iter().zip(irrad.iter()) {
                let Some(t_str) = t.as_str() else { continue };
                let Some(w_f) = w.as_f64() else { continue };
                // Open-Meteo format: "2026-04-22T13:00"
                let Some(date_part) = t_str.get(..10) else {
                    continue;
                };
                let kwh_contrib = (w_f / 1000.0) * SYSTEM_EFFICIENCY * plane.kwp;
                if date_part == today.format("%Y-%m-%d").to_string() {
                    totals_today_kwh += kwh_contrib;
                } else if date_part == tomorrow.format("%Y-%m-%d").to_string() {
                    totals_tomorrow_kwh += kwh_contrib;
                }
            }
        }

        Ok(ForecastTotals {
            today_kwh: totals_today_kwh,
            tomorrow_kwh: totals_tomorrow_kwh,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_configured_without_planes() {
        let c = OpenMeteoClient::new(super::super::http_client(), 50.0, 0.0, vec![]);
        assert!(!c.is_configured());
    }
}
