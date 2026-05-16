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
use tokio::sync::{mpsc, Mutex, Notify};
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
    /// PR2: cloud-cover modulation knobs. Snapshotted alongside the
    /// season/Wh knobs so a single locked read of `World.knobs` covers
    /// every input to a baseline tick.
    cloud_sunny_threshold_pct: u32,
    cloud_cloudy_threshold_pct: u32,
    cloud_factor_sunny: f64,
    cloud_factor_partial: f64,
    cloud_factor_cloudy: f64,
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
            w.knobs.baseline_cloud_sunny_threshold_pct,
            w.knobs.baseline_cloud_cloudy_threshold_pct,
            w.knobs.baseline_cloud_factor_sunny,
            w.knobs.baseline_cloud_factor_partial,
            w.knobs.baseline_cloud_factor_cloudy,
        )
    };
    let (raw_start, raw_end, wh_w, wh_s, c_sun_t, c_cld_t, c_f_sun, c_f_par, c_f_cld) = k;
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
        cloud_sunny_threshold_pct: c_sun_t,
        cloud_cloudy_threshold_pct: c_cld_t,
        cloud_factor_sunny: c_f_sun,
        cloud_factor_partial: c_f_par,
        cloud_factor_cloudy: c_f_cld,
    }
}

/// PR2: maximum age of the cloud forecast that the baseline scheduler
/// will trust. Past this, fall back to no modulation (cleanly
/// degraded behaviour: a stale poller silently drops cloud
/// awareness, doesn't crash). 2 h is comfortable margin over the
/// default 15-min current-weather cadence.
const CLOUD_FORECAST_MAX_AGE: Duration = Duration::from_secs(2 * 3600);

/// PR2: pull a cloned `(hourly_cover_pct, fetched_at)` pair if the
/// stored cloud forecast is non-empty AND fresh enough to act on.
async fn read_fresh_cloud_forecast(
    world: &Arc<Mutex<World>>,
    now: Instant,
) -> Option<Vec<f64>> {
    let w = world.lock().await;
    let snap = w.typed_sensors.weather_cloud_forecast.as_ref()?;
    if snap.hourly_cover_pct.len() != 48 {
        return None;
    }
    if now.saturating_duration_since(snap.fetched_at) > CLOUD_FORECAST_MAX_AGE {
        return None;
    }
    Some(snap.hourly_cover_pct.clone())
}

/// PR2: 3-bucket multiplier selecting cloud-factor by cover %. NaN
/// (or an out-of-bounds bucket index — shouldn't happen given knob
/// ranges) returns 1.0 so a missing per-hour datum collapses to the
/// unmodulated path.
fn cloud_factor(cover_pct: f64, k: BaselineKnobs) -> f64 {
    if !cover_pct.is_finite() {
        return 1.0;
    }
    let sunny_t = f64::from(k.cloud_sunny_threshold_pct);
    let cloudy_t = f64::from(k.cloud_cloudy_threshold_pct);
    if cover_pct < sunny_t {
        k.cloud_factor_sunny
    } else if cover_pct < cloudy_t {
        k.cloud_factor_partial
    } else {
        k.cloud_factor_cloudy
    }
}

/// Apply per-hour cloud modulation to a 48-element hourly_kwh array.
/// Slots with non-finite cloud cover (or no cloud array) are left
/// unchanged. Returns the cloud array echoed for the emitted
/// snapshot (empty when no modulation occurred).
fn apply_cloud_modulation(
    hourly_kwh: &mut [f64],
    cloud: Option<&[f64]>,
    k: BaselineKnobs,
) -> Vec<f64> {
    let Some(cloud) = cloud else { return Vec::new() };
    if cloud.len() != 48 {
        return Vec::new();
    }
    for (kwh, &cover) in hourly_kwh.iter_mut().zip(cloud.iter()) {
        *kwh *= cloud_factor(cover, k);
    }
    cloud.to_vec()
}

/// Public entry point — spawned by `main` when
/// `cfg.forecast.baseline.is_configured()` returns true.
///
/// PR2: `cloud_forecast_arrived` lets the current-weather poller wake
/// the scheduler the moment a fresh cloud forecast lands, so the
/// startup race (baseline ticks at t=0 before the cloud HTTP fetch
/// completes; next cadence isn't for an hour) doesn't strand the
/// dashboard on an unmodulated, no-cloud snapshot for up to one full
/// cadence. The notify is best-effort: missed notifications are fine
/// — the next periodic tick picks up the data anyway.
pub async fn run_baseline_scheduler(
    params: BaselineParams,
    world: Arc<Mutex<World>>,
    tx: mpsc::Sender<Event>,
    cloud_forecast_arrived: Arc<Notify>,
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
        // Wake on either the periodic cadence or a cloud-forecast
        // arrival. The first cadence tick fires immediately (tokio
        // interval semantics) — that's fine; the `notified()` branch
        // takes care of the case where cloud data lands after that
        // initial tick but well before the next cadence boundary.
        tokio::select! {
            _ = ticker.tick() => {}
            () = cloud_forecast_arrived.notified() => {}
        }

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

        let mut hourly_kwh =
            build_hourly_kwh(today_sr_ss, tomorrow_sr_ss, wh_today, wh_tomorrow, today, tz);
        // PR2: if a fresh hourly cloud forecast is available, scale
        // each per-hour Wh credit by the bucket factor selected for
        // that hour's cloud cover. Stale / missing → leave Wh
        // unmodulated and emit an empty cloud array (signals "no
        // cloud info this tick" to the dashboard popup).
        let now = Instant::now();
        let cloud_arr = read_fresh_cloud_forecast(&world, now).await;
        let hourly_cloud_cover_pct =
            apply_cloud_modulation(&mut hourly_kwh, cloud_arr.as_deref(), bk);
        let today_kwh: f64 = hourly_kwh.iter().take(24).sum();
        let tomorrow_kwh: f64 = hourly_kwh.iter().skip(24).sum();

        let send_result = tx
            .send(Event::TypedSensor(TypedReading::Forecast {
                provider: ForecastProvider::Baseline,
                today_kwh,
                tomorrow_kwh,
                hourly_kwh,
                // Baseline doesn't model temperature.
                hourly_temperature_c: Vec::new(),
                // PR2: echo the cloud array we modulated against, so
                // the per-hour forecast popup can show what drove the
                // numbers. Empty when no fresh cloud forecast existed.
                hourly_cloud_cover_pct,
                at: now,
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

    fn default_cloud_knobs() -> BaselineKnobs {
        BaselineKnobs {
            winter_start: (11, 1),
            winter_end: (3, 1),
            wh_per_hour_winter: 100.0,
            wh_per_hour_summer: 1000.0,
            cloud_sunny_threshold_pct: 30,
            cloud_cloudy_threshold_pct: 70,
            cloud_factor_sunny: 1.0,
            cloud_factor_partial: 0.6,
            cloud_factor_cloudy: 0.25,
        }
    }

    #[test]
    fn cloud_factor_buckets_at_default_thresholds() {
        let k = default_cloud_knobs();
        assert!((cloud_factor(0.0, k) - 1.0).abs() < 1e-9);
        // Just below sunny threshold.
        assert!((cloud_factor(29.999, k) - 1.0).abs() < 1e-9);
        // At sunny threshold → partial.
        assert!((cloud_factor(30.0, k) - 0.6).abs() < 1e-9);
        assert!((cloud_factor(50.0, k) - 0.6).abs() < 1e-9);
        // Just below cloudy threshold.
        assert!((cloud_factor(69.999, k) - 0.6).abs() < 1e-9);
        // At cloudy threshold → cloudy.
        assert!((cloud_factor(70.0, k) - 0.25).abs() < 1e-9);
        assert!((cloud_factor(100.0, k) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn cloud_factor_returns_one_on_nan() {
        // NaN sentinel marks "no per-hour datum"; we must not modulate.
        assert!((cloud_factor(f64::NAN, default_cloud_knobs()) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn apply_cloud_modulation_scales_and_echoes_cloud_array() {
        let mut hk = vec![1.0; 48];
        let mut cloud = vec![10.0; 48]; // all sunny
        // Hour 12: cloudy.
        cloud[12] = 90.0;
        // Hour 30: partial.
        cloud[30] = 50.0;
        // Hour 40: NaN — must leave that hour unchanged.
        cloud[40] = f64::NAN;
        let echo = apply_cloud_modulation(&mut hk, Some(&cloud), default_cloud_knobs());
        assert_eq!(echo.len(), 48);
        assert!((echo[12] - 90.0).abs() < 1e-9);
        // Spot-check the three buckets.
        assert!((hk[0] - 1.0).abs() < 1e-9, "sunny untouched");
        assert!((hk[12] - 0.25).abs() < 1e-9, "cloudy: {}", hk[12]);
        assert!((hk[30] - 0.6).abs() < 1e-9, "partial: {}", hk[30]);
        assert!((hk[40] - 1.0).abs() < 1e-9, "NaN slot must not modulate");
    }

    #[test]
    fn apply_cloud_modulation_skips_when_no_cloud_array() {
        let mut hk = vec![1.0; 48];
        let echo = apply_cloud_modulation(&mut hk, None, default_cloud_knobs());
        assert!(echo.is_empty());
        assert!(hk.iter().all(|v| (v - 1.0).abs() < 1e-9));
    }

    #[test]
    fn apply_cloud_modulation_skips_on_wrong_length() {
        let mut hk = vec![1.0; 48];
        let cloud = vec![50.0; 24]; // half-length → reject
        let echo = apply_cloud_modulation(&mut hk, Some(&cloud), default_cloud_knobs());
        assert!(echo.is_empty());
        assert!(hk.iter().all(|v| (v - 1.0).abs() < 1e-9));
    }

    #[test]
    fn cloud_modulation_disabled_when_all_factors_one() {
        let mut k = default_cloud_knobs();
        k.cloud_factor_sunny = 1.0;
        k.cloud_factor_partial = 1.0;
        k.cloud_factor_cloudy = 1.0;
        let mut hk = vec![2.0; 48];
        let cloud = vec![90.0; 48]; // would normally crush to 0.5
        apply_cloud_modulation(&mut hk, Some(&cloud), k);
        assert!(hk.iter().all(|v| (v - 2.0).abs() < 1e-9));
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
