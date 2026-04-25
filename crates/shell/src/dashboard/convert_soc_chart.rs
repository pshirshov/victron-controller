//! PR-soc-chart-segments: piecewise-linear projection compute for the
//! dashboard's SoC chart.
//!
//! Walks a 24 h horizon split at schedule-window edges, the
//! `next_full_charge` push window, and SoC clamps. Each segment has a
//! `kind` (Natural / Idle / ScheduledCharge / FullChargePush / Clamped)
//! and an end SoC computed from the slope appropriate for that segment.
//!
//! Pure: callable from tests with a hand-built world.

use std::str::FromStr;

use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, TimeZone, Utc};

use victron_controller_core::controllers::schedules::{DAYS_ENABLED, ScheduleSpec};
use victron_controller_core::tass::{Actual, Actuated, Freshness};
use victron_controller_core::topology::HardwareParams;
use victron_controller_core::world::World;

use victron_controller_dashboard_model::victron_controller::dashboard::soc_chart::SocChart as ModelSocChart;
use victron_controller_dashboard_model::victron_controller::dashboard::soc_history_sample::SocHistorySample as ModelSocSample;
use victron_controller_dashboard_model::victron_controller::dashboard::soc_projection::SocProjection as ModelSocProjection;
use victron_controller_dashboard_model::victron_controller::dashboard::soc_projection_kind::SocProjectionKind as ModelKind;
use victron_controller_dashboard_model::victron_controller::dashboard::soc_projection_segment::SocProjectionSegment as ModelSegment;

use crate::dashboard::soc_history::SocHistorySample as ShellSocSample;

/// Idle threshold in W: below this magnitude `net_power` we treat the
/// battery as quiescent and emit `Idle` segments. 50 W is well below
/// the noise floor of typical home loads.
const SOC_IDLE_POWER_W: f64 = 50.0;

/// Cap the projection horizon at +24 h.
const SOC_PROJECTION_HORIZON_H: i64 = 24;

/// SoC floor used as the depleting clamp.
const SOC_DEPLETION_FLOOR_PCT: f64 = 10.0;

/// SoC ceiling used as the natural filling clamp.
const SOC_FULL_PCT: f64 = 100.0;

/// Default scheduled-charge ceiling for `ScheduledCharge` slope. This is
/// an approximate ceiling; real rates depend on inverter limits, the
/// grid-import-limit knob, and battery state. We use it when
/// `max_grid_current_a * grid_nominal_voltage_v` would exceed it.
const MAX_CHARGE_RATE_W_DEFAULT: f64 = 5000.0;

/// Width of the `FullChargePush` window, starting at `next_full_charge`.
/// 1 h is a heuristic — the actual full-charge cycle ends when the
/// battery hits 100 %, but for projection purposes assume one hour of
/// aggressive charging.
const FULL_CHARGE_PUSH_DURATION_H: i64 = 1;

const HOUR_MS: i64 = 3_600_000;

/// Build the wire `SocChart` from the shell-side history ring + the
/// segment-walker projection.
#[must_use]
pub fn compute_soc_chart(
    world: &World,
    history: &[ShellSocSample],
    hardware: HardwareParams,
    now_ms: i64,
) -> ModelSocChart {
    let s = &world.sensors;
    let fresh = |a: &Actual<f64>| matches!(a.freshness, Freshness::Fresh);
    let usable = |a: &Actual<f64>| -> Option<f64> {
        if !fresh(a) {
            return None;
        }
        let v = a.value?;
        if v.is_finite() {
            Some(v)
        } else {
            None
        }
    };

    let now_soc = usable(&s.battery_soc);
    let net_power_w = usable(&s.battery_dc_power);
    let installed_ah = usable(&s.battery_installed_capacity);
    let soh_pct = usable(&s.battery_soh);

    let capacity_wh = match (installed_ah, soh_pct) {
        (Some(ah), Some(soh)) if ah > 0.0 && soh > 0.0 => {
            Some(ah * (soh / 100.0) * hardware.battery_nominal_voltage_v)
        }
        _ => None,
    };

    let charge_rate_w = derive_charge_rate_w(hardware);

    let projection = compute_projection(&ProjectionInputs {
        now_ms,
        now_soc,
        net_power_w,
        capacity_wh,
        charge_rate_w,
        schedule_0: enabled_schedule(&world.schedule_0),
        schedule_1: enabled_schedule(&world.schedule_1),
        next_full_charge: world.bookkeeping.next_full_charge,
        timezone_iana: &world.timezone,
    });

    let history_wire: Vec<ModelSocSample> = history
        .iter()
        .map(|s| ModelSocSample {
            epoch_ms: s.epoch_ms,
            soc_pct: s.soc,
        })
        .collect();

    ModelSocChart {
        history: history_wire,
        projection,
        now_epoch_ms: now_ms,
        now_soc_pct: now_soc,
        // Surfacing the controller-side targets gives the chart two
        // horizontal reference lines so the operator can see the
        // SoC envelope being defended.
        discharge_target_pct: Some(world.bookkeeping.soc_end_of_day_target),
        charge_target_pct: Some(world.bookkeeping.battery_selected_soc_target),
    }
}

/// `min(max_grid_current_a * grid_nominal_voltage_v, MAX_CHARGE_RATE_W_DEFAULT)`,
/// or None when the hardware values aren't sane. This is an approximate
/// ceiling; real rates depend on inverter limits, the grid-import-limit
/// knob, and battery state.
fn derive_charge_rate_w(hardware: HardwareParams) -> Option<f64> {
    let raw = hardware.max_grid_current_a * hardware.grid_nominal_voltage_v;
    if raw.is_finite() && raw > 0.0 {
        Some(raw.min(MAX_CHARGE_RATE_W_DEFAULT))
    } else {
        None
    }
}

/// Return the schedule's `ScheduleSpec` if it's enabled (`days == 7`).
/// Reads `target.value` first (commanded shape — what we're aiming for),
/// falls back to `actual.value` (last D-Bus readback).
fn enabled_schedule(a: &Actuated<ScheduleSpec>) -> Option<ScheduleSpec> {
    let spec = a.target.value.or(a.actual.value)?;
    if spec.days == DAYS_ENABLED {
        Some(spec)
    } else {
        None
    }
}

struct ProjectionInputs<'a> {
    now_ms: i64,
    now_soc: Option<f64>,
    net_power_w: Option<f64>,
    capacity_wh: Option<f64>,
    charge_rate_w: Option<f64>,
    schedule_0: Option<ScheduleSpec>,
    schedule_1: Option<ScheduleSpec>,
    next_full_charge: Option<NaiveDateTime>,
    timezone_iana: &'a str,
}

#[derive(Debug, Clone, Copy)]
struct Window {
    start_ms: i64,
    end_ms: i64,
    /// SoC ceiling within the window (`schedule.soc` for ScheduledCharge,
    /// 100 for FullChargePush). End SoC inside the window is clamped to
    /// this — once hit we emit a `Clamped` segment for the remainder.
    soc_ceiling: f64,
    kind: WindowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowKind {
    Schedule,
    FullCharge,
}

fn compute_projection(inputs: &ProjectionInputs<'_>) -> ModelSocProjection {
    let none_proj = || ModelSocProjection {
        segments: Vec::new(),
        net_power_w: inputs.net_power_w,
        capacity_wh: inputs.capacity_wh,
        charge_rate_w: inputs.charge_rate_w,
    };

    // We need at minimum a SoC and a capacity to produce any segment.
    let (Some(soc0), Some(capacity_wh)) = (inputs.now_soc, inputs.capacity_wh) else {
        // Surface the diagnostic that the inputs were short — at debug
        // level since this happens normally during boot.
        tracing::debug!(
            target: "soc_chart",
            now_soc = ?inputs.now_soc,
            capacity_wh = ?inputs.capacity_wh,
            net_power_w = ?inputs.net_power_w,
            "soc_chart projection skipped: missing required inputs"
        );
        return none_proj();
    };
    if capacity_wh <= 0.0 {
        return none_proj();
    }
    let net_power_w = inputs.net_power_w.unwrap_or(0.0);

    // Build the window list inside [now, now + 24h], rooted in the
    // configured display TZ so `start_s` (seconds from local midnight)
    // maps to the right wall-clock time.
    let horizon_ms = inputs
        .now_ms
        .saturating_add(SOC_PROJECTION_HORIZON_H * HOUR_MS);
    let tz = chrono_tz::Tz::from_str(inputs.timezone_iana).unwrap_or(chrono_tz::UTC);

    let mut windows: Vec<Window> = Vec::new();
    for spec in [inputs.schedule_0, inputs.schedule_1].iter().flatten() {
        for win in expand_schedule_windows(*spec, inputs.now_ms, horizon_ms, tz) {
            windows.push(win);
        }
    }
    if let Some(nfc) = inputs.next_full_charge {
        if let Some(win) = full_charge_window(nfc, inputs.now_ms, horizon_ms, tz) {
            windows.push(win);
        }
    }

    // Sort + dedup by start (windows can overlap — `classify_at` resolves
    // conflicts by precedence: FullChargePush > ScheduledCharge).
    windows.sort_by_key(|w| (w.start_ms, w.end_ms));

    // Build event list: now, horizon, all window starts/ends.
    let mut events: Vec<i64> = Vec::with_capacity(2 + windows.len() * 2);
    events.push(inputs.now_ms);
    events.push(horizon_ms);
    for w in &windows {
        if w.start_ms > inputs.now_ms && w.start_ms < horizon_ms {
            events.push(w.start_ms);
        }
        if w.end_ms > inputs.now_ms && w.end_ms < horizon_ms {
            events.push(w.end_ms);
        }
    }
    events.sort_unstable();
    events.dedup();

    // Walk pairwise, emitting one or two segments per gap (one extra
    // when an SoC clamp fires partway through).
    let mut segments: Vec<ModelSegment> = Vec::new();
    let mut soc = soc0;
    for pair in events.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if a >= b {
            continue;
        }
        // Classify by midpoint so a window touching the boundary
        // (start_ms == a) still picks the in-window kind.
        let midpoint = a.saturating_add((b - a) / 2);
        let active_window = active_window_at(&windows, midpoint);
        let (slope_w, kind, ceiling, floor) = classify_segment(
            active_window,
            net_power_w,
            inputs.charge_rate_w,
            soc,
        );

        let dur_h = (b - a) as f64 / (HOUR_MS as f64);
        let slope_pct_per_hour = slope_w / capacity_wh * 100.0;
        let raw_end = soc + slope_pct_per_hour * dur_h;

        // If `Clamped` (slope = 0) — emit one flat segment at `soc`.
        if matches!(kind, ModelKind::Clamped) {
            segments.push(seg(a, b, soc, soc, ModelKind::Clamped));
            // `soc` unchanged.
            continue;
        }

        // Compute clamp ceiling/floor and detect mid-segment clamp hits.
        let mut hit_at_ms: Option<(i64, f64)> = None;
        if slope_pct_per_hour > 0.0 && raw_end > ceiling {
            let pct_to_ceil = ceiling - soc;
            if pct_to_ceil <= 0.0 {
                // Already at/above ceiling — emit only Clamped.
                segments.push(seg(a, b, soc, soc, ModelKind::Clamped));
                continue;
            }
            let h = pct_to_ceil / slope_pct_per_hour;
            let split_ms = a.saturating_add((h * (HOUR_MS as f64)) as i64);
            hit_at_ms = Some((split_ms.clamp(a, b), ceiling));
        } else if slope_pct_per_hour < 0.0 && raw_end < floor {
            let pct_to_floor = soc - floor;
            if pct_to_floor <= 0.0 {
                segments.push(seg(a, b, soc, soc, ModelKind::Clamped));
                continue;
            }
            let h = pct_to_floor / -slope_pct_per_hour;
            let split_ms = a.saturating_add((h * (HOUR_MS as f64)) as i64);
            hit_at_ms = Some((split_ms.clamp(a, b), floor));
        }

        if let Some((split_ms, end_at)) = hit_at_ms {
            // Rising/falling part up to the clamp, then a Clamped tail.
            if split_ms > a {
                segments.push(seg(a, split_ms, soc, end_at, kind));
            }
            if split_ms < b {
                segments.push(seg(split_ms, b, end_at, end_at, ModelKind::Clamped));
            }
            soc = end_at;
        } else {
            segments.push(seg(a, b, soc, raw_end, kind));
            soc = raw_end;
        }
    }

    ModelSocProjection {
        segments,
        net_power_w: inputs.net_power_w,
        capacity_wh: inputs.capacity_wh,
        charge_rate_w: inputs.charge_rate_w,
    }
}

fn seg(a: i64, b: i64, start_soc: f64, end_soc: f64, kind: ModelKind) -> ModelSegment {
    ModelSegment {
        start_epoch_ms: a,
        end_epoch_ms: b,
        start_soc_pct: start_soc,
        end_soc_pct: end_soc,
        kind,
    }
}

/// Return the highest-priority window that contains `epoch_ms` (no
/// window if none match). FullChargePush wins over Schedule.
fn active_window_at(windows: &[Window], epoch_ms: i64) -> Option<Window> {
    let mut best: Option<Window> = None;
    for w in windows {
        if epoch_ms >= w.start_ms && epoch_ms < w.end_ms {
            let win_priority = u8::from(matches!(w.kind, WindowKind::FullCharge));
            let cur_priority =
                best.map_or(0, |b| u8::from(matches!(b.kind, WindowKind::FullCharge)));
            if best.is_none() || win_priority > cur_priority {
                best = Some(*w);
            }
        }
    }
    best
}

/// Returns `(slope_w, kind, soc_ceiling, soc_floor)` for the segment.
/// Floor is always `SOC_DEPLETION_FLOOR_PCT`. Ceiling depends on the
/// active window's spec.
fn classify_segment(
    active: Option<Window>,
    natural_net_power_w: f64,
    charge_rate_w: Option<f64>,
    soc: f64,
) -> (f64, ModelKind, f64, f64) {
    let floor = SOC_DEPLETION_FLOOR_PCT;
    if let Some(win) = active {
        // Inside a window. If we're already at/above the per-window
        // ceiling, emit Clamped (zero slope).
        if soc >= win.soc_ceiling {
            return (0.0, ModelKind::Clamped, win.soc_ceiling, floor);
        }
        // Use the charge rate if known; otherwise treat as Natural slope
        // (don't fabricate a charge rate from nothing).
        let kind = match win.kind {
            WindowKind::FullCharge => ModelKind::FullChargePush,
            WindowKind::Schedule => ModelKind::ScheduledCharge,
        };
        match charge_rate_w {
            Some(w) => (w, kind, win.soc_ceiling, floor),
            None => (natural_net_power_w, kind, win.soc_ceiling, floor),
        }
    } else {
        // Outside any window. Idle when net power is below the noise
        // threshold; otherwise Natural.
        if natural_net_power_w.abs() < SOC_IDLE_POWER_W {
            (0.0, ModelKind::Idle, SOC_FULL_PCT, floor)
        } else {
            (natural_net_power_w, ModelKind::Natural, SOC_FULL_PCT, floor)
        }
    }
}

/// Expand a daily (`days == 7`) schedule into its absolute epoch-ms
/// windows that intersect `[now_ms, horizon_ms]`. Walk the local
/// calendar dates spanning the horizon and emit one window per date.
fn expand_schedule_windows(
    spec: ScheduleSpec,
    now_ms: i64,
    horizon_ms: i64,
    tz: chrono_tz::Tz,
) -> Vec<Window> {
    if spec.duration_s <= 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    // Walk local dates that could intersect — the horizon is ≤ 24 h, so
    // we only need today + tomorrow + day-after (defensive against
    // window straddling midnight).
    let now_utc: DateTime<Utc> = match Utc.timestamp_millis_opt(now_ms).single() {
        Some(dt) => dt,
        None => return Vec::new(),
    };
    let now_local = now_utc.with_timezone(&tz);
    let base_date = now_local.date_naive();
    for day_offset in -1_i64..=2 {
        let date = base_date + ChronoDuration::days(day_offset);
        let local_midnight = match date.and_hms_opt(0, 0, 0) {
            Some(dt) => dt,
            None => continue,
        };
        // Disambiguate DST-fold via `from_local_datetime`. Pick `single`
        // when unambiguous; otherwise the earliest mapping (the chart is
        // a forecast, not a control signal — sub-hour skew during DST
        // transitions is acceptable).
        let local_midnight_dt = match tz.from_local_datetime(&local_midnight) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(early, _late) => early,
            chrono::LocalResult::None => continue,
        };
        let start_utc = local_midnight_dt + ChronoDuration::seconds(i64::from(spec.start_s));
        let end_utc = start_utc + ChronoDuration::seconds(i64::from(spec.duration_s));
        let start_ms = start_utc.timestamp_millis();
        let end_ms = end_utc.timestamp_millis();
        // Clip to the horizon.
        let clipped_start = start_ms.max(now_ms);
        let clipped_end = end_ms.min(horizon_ms);
        if clipped_start < clipped_end {
            out.push(Window {
                start_ms: clipped_start,
                end_ms: clipped_end,
                soc_ceiling: spec.soc,
                kind: WindowKind::Schedule,
            });
        }
    }
    out
}

fn full_charge_window(
    nfc: NaiveDateTime,
    now_ms: i64,
    horizon_ms: i64,
    tz: chrono_tz::Tz,
) -> Option<Window> {
    let local_dt = match tz.from_local_datetime(&nfc) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(early, _late) => early,
        chrono::LocalResult::None => return None,
    };
    let start_ms = local_dt.timestamp_millis();
    let end_ms = start_ms.saturating_add(FULL_CHARGE_PUSH_DURATION_H * HOUR_MS);
    let clipped_start = start_ms.max(now_ms);
    let clipped_end = end_ms.min(horizon_ms);
    if clipped_start >= clipped_end {
        return None;
    }
    Some(Window {
        start_ms: clipped_start,
        end_ms: clipped_end,
        soc_ceiling: SOC_FULL_PCT,
        kind: WindowKind::FullCharge,
    })
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use victron_controller_core::controllers::schedules::{DAYS_DISABLED, DAYS_ENABLED};
    use victron_controller_core::Owner;
    use victron_controller_core::tass::{Actual, Actuated, Freshness, TargetPhase};

    fn fresh(value: f64, now: Instant) -> Actual<f64> {
        let mut a = Actual::unknown(now);
        a.value = Some(value);
        a.freshness = Freshness::Fresh;
        a.since = now;
        a
    }

    /// World pre-populated with the four SoC-chart inputs all Fresh.
    /// Capacity = 200 Ah × 100 % SoH × 50 V = 10000 Wh.
    fn world_with_inputs(soc: f64, dc_power: f64, soh: f64, ah: f64) -> World {
        let now = Instant::now();
        let mut w = World::fresh_boot(now);
        w.sensors.battery_soc = fresh(soc, now);
        w.sensors.battery_dc_power = fresh(dc_power, now);
        w.sensors.battery_soh = fresh(soh, now);
        w.sensors.battery_installed_capacity = fresh(ah, now);
        w
    }

    fn hw_50v() -> HardwareParams {
        let mut h = HardwareParams::defaults();
        // Round capacity arithmetic: 10000 Wh = 200 Ah × 100 % × 50 V.
        h.battery_nominal_voltage_v = 50.0;
        h
    }

    fn install_schedule(w: &mut World, slot: usize, spec: ScheduleSpec) {
        let target = match slot {
            0 => &mut w.schedule_0,
            1 => &mut w.schedule_1,
            _ => panic!("invalid slot"),
        };
        target.target.value = Some(spec);
        target.target.owner = Owner::ScheduleController;
        target.target.phase = TargetPhase::Confirmed;
    }

    /// Pick the time-of-day sin time so a depleting natural slope drops
    /// SoC without a schedule firing. Use a fixed UTC `now_ms` so the
    /// midnight-relative scheduling is deterministic. 2026-04-25 23:00 UTC.
    const TEST_NOW_MS: i64 = 1_777_503_600_000; // 2026-04-25 23:00:00 UTC

    #[test]
    fn no_schedule_natural_slope_only() {
        // Depleting at -500 W from 50 % into a 10 kWh pack → -5 %/h.
        // Should produce a single Natural segment that splits at the
        // 10 % floor and emits a trailing Clamped tail.
        let w = world_with_inputs(50.0, -500.0, 100.0, 200.0);
        let chart = compute_soc_chart(&w, &[], hw_50v(), TEST_NOW_MS);
        let segs = &chart.projection.segments;
        // Expect: Natural [now, now+8h] from 50→10, then Clamped tail.
        assert!(!segs.is_empty(), "expected some segments");
        let first = &segs[0];
        assert_eq!(first.kind, ModelKind::Natural);
        assert!((first.start_soc_pct - 50.0).abs() < 1e-6);
        assert!((first.end_soc_pct - SOC_DEPLETION_FLOOR_PCT).abs() < 1e-6);
        // Last segment should be Clamped at the floor.
        let last = segs.last().unwrap();
        assert_eq!(last.kind, ModelKind::Clamped);
        assert!((last.end_soc_pct - SOC_DEPLETION_FLOOR_PCT).abs() < 1e-6);
    }

    #[test]
    fn idle_outside_window() {
        // 30 W is below the 50 W idle floor → single Idle segment.
        let w = world_with_inputs(50.0, 30.0, 100.0, 200.0);
        let chart = compute_soc_chart(&w, &[], hw_50v(), TEST_NOW_MS);
        let segs = &chart.projection.segments;
        assert_eq!(segs.len(), 1, "expected single Idle segment, got {segs:?}");
        assert_eq!(segs[0].kind, ModelKind::Idle);
        assert!((segs[0].start_soc_pct - 50.0).abs() < 1e-6);
        assert!((segs[0].end_soc_pct - 50.0).abs() < 1e-6);
        // Diagnostics still propagated.
        assert_eq!(chart.projection.net_power_w, Some(30.0));
        assert_eq!(chart.projection.capacity_wh, Some(10_000.0));
    }

    #[test]
    fn schedule_window_charges_to_target() {
        // Now = 23:00 UTC (TEST_NOW_MS). schedule_0 at 02:00 UTC for 3 h,
        // soc=80, days=7 (enabled). SoC=40, depleting at -500 W
        // (-5 %/h into 10 kWh).
        // Expected segment chain (using UTC throughout):
        //   23:00 → 02:00 (3h Natural, SoC drops 40 → 25)
        //   02:00 → 02:??  (ScheduledCharge climbing to 80)
        //   02:?? → 05:00 (Clamped at 80)
        //   05:00 → 23:00+24h (Natural again — depleting toward 10)
        let mut w = world_with_inputs(40.0, -500.0, 100.0, 200.0);
        // Force UTC for the test so scheduled windows align deterministically.
        w.timezone = "Etc/UTC".to_string();
        install_schedule(
            &mut w,
            0,
            ScheduleSpec {
                start_s: 2 * 3600,
                duration_s: 3 * 3600,
                discharge: 0,
                soc: 80.0,
                days: DAYS_ENABLED,
            },
        );
        let chart = compute_soc_chart(&w, &[], hw_50v(), TEST_NOW_MS);
        let segs = &chart.projection.segments;

        // The first segment must be Natural (depleting before schedule fires).
        assert_eq!(segs[0].kind, ModelKind::Natural);
        assert!((segs[0].start_soc_pct - 40.0).abs() < 1e-6);

        // There must be at least one ScheduledCharge segment.
        let has_sched = segs.iter().any(|s| s.kind == ModelKind::ScheduledCharge);
        assert!(has_sched, "expected a ScheduledCharge segment in {segs:?}");

        // The ScheduledCharge climbs to 80 then we expect a Clamped tail.
        let mut found_climb_then_clamp = false;
        for win in segs.windows(2) {
            if win[0].kind == ModelKind::ScheduledCharge
                && (win[0].end_soc_pct - 80.0).abs() < 1e-6
                && win[1].kind == ModelKind::Clamped
            {
                found_climb_then_clamp = true;
                break;
            }
        }
        assert!(
            found_climb_then_clamp,
            "expected ScheduledCharge→Clamped at 80%, got {segs:?}"
        );
    }

    #[test]
    fn full_charge_push_overrides() {
        // next_full_charge in 30 min. Should produce a FullChargePush
        // segment starting at that time, climbing to 100.
        let mut w = world_with_inputs(50.0, -500.0, 100.0, 200.0);
        w.timezone = "Etc/UTC".to_string();
        // 30 min after TEST_NOW_MS in UTC.
        let nfc_ms = TEST_NOW_MS + 30 * 60 * 1000;
        let nfc = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(nfc_ms)
            .unwrap()
            .naive_utc();
        w.bookkeeping.next_full_charge = Some(nfc);
        let chart = compute_soc_chart(&w, &[], hw_50v(), TEST_NOW_MS);
        let segs = &chart.projection.segments;
        let has_fc = segs.iter().any(|s| s.kind == ModelKind::FullChargePush);
        assert!(has_fc, "expected a FullChargePush segment in {segs:?}");
        // The FullChargePush segment should start at nfc_ms.
        let fc_seg = segs
            .iter()
            .find(|s| s.kind == ModelKind::FullChargePush)
            .unwrap();
        assert_eq!(fc_seg.start_epoch_ms, nfc_ms);
    }

    #[test]
    fn targets_propagated() {
        let mut w = world_with_inputs(50.0, -500.0, 100.0, 200.0);
        w.bookkeeping.soc_end_of_day_target = 35.0;
        w.bookkeeping.battery_selected_soc_target = 85.0;
        let chart = compute_soc_chart(&w, &[], hw_50v(), TEST_NOW_MS);
        assert_eq!(chart.discharge_target_pct, Some(35.0));
        assert_eq!(chart.charge_target_pct, Some(85.0));
    }

    #[test]
    fn skips_disabled_schedule() {
        // schedule_0 with days=DAYS_DISABLED → ignored. The projection
        // should look identical to the no-schedule case.
        let mut w = world_with_inputs(40.0, -500.0, 100.0, 200.0);
        w.timezone = "Etc/UTC".to_string();
        install_schedule(
            &mut w,
            0,
            ScheduleSpec {
                start_s: 2 * 3600,
                duration_s: 3 * 3600,
                discharge: 0,
                soc: 80.0,
                days: DAYS_DISABLED,
            },
        );
        let chart = compute_soc_chart(&w, &[], hw_50v(), TEST_NOW_MS);
        let segs = &chart.projection.segments;
        // No ScheduledCharge anywhere.
        assert!(
            segs.iter().all(|s| s.kind != ModelKind::ScheduledCharge),
            "disabled schedule must not produce a ScheduledCharge segment: {segs:?}"
        );
    }

    #[test]
    fn omits_segments_when_inputs_stale() {
        let mut w = world_with_inputs(50.0, -500.0, 100.0, 200.0);
        // SoC stale → no projection.
        w.sensors.battery_soc.freshness = Freshness::Stale;
        let chart = compute_soc_chart(&w, &[], hw_50v(), TEST_NOW_MS);
        assert!(chart.projection.segments.is_empty());
        // But targets and history pass-through are still set.
        assert_eq!(chart.projection.net_power_w, Some(-500.0));
    }

    #[test]
    fn passes_history_through() {
        let w = world_with_inputs(50.0, 1000.0, 100.0, 200.0);
        let history = vec![
            ShellSocSample { epoch_ms: 100, soc: 47.0 },
            ShellSocSample { epoch_ms: 200, soc: 48.5 },
        ];
        let chart = compute_soc_chart(&w, &history, hw_50v(), TEST_NOW_MS);
        assert_eq!(chart.history.len(), 2);
        assert_eq!(chart.history[0].epoch_ms, 100);
        assert!((chart.history[0].soc_pct - 47.0).abs() < 1e-9);
        assert_eq!(chart.now_epoch_ms, TEST_NOW_MS);
    }

    // Suppress warnings about unused `Actuated` import in case the
    // schedule installer path stops needing it.
    #[allow(dead_code)]
    fn _imports_used(_a: Actuated<ScheduleSpec>) {}
}
