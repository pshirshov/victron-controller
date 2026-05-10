//! Solcast Rooftop API client.
//!
//! Free tier: API key + up to 2 rooftop site IDs, 10 calls/day/site.
//! Endpoint: `GET https://api.solcast.com.au/rooftop_sites/{site}/forecasts?format=json`
//!
//! Response shape (abridged):
//! ```json
//! { "forecasts": [
//!     {"pv_estimate": 0.345, "period_end": "2026-04-22T12:30:00.0000000Z",
//!      "period": "PT30M"},
//!     ...
//! ]}
//! ```
//!
//! Each entry is a forecasted average power (kW) over a half-hour
//! period. We sum over the current and next calendar day, multiplied
//! by the period length (0.5 h), to get daily kWh totals. Sites are
//! summed when multiple are configured.

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use reqwest::Client as HttpClient;
use serde_json::Value;

use victron_controller_core::types::ForecastProvider;

use super::{as_f64, fetch_json, ForecastFetcher, ForecastTotals};

#[derive(Debug, Clone)]
pub struct SolcastClient {
    http: HttpClient,
    api_key: String,
    site_ids: Vec<String>,
    /// Site timezone — NOT the machine TZ. See A-50.
    tz: Tz,
}

impl SolcastClient {
    #[must_use]
    pub fn new(http: HttpClient, api_key: String, site_ids: Vec<String>, tz: Tz) -> Self {
        Self {
            http,
            api_key,
            site_ids,
            tz,
        }
    }

    #[must_use]
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty() && !self.site_ids.is_empty()
    }
}

#[async_trait]
impl ForecastFetcher for SolcastClient {
    fn provider(&self) -> ForecastProvider {
        ForecastProvider::Solcast
    }

    async fn fetch(&self) -> Result<ForecastTotals> {
        let mut totals_today = 0.0;
        let mut totals_tomorrow = 0.0;
        let now = Utc::now().with_timezone(&self.tz);
        let today = now.date_naive();
        let tomorrow = today.succ_opt().context("today.succ_opt")?;

        // A-26: distinguish a truly zero forecast from schema drift.
        // We count total and parseable items across all sites. If the
        // API returned items but we couldn't parse ANY of them, the
        // schema has changed and we must not silently report 0 kWh —
        // that would trigger battery-saver behaviour on a sunny day.
        let mut total_items = 0usize;
        let mut parsed_items = 0usize;

        // PR-soc-chart-solar: per-hour kWh, length 48 starting at local
        // midnight today (hours 0..24 = today, 24..48 = tomorrow).
        // Solcast typically delivers PT30M periods → two consecutive
        // half-hour buckets sum into one hourly entry.
        let mut hourly_kwh: Vec<f64> = vec![0.0; 48];
        let mut saw_any_hourly = false;

        for site in &self.site_ids {
            let url = format!("https://api.solcast.com.au/rooftop_sites/{site}/forecasts");
            let body = fetch_json(
                &self.http,
                &url,
                &[("format", "json"), ("api_key", &self.api_key)],
            )
            .await?;

            let Some(items) = body.get("forecasts").and_then(|v| v.as_array()) else {
                continue;
            };
            total_items += items.len();
            for item in items {
                let Some(kwh_contrib) = item_to_kwh(item) else {
                    continue;
                };
                let Some((day, hour)) = item_local_day_hour(item, self.tz) else {
                    continue;
                };
                parsed_items += 1;
                if day == today {
                    totals_today += kwh_contrib;
                    if hour < 24 {
                        hourly_kwh[hour] += kwh_contrib;
                        saw_any_hourly = true;
                    }
                } else if day == tomorrow {
                    totals_tomorrow += kwh_contrib;
                    if hour < 24 {
                        hourly_kwh[24 + hour] += kwh_contrib;
                        saw_any_hourly = true;
                    }
                }
            }
        }

        if total_items == 0 {
            anyhow::bail!(
                "Solcast response had no forecast items across {} site(s); \
                 treating as fetch failure (A-26)",
                self.site_ids.len()
            );
        }
        if parsed_items == 0 {
            anyhow::bail!(
                "Solcast returned {total_items} forecast items but none \
                 parsed (schema drift?); refusing to emit 0 kWh (A-26)"
            );
        }

        let final_hourly = if saw_any_hourly {
            hourly_kwh
        } else {
            tracing::debug!("solcast: no hourly entries parsed; emitting empty hourly_kwh");
            Vec::new()
        };

        Ok(ForecastTotals {
            today_kwh: totals_today,
            tomorrow_kwh: totals_tomorrow,
            hourly_kwh: final_hourly,
            // Solcast doesn't supply temperature; planner treats empty as
            // "no forecast temperature available" and consults the
            // sensor.
            hourly_temperature_c: Vec::new(),
        })
    }
}

/// Convert one forecast entry to the kWh it contributes to a daily
/// total — `pv_estimate (kW) × period (h)`. Periods are ISO-8601
/// durations like `PT30M`; we only handle common cases (PT30M, PT60M,
/// PT15M, PT5M).
fn item_to_kwh(item: &Value) -> Option<f64> {
    let kw = item.get("pv_estimate").and_then(as_f64)?;
    let period = item.get("period").and_then(|v| v.as_str()).unwrap_or("PT30M");
    let hours = period_to_hours(period)?;
    Some(kw * hours)
}

fn period_to_hours(s: &str) -> Option<f64> {
    let rest = s.strip_prefix("PT")?;
    if let Some(m) = rest.strip_suffix('M') {
        m.parse::<f64>().ok().map(|m| m / 60.0)
    } else if let Some(h) = rest.strip_suffix('H') {
        h.parse::<f64>().ok()
    } else {
        None
    }
}

/// `period_end` is a UTC timestamp marking the END of the period.
/// Using it directly misattributes boundary periods — the 23:30–00:00
/// bucket's `period_end` is 00:00 of the next day, so 30 min of
/// Monday's production gets bucketed into Tuesday. A-27: use the
/// period's midpoint (= `period_end - period/2`) for day attribution.
/// A-50: project the midpoint into the configured site TZ, not the
/// machine's `Local` (Venus runs UTC).
///
/// PR-soc-chart-solar: returns the local-clock `hour` 0..23 alongside
/// the local date so the caller can bucket per-period kWh into hourly
/// slots for the SoC-chart projection.
fn item_local_day_hour(item: &Value, tz: Tz) -> Option<(NaiveDate, usize)> {
    use chrono::Timelike;
    let s = item.get("period_end").and_then(|v| v.as_str())?;
    let utc: DateTime<Utc> = s.parse().ok()?;
    let period_str = item.get("period").and_then(|v| v.as_str()).unwrap_or("PT30M");
    let period_hours = period_to_hours(period_str).unwrap_or(0.5);
    let half = chrono::Duration::milliseconds((period_hours * 1_800_000.0) as i64);
    let midpoint_utc = utc.checked_sub_signed(half)?;
    let local = midpoint_utc.with_timezone(&tz);
    let hour = local.hour() as usize;
    Some((local.date_naive(), hour))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn period_to_hours_handles_common_shapes() {
        assert!((period_to_hours("PT30M").unwrap() - 0.5).abs() < 1e-9);
        assert!((period_to_hours("PT15M").unwrap() - 0.25).abs() < 1e-9);
        assert!((period_to_hours("PT60M").unwrap() - 1.0).abs() < 1e-9);
        assert!((period_to_hours("PT1H").unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn period_to_hours_none_on_garbage() {
        assert!(period_to_hours("garbage").is_none());
        assert!(period_to_hours("PT").is_none());
    }

    #[test]
    fn item_to_kwh_multiplies_kw_by_period_hours() {
        let item = json!({"pv_estimate": 2.0, "period_end": "2026-04-22T12:30:00Z", "period": "PT30M"});
        assert!((item_to_kwh(&item).unwrap() - 1.0).abs() < 1e-9); // 2 kW × 0.5 h
    }

    #[test]
    fn is_configured_requires_both_key_and_sites() {
        let http = http_client_blank();
        let tz = chrono_tz::Europe::London;
        assert!(!SolcastClient::new(http.clone(), String::new(), vec![], tz).is_configured());
        assert!(!SolcastClient::new(http.clone(), "key".into(), vec![], tz).is_configured());
        assert!(!SolcastClient::new(http.clone(), String::new(), vec!["s".into()], tz).is_configured());
        assert!(SolcastClient::new(http, "key".into(), vec!["s".into()], tz).is_configured());
    }

    fn http_client_blank() -> HttpClient {
        super::super::http_client()
    }
}
