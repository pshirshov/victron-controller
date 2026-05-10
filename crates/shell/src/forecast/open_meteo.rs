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
use chrono::Utc;
use chrono_tz::Tz;
use reqwest::Client as HttpClient;

use victron_controller_core::types::ForecastProvider;

use super::forecast_solar::forecast_solar_azimuth_pub;
use super::{fetch_json, ForecastFetcher, ForecastTotals, Plane};

/// Default panel + inverter + BOS efficiency when the user doesn't
/// set one in config. A-43 made this configurable; 0.75 preserves
/// pre-PR behavior for users who leave `[forecast.open_meteo]
/// system_efficiency` unset.
pub const DEFAULT_SYSTEM_EFFICIENCY: f64 = 0.75;

#[derive(Debug, Clone)]
pub struct OpenMeteoClient {
    http: HttpClient,
    latitude: f64,
    longitude: f64,
    planes: Vec<Plane>,
    system_efficiency: f64,
    /// Site timezone. A-50: pinned both in the `timezone=` query
    /// parameter (so Open-Meteo returns site-local timestamps) AND
    /// used for our today/tomorrow bucketing. Previously we sent
    /// `timezone=auto` and compared against `Local::now()` — on a
    /// Venus with TZ=UTC the two disagree by the site's UTC offset.
    tz: Tz,
}

impl OpenMeteoClient {
    #[must_use]
    pub fn new(
        http: HttpClient,
        latitude: f64,
        longitude: f64,
        planes: Vec<Plane>,
        system_efficiency: f64,
        tz: Tz,
    ) -> Self {
        // Defensive clamp — config parsing accepts any f64; out-of-range
        // values would silently skew weather_soc if we didn't clamp.
        let system_efficiency = system_efficiency.clamp(0.1, 1.0);
        Self {
            http,
            latitude,
            longitude,
            planes,
            system_efficiency,
            tz,
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
        let today = Utc::now().with_timezone(&self.tz).date_naive();
        let tomorrow = today.succ_opt().context("today.succ_opt")?;

        let mut totals_today_kwh = 0.0;
        let mut totals_tomorrow_kwh = 0.0;
        // PR-soc-chart-solar: per-hour kWh, length 48 starting at local
        // midnight today (hours 0..24 = today, 24..48 = tomorrow).
        let mut hourly_kwh: Vec<f64> = vec![0.0; 48];
        let mut saw_any_hourly = false;
        // Per-hour temperature (°C), same indexing convention as
        // `hourly_kwh`. Sentinel `NaN` marks "not yet populated"; we
        // fold the first non-NaN reading per (hour, plane-loop) iteration
        // — the temperature field is plane-independent, so subsequent
        // planes overwrite with the same value (idempotent). After the
        // loop, NaN slots collapse the vector to empty if none were set.
        let mut hourly_temperature_c: Vec<f64> = vec![f64::NAN; 48];
        let mut saw_any_temperature = false;

        let url = "https://api.open-meteo.com/v1/forecast";
        let tz_name = self.tz.name();
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
                    // Comma-separated list of hourly variables. Adding
                    // `temperature_2m` so the WeatherSoc planner can
                    // average forecast temperature across daylight hours
                    // instead of consulting the instantaneous sensor.
                    ("hourly", "global_tilted_irradiance,temperature_2m"),
                    ("tilt", &tilt),
                    ("azimuth", &az),
                    // A-50: pin site TZ explicitly (reqwest URL-encodes
                    // the slash) so the API and our bucketing agree.
                    ("timezone", tz_name),
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
            // `temperature_2m` is plane-independent (same value across
            // planes for any given hour), but Open-Meteo returns it in
            // every per-plane response since we ask for it in the
            // `hourly=` list. The fold below overwrites previous
            // populates with the same value — idempotent — so we don't
            // care which plane's response wins.
            let temps: Option<&Vec<serde_json::Value>> = body
                .pointer("/hourly/temperature_2m")
                .and_then(|v| v.as_array());

            // Sum hourly irradiance (W/m²) × 1 h × efficiency × kWp/1000
            // into today/tomorrow buckets, using the time string's date.
            // Also accumulate per-hour into `hourly_kwh` indexed by
            // local-clock hour (today = 0..24, tomorrow = 24..48).
            let today_str = today.format("%Y-%m-%d").to_string();
            let tomorrow_str = tomorrow.format("%Y-%m-%d").to_string();
            for (i, (t, w)) in times.iter().zip(irrad.iter()).enumerate() {
                let Some(t_str) = t.as_str() else { continue };
                let Some(w_f) = w.as_f64() else { continue };
                // Open-Meteo format: "2026-04-22T13:00"
                let Some(date_part) = t_str.get(..10) else {
                    continue;
                };
                let kwh_contrib = (w_f / 1000.0) * self.system_efficiency * plane.kwp;
                let hour: Option<usize> = t_str
                    .get(11..13)
                    .and_then(|h| h.parse::<usize>().ok())
                    .filter(|h| *h < 24);
                let temp_c: Option<f64> = temps
                    .and_then(|arr| arr.get(i))
                    .and_then(serde_json::Value::as_f64);
                if date_part == today_str {
                    totals_today_kwh += kwh_contrib;
                    if let Some(h) = hour {
                        hourly_kwh[h] += kwh_contrib;
                        saw_any_hourly = true;
                        if let Some(c) = temp_c {
                            hourly_temperature_c[h] = c;
                            saw_any_temperature = true;
                        }
                    }
                } else if date_part == tomorrow_str {
                    totals_tomorrow_kwh += kwh_contrib;
                    if let Some(h) = hour {
                        hourly_kwh[24 + h] += kwh_contrib;
                        saw_any_hourly = true;
                        if let Some(c) = temp_c {
                            hourly_temperature_c[24 + h] = c;
                            saw_any_temperature = true;
                        }
                    }
                }
            }
        }

        let final_hourly = if saw_any_hourly {
            hourly_kwh
        } else {
            tracing::debug!(
                "open_meteo: no hourly entries parsed; emitting empty hourly_kwh"
            );
            Vec::new()
        };
        // Collapse to empty when no temperature samples landed at all
        // (provider partial response, schema drift, etc.). The
        // downstream WeatherSoc planner treats an empty vector as "no
        // forecast temperature available" and falls back to the local
        // sensor — same shape as the kwh fallback above.
        let final_hourly_temp = if saw_any_temperature {
            hourly_temperature_c
        } else {
            tracing::debug!(
                "open_meteo: no temperature entries parsed; emitting empty hourly_temperature_c"
            );
            Vec::new()
        };

        Ok(ForecastTotals {
            today_kwh: totals_today_kwh,
            tomorrow_kwh: totals_tomorrow_kwh,
            hourly_kwh: final_hourly,
            hourly_temperature_c: final_hourly_temp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_configured_without_planes() {
        let c = OpenMeteoClient::new(
            super::super::http_client(),
            50.0,
            0.0,
            vec![],
            DEFAULT_SYSTEM_EFFICIENCY,
            chrono_tz::Europe::London,
        );
        assert!(!c.is_configured());
    }
}
