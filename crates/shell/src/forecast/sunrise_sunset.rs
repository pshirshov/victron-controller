//! PR-keep-batteries-charged. Always-on sunrise/sunset scheduler,
//! decoupled from `[forecast.baseline]`.
//!
//! Spawns when `[location]` is configured. Emits `Event::SunriseSunset`
//! on its cadence; consumed by `core::process::apply_event` which
//! writes `world.sunrise` / `world.sunset` /
//! `world.sunrise_sunset_updated_at`.
//!
//! `[forecast.baseline]` no longer emits `Event::SunriseSunset` — this
//! module is the single producer. Baseline still computes sunrise /
//! sunset *internally* for its hourly-Wh accounting using its own
//! coordinates; that path is independent.

use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use sunrise::{Coordinates, SolarDay, SolarEvent};
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, info, warn};

use victron_controller_core::types::Event;

#[derive(Debug, Clone, Copy)]
pub struct SunriseSunsetParams {
    pub latitude: f64,
    pub longitude: f64,
    pub cadence: Duration,
    pub tz: Tz,
}

/// Always-on scheduler: emits `Event::SunriseSunset` every `cadence`
/// for as long as the runtime receiver is alive. Coordinate sanity is
/// the sunrise crate's job — invalid inputs cause a single warn at
/// startup and a clean exit.
pub async fn run_sunrise_sunset_scheduler(
    params: SunriseSunsetParams,
    tx: mpsc::Sender<Event>,
) -> Result<()> {
    let SunriseSunsetParams { latitude, longitude, cadence, tz } = params;
    let coord = match Coordinates::new(latitude, longitude) {
        Some(c) => c,
        None => {
            warn!(
                latitude,
                longitude,
                "sunrise/sunset: coordinates rejected by sunrise crate; scheduler not started"
            );
            return Ok(());
        }
    };
    info!(
        latitude,
        longitude,
        cadence_s = cadence.as_secs(),
        tz = tz.name(),
        "sunrise/sunset scheduler started"
    );
    let mut ticker = interval(cadence);
    loop {
        ticker.tick().await;

        let now_local = Utc::now().with_timezone(&tz);
        let today = now_local.date_naive();

        let day = SolarDay::new(coord, today);
        let sunrise_utc: Option<DateTime<Utc>> = day.event_time(SolarEvent::Sunrise);
        let sunset_utc: Option<DateTime<Utc>> = day.event_time(SolarEvent::Sunset);

        match (sunrise_utc, sunset_utc) {
            (Some(sr), Some(ss)) => {
                let sunrise_local = sr.with_timezone(&tz).naive_local();
                let sunset_local = ss.with_timezone(&tz).naive_local();
                if tx
                    .send(Event::SunriseSunset {
                        sunrise: sunrise_local,
                        sunset: sunset_local,
                        at: Instant::now(),
                    })
                    .await
                    .is_err()
                {
                    info!("runtime receiver closed; sunrise/sunset scheduler exiting");
                    return Ok(());
                }
            }
            _ => {
                // Polar day / polar night. Skip emission rather than
                // fabricating a value — `world.sunrise_sunset_updated_at`
                // ages out over `SUNRISE_SUNSET_FRESHNESS` and consumers
                // (incl. the ESS-state override) bias-to-safety.
                debug!(?today, "sunrise/sunset: no sunrise/sunset (polar day)");
            }
        }
    }
}
