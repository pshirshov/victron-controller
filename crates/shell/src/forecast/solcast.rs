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
use chrono::{DateTime, Local, NaiveDate, Utc};
use reqwest::Client as HttpClient;
use serde_json::Value;

use victron_controller_core::types::ForecastProvider;

use super::{as_f64, fetch_json, ForecastFetcher, ForecastTotals};

#[derive(Debug, Clone)]
pub struct SolcastClient {
    http: HttpClient,
    api_key: String,
    site_ids: Vec<String>,
}

impl SolcastClient {
    #[must_use]
    pub fn new(http: HttpClient, api_key: String, site_ids: Vec<String>) -> Self {
        Self {
            http,
            api_key,
            site_ids,
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
        let now = Local::now();
        let today = now.date_naive();
        let tomorrow = today.succ_opt().context("today.succ_opt")?;

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
            for item in items {
                let Some(kwh_contrib) = item_to_kwh(item) else {
                    continue;
                };
                let day = match item_day_local(item) {
                    Some(d) => d,
                    None => continue,
                };
                if day == today {
                    totals_today += kwh_contrib;
                } else if day == tomorrow {
                    totals_tomorrow += kwh_contrib;
                }
            }
        }

        Ok(ForecastTotals {
            today_kwh: totals_today,
            tomorrow_kwh: totals_tomorrow,
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

/// `period_end` is a UTC timestamp marking the end of the period.
/// For a calendar-local bucket we convert to Local and take the date.
fn item_day_local(item: &Value) -> Option<NaiveDate> {
    let s = item.get("period_end").and_then(|v| v.as_str())?;
    let utc: DateTime<Utc> = s.parse().ok()?;
    let local = utc.with_timezone(&Local);
    Some(local.date_naive())
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
        assert!(!SolcastClient::new(http.clone(), String::new(), vec![]).is_configured());
        assert!(!SolcastClient::new(http.clone(), "key".into(), vec![]).is_configured());
        assert!(!SolcastClient::new(http.clone(), String::new(), vec!["s".into()]).is_configured());
        assert!(SolcastClient::new(http, "key".into(), vec!["s".into()]).is_configured());
    }

    fn http_client_blank() -> HttpClient {
        super::super::http_client()
    }
}
