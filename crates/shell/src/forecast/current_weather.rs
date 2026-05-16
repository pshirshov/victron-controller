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
//!
//! PR2: also requests `hourly=cloud_cover` over the next two days and
//! emits a `TypedReading::WeatherCloudForecast` so the baseline
//! forecaster has cloud data even when the solar-forecast providers
//! are stale or disabled. One endpoint, one poll, two events.

use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Utc;
use chrono_tz::Tz;
use reqwest::Client as HttpClient;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info, warn};

use victron_controller_core::types::{
    Event, SensorId, SensorReading, TimerId, TimerStatus, TypedReading,
};

use super::{epoch_ms_now, fetch_json};

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
            // PR2: piggyback hourly cloud cover for the baseline
            // forecaster. Two days = 48 hours starting at local
            // midnight today, matching ForecastSnapshot's convention.
            ("hourly", "cloud_cover"),
            ("forecast_days", "2"),
            // A-50: match the configured site TZ (don't leak machine TZ
            // into the forecast pipeline).
            ("timezone", tz_name),
        ];
        // PR-timers-section: track the per-fire status so a TimerState
        // can be emitted after every cycle (success or error).
        let mut timer_status = TimerStatus::Idle;
        match fetch_json(&http, url, &query).await {
            Ok(body) => {
                let Some(t) = body
                    .pointer("/current/temperature_2m")
                    .and_then(serde_json::Value::as_f64)
                else {
                    warn!("open-meteo response missing /current/temperature_2m");
                    timer_status = TimerStatus::FailedLastRun;
                    if !emit_timer_state(&tx, cadence, timer_status).await {
                        return Ok(());
                    }
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

                // PR2: parse hourly cloud_cover into a 48-element array
                // indexed by local clock hour (today 0..24, tomorrow
                // 24..48). Missing entries stay NaN; if no rows landed
                // at all we skip the event rather than emit a noisy
                // all-NaN array. Schema/quota drift in the response is
                // tolerated — temperature still emitted regardless.
                if let Some(arr) = parse_hourly_clouds(&body, tz) {
                    debug!(non_nan = arr.iter().filter(|v| v.is_finite()).count(), "open-meteo cloud forecast fetched");
                    if tx
                        .send(Event::TypedSensor(TypedReading::WeatherCloudForecast {
                            hourly_cover_pct: arr,
                            at: Instant::now(),
                        }))
                        .await
                        .is_err()
                    {
                        info!("runtime receiver closed; temperature poller exiting");
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "open-meteo current-weather fetch failed");
                timer_status = TimerStatus::FailedLastRun;
            }
        }
        if !emit_timer_state(&tx, cadence, timer_status).await {
            return Ok(());
        }
    }
}

/// Parse the `hourly.time` + `hourly.cloud_cover` arrays into a
/// length-48 vector indexed by local clock hour (today 0..24, tomorrow
/// 24..48). Returns `None` when the response shape is missing both
/// arrays so the caller can skip emitting an empty event. Slots that
/// don't appear in the response remain `NaN`; the consumer
/// (`baseline.rs`) treats those as "no signal, don't modulate" per
/// hour.
fn parse_hourly_clouds(body: &serde_json::Value, tz: Tz) -> Option<Vec<f64>> {
    let times = body.pointer("/hourly/time").and_then(|v| v.as_array())?;
    let clouds = body
        .pointer("/hourly/cloud_cover")
        .and_then(|v| v.as_array())?;
    let today = Utc::now().with_timezone(&tz).date_naive();
    let tomorrow = today.succ_opt()?;
    let today_str = today.format("%Y-%m-%d").to_string();
    let tomorrow_str = tomorrow.format("%Y-%m-%d").to_string();
    let mut out = vec![f64::NAN; 48];
    let mut saw_any = false;
    for (t, c) in times.iter().zip(clouds.iter()) {
        let Some(t_str) = t.as_str() else { continue };
        let Some(c_f) = c.as_f64() else { continue };
        let Some(date_part) = t_str.get(..10) else {
            continue;
        };
        let Some(hour) = t_str
            .get(11..13)
            .and_then(|h| h.parse::<usize>().ok())
            .filter(|h| *h < 24)
        else {
            continue;
        };
        if date_part == today_str {
            out[hour] = c_f;
            saw_any = true;
        } else if date_part == tomorrow_str {
            out[24 + hour] = c_f;
            saw_any = true;
        }
    }
    if saw_any { Some(out) } else { None }
}

/// Emit one `Event::TimerState` for the OpenMeteoCurrent timer. Returns
/// `false` when the channel is closed (caller should exit). PR-timers-section.
async fn emit_timer_state(
    tx: &mpsc::Sender<Event>,
    cadence: Duration,
    status: TimerStatus,
) -> bool {
    let last = epoch_ms_now();
    let next = last + i64::try_from(cadence.as_millis()).unwrap_or(i64::MAX);
    tx.send(Event::TimerState {
        id: TimerId::OpenMeteoCurrent,
        last_fire_epoch_ms: last,
        next_fire_epoch_ms: Some(next),
        status,
        at: Instant::now(),
    })
    .await
    .is_ok()
}
