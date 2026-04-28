//! PR-baseline-forecast — locally-computed pessimistic forecast.
//!
//! Used as a last-resort fallback when every cloud provider is stale.
//! No HTTP, no API key — just sunrise/sunset from the `sunrise` crate
//! and a flat per-hour Wh constant during daylight, zero outside.
//!
//! The same scheduler that emits the forecast snapshot also emits
//! `Event::SunriseSunset` so the dashboard can surface today's daylight
//! window as two non-numeric "sensors". `core::world` stores them on
//! `world.sunrise` / `world.sunset` for the dashboard converter.
//!
//! Hour-bucketing notes:
//! - The 48-element hourly array starts at midnight LOCAL today and
//!   covers 24 today + 24 tomorrow, matching Forecast.Solar / Solcast.
//! - For each hour `h`, daylight overlap is computed against today's
//!   sunrise/sunset (hours 0..24) and tomorrow's (hours 24..48). The
//!   credit is `wh_per_hour × overlap_fraction`, so partial-overlap
//!   hours at sunrise / sunset get a fractional value.
//! - DST: we treat the 48-hour window as wall-clock midnight today to
//!   wall-clock midnight day-after-tomorrow. On DST-transition days the
//!   real local span is 23 or 25 hours; we live with the ±4 % drift on
//!   one day per ~6 months — this is a backup forecast, accuracy is
//!   secondary to never failing.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use sunrise::{Coordinates, SolarDay, SolarEvent};
use tokio::sync::{mpsc, Mutex};
use tokio::time::interval;
use tracing::{info, warn};

use victron_controller_core::types::{
    Event, ForecastProvider, TimerId, TimerStatus, TypedReading,
};
use victron_controller_core::World;

use super::epoch_ms_now;

/// Configuration for the baseline scheduler. Install-time only —
/// runtime-tunable values (winter range, per-hour Wh) live on
/// `World::knobs` and are read every cycle.
#[derive(Debug, Clone, Copy)]
pub struct BaselineParams {
    pub latitude: f64,
    pub longitude: f64,
    pub cadence: Duration,
    pub tz: Tz,
}

/// One snapshot of the four runtime knobs the scheduler steers on.
#[derive(Debug, Clone, Copy)]
struct BaselineKnobs {
    winter_start: (u32, u32),
    winter_end: (u32, u32),
    wh_per_hour_winter: f64,
    wh_per_hour_summer: f64,
}

/// Decode an `MMDD` u32 (e.g. 1101) into `(month, day)`. Returns
/// `None` for malformed values (out-of-range month, invalid day for
/// month — uses the leap year 2000 so 02-29 is accepted).
fn decode_mm_dd(mmdd: u32) -> Option<(u32, u32)> {
    let m = mmdd / 100;
    let d = mmdd % 100;
    if !(1..=12).contains(&m) {
        return None;
    }
    NaiveDate::from_ymd_opt(2000, m, d)?;
    Some((m, d))
}

/// Pull the four baseline knobs out of `world.knobs`, validate the
/// MM-DD encodings, and fall back to (Nov 1, Mar 1) on any parse
/// failure. The fallback is logged at Warn so an operator notices —
/// validation also happens at the MQTT/HA layer (`knob_range`), so a
/// failure here means a programmatic bypass set a bad value directly.
async fn read_baseline_knobs(world: &Arc<Mutex<World>>) -> BaselineKnobs {
    let k = {
        let w = world.lock().await;
        (
            w.knobs.baseline_winter_start_mm_dd,
            w.knobs.baseline_winter_end_mm_dd,
            w.knobs.baseline_wh_per_hour_winter,
            w.knobs.baseline_wh_per_hour_summer,
        )
    };
    let (raw_start, raw_end, wh_w, wh_s) = k;
    let winter_start = decode_mm_dd(raw_start).unwrap_or_else(|| {
        warn!(raw_start, "baseline: winter_start_mm_dd invalid; falling back to 11-01");
        (11, 1)
    });
    let winter_end = decode_mm_dd(raw_end).unwrap_or_else(|| {
        warn!(raw_end, "baseline: winter_end_mm_dd invalid; falling back to 03-01");
        (3, 1)
    });
    BaselineKnobs {
        winter_start,
        winter_end,
        wh_per_hour_winter: wh_w,
        wh_per_hour_summer: wh_s,
    }
}

/// Public entry point — spawned by `main` when
/// `cfg.forecast.baseline.is_configured()` returns true.
pub async fn run_baseline_scheduler(
    params: BaselineParams,
    world: Arc<Mutex<World>>,
    tx: mpsc::Sender<Event>,
) -> Result<()> {
    let BaselineParams { latitude, longitude, cadence, tz } = params;
    let coord = match Coordinates::new(latitude, longitude) {
        Some(c) => c,
        None => {
            warn!(
                latitude,
                longitude,
                "baseline forecast: coordinates rejected by sunrise crate; scheduler not started"
            );
            return Ok(());
        }
    };
    info!(
        latitude,
        longitude,
        cadence_s = cadence.as_secs(),
        tz = tz.name(),
        "baseline forecast scheduler started"
    );
    let mut ticker = interval(cadence);
    loop {
        ticker.tick().await;

        let now_local = Utc::now().with_timezone(&tz);
        let today = now_local.date_naive();
        let Some(tomorrow) = today.succ_opt() else {
            warn!(?today, "baseline: today.succ_opt() returned None; skipping");
            continue;
        };

        // Re-read the runtime knobs every cycle so dashboard/HA edits
        // take effect on the next tick (no scheduler restart needed).
        let bk = read_baseline_knobs(&world).await;

        let today_sr_ss = sunrise_sunset_local(coord, today, tz);
        let tomorrow_sr_ss = sunrise_sunset_local(coord, tomorrow, tz);

        // PR-keep-batteries-charged: SunriseSunset emission moved to
        // `forecast::sunrise_sunset::run_sunrise_sunset_scheduler` —
        // that scheduler is always-on when `[location]` is configured
        // and decoupled from this baseline-forecast feature. We still
        // *compute* sunrise/sunset locally here for the hourly-kWh
        // accounting below. Polar-day handling sits inline in
        // `build_hourly_kwh` so a polar bucket is silently zero.
        let _ = (today_sr_ss, today);

        // Pick season per-day so the winter/summer boundary lands on the
        // right calendar date even when today and tomorrow straddle it.
        let wh_today = wh_for_date(
            today,
            bk.winter_start,
            bk.winter_end,
            bk.wh_per_hour_winter,
            bk.wh_per_hour_summer,
        );
        let wh_tomorrow = wh_for_date(
            tomorrow,
            bk.winter_start,
            bk.winter_end,
            bk.wh_per_hour_winter,
            bk.wh_per_hour_summer,
        );

        let hourly_kwh = build_hourly_kwh(today_sr_ss, tomorrow_sr_ss, wh_today, wh_tomorrow, today, tz);
        let today_kwh: f64 = hourly_kwh.iter().take(24).sum();
        let tomorrow_kwh: f64 = hourly_kwh.iter().skip(24).sum();

        let send_result = tx
            .send(Event::TypedSensor(TypedReading::Forecast {
                provider: ForecastProvider::Baseline,
                today_kwh,
                tomorrow_kwh,
                hourly_kwh,
                at: Instant::now(),
            }))
            .await;
        if send_result.is_err() {
            info!("runtime receiver closed; baseline scheduler exiting");
            return Ok(());
        }

        // Per-fire timer state (mirrors `forecast::run_scheduler`).
        let last_fire_ms = epoch_ms_now();
        let interval_ms = i64::try_from(cadence.as_millis()).unwrap_or(i64::MAX);
        let next_fire_ms = last_fire_ms + interval_ms;
        if tx
            .send(Event::TimerState {
                id: TimerId::ForecastBaseline,
                last_fire_epoch_ms: last_fire_ms,
                next_fire_epoch_ms: Some(next_fire_ms),
                status: TimerStatus::Idle,
                at: Instant::now(),
            })
            .await
            .is_err()
        {
            return Ok(());
        }
    }
}

/// Today's sunrise / sunset in *local* time. `None` for polar days.
fn sunrise_sunset_local(
    coord: Coordinates,
    date: NaiveDate,
    tz: Tz,
) -> Option<(NaiveDateTime, NaiveDateTime)> {
    let day = SolarDay::new(coord, date);
    let sunrise_utc: DateTime<Utc> = day.event_time(SolarEvent::Sunrise)?;
    let sunset_utc: DateTime<Utc> = day.event_time(SolarEvent::Sunset)?;
    Some((
        sunrise_utc.with_timezone(&tz).naive_local(),
        sunset_utc.with_timezone(&tz).naive_local(),
    ))
}

/// Build the 48-element hourly kWh array. Fractional credit for hours
/// that only partially overlap the daylight window. Always returns 48
/// entries — polar days get all-zero arrays for the missing-sun side.
///
/// `wh_today` / `wh_tomorrow` are evaluated independently so a winter→
/// summer (or vice-versa) boundary that falls between today and tomorrow
/// uses the correct constant on each side.
fn build_hourly_kwh(
    today: Option<(NaiveDateTime, NaiveDateTime)>,
    tomorrow: Option<(NaiveDateTime, NaiveDateTime)>,
    wh_today: f64,
    wh_tomorrow: f64,
    today_date: NaiveDate,
    tz: Tz,
) -> Vec<f64> {
    let mut out = vec![0.0; 48];
    if wh_today <= 0.0 && wh_tomorrow <= 0.0 {
        return out;
    }
    // Local midnight at the start of today, in the configured TZ. We
    // anchor the 48-h window here. `from_local_datetime` can return
    // `None` / `Ambiguous` on DST transitions — pick the earliest valid
    // wall-clock interpretation, which keeps the array shape correct
    // even when the spring-forward/fall-back hour is exactly 00:00. On
    // the rare zones whose DST shift lands exactly at 00:00, fall
    // through to the all-zero default rather than fabricating values.
    let midnight_today_naive = today_date.and_hms_opt(0, 0, 0).expect("00:00:00 valid");
    if matches!(
        tz.from_local_datetime(&midnight_today_naive),
        chrono::LocalResult::None,
    ) {
        return out;
    }

    // Helper: compute fractional overlap in [0, 1] between a wall-clock
    // hour `[h_start, h_start + 1h)` and a daylight window
    // `[sunrise, sunset]`. All inputs are local NaiveDateTime; we compare
    // by signed seconds since the day's local midnight to keep DST out
    // of the inner loop.
    fn overlap_hours(
        hour_start_local_secs: i64,
        sunrise_local_secs: i64,
        sunset_local_secs: i64,
    ) -> f64 {
        let hour_end = hour_start_local_secs + 3600;
        let lo = hour_start_local_secs.max(sunrise_local_secs);
        let hi = hour_end.min(sunset_local_secs);
        let overlap = (hi - lo).max(0);
        f64::from(i32::try_from(overlap).unwrap_or(0)) / 3600.0
    }

    let local_secs_of = |dt: NaiveDateTime, anchor: NaiveDate| -> i64 {
        let anchor_midnight = anchor.and_hms_opt(0, 0, 0).expect("midnight");
        let delta = dt.signed_duration_since(anchor_midnight);
        delta.num_seconds()
    };

    // Today's 24 hours, credited at today's seasonal Wh constant.
    if let (Some((sr, ss)), true) = (today, wh_today > 0.0) {
        let kwh_per_hour_today = wh_today / 1000.0;
        let sr_s = local_secs_of(sr, today_date);
        let ss_s = local_secs_of(ss, today_date);
        for (h, slot) in out.iter_mut().enumerate().take(24) {
            let h_start = i64::try_from(h).unwrap_or(0) * 3600;
            *slot = kwh_per_hour_today * overlap_hours(h_start, sr_s, ss_s);
        }
    }
    // Tomorrow's 24 hours — same overlap but anchored at tomorrow's
    // local midnight (= +24 h from today's local midnight, ignoring DST
    // shift; see DST note in module-level docs). Crucially, credited at
    // tomorrow's seasonal constant, not today's, so the winter/summer
    // boundary falls cleanly on the correct calendar date.
    if let (Some((sr, ss)), true) = (tomorrow, wh_tomorrow > 0.0) {
        let kwh_per_hour_tomorrow = wh_tomorrow / 1000.0;
        let tomorrow_date = today_date.succ_opt().expect("succ");
        let sr_s = local_secs_of(sr, tomorrow_date);
        let ss_s = local_secs_of(ss, tomorrow_date);
        for (h, slot) in out.iter_mut().enumerate().skip(24).take(24) {
            let local_h = i64::try_from(h - 24).unwrap_or(0) * 3600;
            *slot = kwh_per_hour_tomorrow * overlap_hours(local_h, sr_s, ss_s);
        }
    }
    // Suppress the "tz only used in match" lint when the match above
    // fully covers the LocalResult variants — `tz` is the only consumer
    // of the from_local_datetime call, used purely as a startup-time
    // sanity probe for DST-at-midnight zones.
    let _ = tz;

    out
}

/// True iff `(month, day)` falls within an inclusive `start..=end`
/// MM-DD range that may wrap across year boundary (e.g. Nov 1 → Mar 1).
fn mm_dd_in_range(today: (u32, u32), start: (u32, u32), end: (u32, u32)) -> bool {
    let to_ord = |(m, d): (u32, u32)| m * 100 + d;
    let t = to_ord(today);
    let s = to_ord(start);
    let e = to_ord(end);
    if s <= e {
        t >= s && t <= e
    } else {
        t >= s || t <= e
    }
}

/// True iff `date` falls within the configured (possibly year-wrapping)
/// winter MM-DD range.
#[must_use]
fn mm_dd_in_winter(date: NaiveDate, start: (u32, u32), end: (u32, u32)) -> bool {
    mm_dd_in_range((date.month(), date.day()), start, end)
}

/// Pick the per-day Wh-per-daylight-hour constant for a specific
/// calendar date, given the configured winter range and per-season
/// constants.
#[must_use]
fn wh_for_date(
    date: NaiveDate,
    winter_start: (u32, u32),
    winter_end: (u32, u32),
    wh_winter: f64,
    wh_summer: f64,
) -> f64 {
    if mm_dd_in_winter(date, winter_start, winter_end) {
        wh_winter
    } else {
        wh_summer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveDateTime};

    fn dt(date: NaiveDate, h: u32, m: u32) -> NaiveDateTime {
        date.and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn hourly_kwh_is_zero_when_wh_per_hour_zero() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let today = Some((dt(date, 4, 0), dt(date, 22, 0)));
        let out =
            build_hourly_kwh(today, today, 0.0, 0.0, date, chrono_tz::UTC);
        assert_eq!(out.len(), 48);
        assert!(out.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn full_overlap_hour_credits_full_kwh() {
        // Daylight 04:00..22:00, 200 Wh/hour ⇒ each fully-daylight hour
        // gets 0.2 kWh.
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let tomorrow = date.succ_opt().unwrap();
        let today = Some((dt(date, 4, 0), dt(date, 22, 0)));
        let tomorrow_dl = Some((dt(tomorrow, 4, 0), dt(tomorrow, 22, 0)));
        let out = build_hourly_kwh(today, tomorrow_dl, 200.0, 200.0, date, chrono_tz::UTC);
        // Hour 4..22 today: full kWh.
        for (h, &v) in out.iter().enumerate().take(22).skip(4) {
            assert!((v - 0.2).abs() < 1e-9, "hour {h} expected 0.2, got {v}");
        }
        // Night: zero.
        for &v in out.iter().take(4) {
            assert_eq!(v, 0.0);
        }
        for &v in out.iter().take(24).skip(22) {
            assert_eq!(v, 0.0);
        }
    }

    #[test]
    fn partial_overlap_hour_gets_fractional_kwh() {
        // Sunrise at 04:30 ⇒ hour 4 has 30 min of daylight ⇒ 0.5 kWh @
        // 1000 Wh/hour.
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let today = Some((dt(date, 4, 30), dt(date, 22, 0)));
        let out = build_hourly_kwh(today, None, 1000.0, 0.0, date, chrono_tz::UTC);
        assert!((out[4] - 0.5).abs() < 1e-9, "hour 4: {}", out[4]);
        // Sunset at 22:00 sharp ⇒ hour 21 fully credited (1.0), hour 22 zero.
        assert!((out[21] - 1.0).abs() < 1e-9, "hour 21: {}", out[21]);
        assert_eq!(out[22], 0.0);
    }

    #[test]
    fn polar_day_today_yields_all_zero_today_window() {
        let date = NaiveDate::from_ymd_opt(2026, 12, 21).unwrap();
        let out = build_hourly_kwh(None, None, 1000.0, 1000.0, date, chrono_tz::UTC);
        assert_eq!(out.len(), 48);
        assert!(out.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn season_boundary_picks_per_day_wh_constant() {
        // Today still in winter, tomorrow first summer day. Verify the
        // tomorrow bucket uses summer Wh, not winter.
        let date = NaiveDate::from_ymd_opt(2026, 2, 28).unwrap();
        let tomorrow = date.succ_opt().unwrap();
        let today = Some((dt(date, 8, 0), dt(date, 17, 0)));
        let tomorrow_dl = Some((dt(tomorrow, 8, 0), dt(tomorrow, 17, 0)));
        // wh_today = 100 (winter), wh_tomorrow = 1000 (summer).
        let out = build_hourly_kwh(today, tomorrow_dl, 100.0, 1000.0, date, chrono_tz::UTC);
        // Mid-day today: 0.1 kWh.
        assert!((out[12] - 0.1).abs() < 1e-9, "hour 12 today: {}", out[12]);
        // Mid-day tomorrow: 1.0 kWh — would have been 0.1 if the bug
        // were still present.
        assert!((out[36] - 1.0).abs() < 1e-9, "hour 12 tomorrow: {}", out[36]);
    }

    #[test]
    fn winter_range_wraps_year_boundary() {
        let s = (11, 1);
        let e = (3, 1);
        let dec_15 = NaiveDate::from_ymd_opt(2026, 12, 15).unwrap();
        let jul_15 = NaiveDate::from_ymd_opt(2026, 7, 15).unwrap();
        assert!(mm_dd_in_winter(dec_15, s, e));
        assert!(!mm_dd_in_winter(jul_15, s, e));
    }

    #[test]
    fn known_london_summer_solstice_has_long_daylight() {
        // Sanity check on the sunrise crate via the public path. London,
        // 2026-06-21: sunrise ~04:43 BST, sunset ~21:21 BST → ~16.6 h
        // of daylight.
        let coord = Coordinates::new(51.5, -0.13).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let tz = chrono_tz::Europe::London;
        let (sr, ss) = sunrise_sunset_local(coord, date, tz).unwrap();
        let daylight_h = ss.signed_duration_since(sr).num_minutes() as f64 / 60.0;
        assert!(
            (15.5..17.5).contains(&daylight_h),
            "expected ~16.6h daylight on summer solstice in London, got {daylight_h}"
        );
        // Sunrise must precede sunset on the same calendar day.
        assert!(sr < ss);
    }

}
