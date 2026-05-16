//! Forecast.Solar client.
//!
//! Free tier: no key needed, rate-limited ~12 req/h/IP.
//! Endpoint: `GET https://api.forecast.solar/estimate/{lat}/{lon}/{dec}/{az}/{kwp}`
//!
//! Response shape (abridged):
//! ```json
//! { "result": {
//!     "watts": { "2026-04-22 05:00:00": 0, "2026-04-22 12:00:00": 1234, ... },
//!     "watt_hours_day": { "2026-04-22": 15234, "2026-04-23": 18201 },
//!     "watt_hours_period": { ... }
//! }}
//! ```
//!
//! We read `watt_hours_day` directly (already totals per calendar day
//! in local time of the site). Sum over all configured planes.
//!
//! Azimuth convention: Forecast.Solar uses -180 (N) / -90 (E) / 0 (S)
//! / 90 (W) / 180 (N). We accept user planes in the common 0=N /
//! 180=S convention and convert on the way out.

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use chrono_tz::Tz;
use reqwest::Client as HttpClient;

use victron_controller_core::types::ForecastProvider;

use super::{as_f64, fetch_json, ForecastFetcher, ForecastTotals, Plane};

#[derive(Debug, Clone)]
pub struct ForecastSolarClient {
    http: HttpClient,
    latitude: f64,
    longitude: f64,
    planes: Vec<Plane>,
    /// Site timezone — `watt_hours_day` keys are site-local dates, so
    /// we must bucket against the site TZ, not the machine TZ (A-50).
    tz: Tz,
}

impl ForecastSolarClient {
    #[must_use]
    pub fn new(
        http: HttpClient,
        latitude: f64,
        longitude: f64,
        planes: Vec<Plane>,
        tz: Tz,
    ) -> Self {
        Self {
            http,
            latitude,
            longitude,
            planes,
            tz,
        }
    }

    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.planes.is_empty()
    }
}

#[async_trait]
impl ForecastFetcher for ForecastSolarClient {
    fn provider(&self) -> ForecastProvider {
        ForecastProvider::ForecastSolar
    }

    async fn fetch(&self) -> Result<ForecastTotals> {
        let today = Utc::now().with_timezone(&self.tz).date_naive();
        let tomorrow = today.succ_opt().context("today.succ_opt")?;

        let mut totals_today_wh = 0.0;
        let mut totals_tomorrow_wh = 0.0;
        // PR-soc-chart-solar: per-hour Wh accumulators, length 48 starting
        // at local midnight today (hours 0..24 = today, 24..48 = tomorrow).
        let mut hourly_wh: Vec<f64> = vec![0.0; 48];
        let mut saw_any_hourly = false;

        for plane in &self.planes {
            // dec = declination (tilt); az = forecast.solar azimuth;
            // their azimuth is signed with S=0, E=-90, W=+90.
            let dec = plane.tilt_deg.round() as i32;
            let az = forecast_solar_azimuth(plane.azimuth_deg).round() as i32;
            let kwp = plane.kwp;
            let url = format!(
                "https://api.forecast.solar/estimate/{lat}/{lon}/{dec}/{az}/{kwp}",
                lat = self.latitude,
                lon = self.longitude,
            );
            let body = fetch_json(&self.http, &url, &[]).await?;

            let today_key = today.format("%Y-%m-%d").to_string();
            let tomorrow_key = tomorrow.format("%Y-%m-%d").to_string();

            if let Some(day_map) = body
                .pointer("/result/watt_hours_day")
                .and_then(|v| v.as_object())
            {
                if let Some(wh) = day_map.get(&today_key).and_then(as_f64) {
                    totals_today_wh += wh;
                }
                if let Some(wh) = day_map.get(&tomorrow_key).and_then(as_f64) {
                    totals_tomorrow_wh += wh;
                }
            }

            // Per-hour accumulation. Forecast.Solar's `watt_hours_period`
            // keys are site-local timestamps "YYYY-MM-DD HH:MM:SS"
            // representing the END of a period; values are Wh produced
            // during that period. Bucket by the end timestamp's hour
            // (close enough for the chart at hour granularity).
            if let Some(period_map) = body
                .pointer("/result/watt_hours_period")
                .and_then(|v| v.as_object())
            {
                for (k, v) in period_map {
                    let Some(wh) = as_f64(v) else { continue };
                    // Expect "YYYY-MM-DD HH:MM:SS" — at minimum 13 chars.
                    let Some(date_part) = k.get(..10) else { continue };
                    let Some(hour) = k.get(11..13).and_then(|h| h.parse::<usize>().ok()) else {
                        continue;
                    };
                    if hour >= 24 {
                        continue;
                    }
                    if date_part == today_key {
                        hourly_wh[hour] += wh;
                        saw_any_hourly = true;
                    } else if date_part == tomorrow_key {
                        hourly_wh[24 + hour] += wh;
                        saw_any_hourly = true;
                    }
                }
            }
        }

        let final_hourly = if saw_any_hourly {
            hourly_wh.iter().map(|wh| wh / 1000.0).collect::<Vec<_>>()
        } else {
            tracing::debug!(
                "forecast_solar: no hourly entries parsed; emitting empty hourly_kwh"
            );
            Vec::new()
        };

        Ok(ForecastTotals {
            today_kwh: totals_today_wh / 1000.0,
            tomorrow_kwh: totals_tomorrow_wh / 1000.0,
            hourly_kwh: final_hourly,
            // Forecast.Solar doesn't supply temperature; planner falls
            // back to the sensor when this is empty.
            hourly_temperature_c: Vec::new(),
            // Forecast.Solar doesn't supply cloud cover.
            hourly_cloud_cover_pct: Vec::new(),
        })
    }
}

/// Publicly exported for open_meteo (same azimuth convention).
pub(crate) fn forecast_solar_azimuth_pub(compass_deg: f64) -> f64 {
    forecast_solar_azimuth(compass_deg)
}

/// Convert from the "compass bearing" azimuth (0=N, 90=E, 180=S, 270=W)
/// the user is likely to type in config into Forecast.Solar's
/// (S=0, E=-90, W=+90, N=±180) form.
fn forecast_solar_azimuth(compass_deg: f64) -> f64 {
    // Normalise to [-180, 180).
    let mut x = compass_deg % 360.0;
    if x < 0.0 {
        x += 360.0;
    }
    // Compass → FS: subtract 180, then wrap into [-180, 180).
    let mut out = x - 180.0;
    if out >= 180.0 {
        out -= 360.0;
    }
    if out < -180.0 {
        out += 360.0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azimuth_conversion_table() {
        assert!((forecast_solar_azimuth(180.0) - 0.0).abs() < 1e-9); // S -> 0
        assert!((forecast_solar_azimuth(90.0) - -90.0).abs() < 1e-9); // E -> -90
        assert!((forecast_solar_azimuth(270.0) - 90.0).abs() < 1e-9); // W -> 90
        assert!((forecast_solar_azimuth(0.0).abs() - 180.0).abs() < 1e-9); // N -> ±180
    }

    #[test]
    fn azimuth_handles_out_of_range() {
        // 360 wraps to 0 (N) → ±180
        assert!((forecast_solar_azimuth(360.0).abs() - 180.0).abs() < 1e-9);
        // Negative compass values
        assert!((forecast_solar_azimuth(-90.0) - 90.0).abs() < 1e-9); // -90 == 270 (W)
    }

    #[test]
    fn not_configured_without_planes() {
        let c = ForecastSolarClient::new(
            super::super::http_client(),
            50.0,
            0.0,
            vec![],
            chrono_tz::Europe::London,
        );
        assert!(!c.is_configured());
    }
}
