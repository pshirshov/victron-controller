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
use std::time::Instant;

use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, TimeZone, Utc};

use victron_controller_core::controllers::forecast_fusion::fused_hourly_kwh;
use victron_controller_core::controllers::schedules::{DAYS_ENABLED, ScheduleSpec};
use victron_controller_core::controllers::setpoint::{
    BalanceHypothetical, BatteryBalanceBranch, SetpointInput, SetpointInputGlobals,
    compute_battery_balance,
};
use victron_controller_core::tass::{Actual, Actuated, Freshness};
use victron_controller_core::topology::{ControllerParams, HardwareParams};
use victron_controller_core::types::ForecastProvider;
use victron_controller_core::world::{ForecastSnapshot, World};

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
    controller_params: ControllerParams,
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
    // PR-soc-chart-solar: held flat across the projection horizon (no
    // EV-charging schedule yet).
    let zappi_power_w = usable(&s.evcharger_ac_power).unwrap_or(0.0);
    // PR-soc-chart-evening-consumption: live `power_consumption`
    // (already includes zappi) held flat across the horizon — matches
    // what the live setpoint controller reads. Falls back to
    // baseload_consumption_w when the sensor isn't Fresh, so projection
    // doesn't go silent on a transient stale reading.
    let live_consumption_w = usable(&s.power_consumption)
        .unwrap_or(hardware.baseload_consumption_w);

    let capacity_wh = match (installed_ah, soh_pct) {
        (Some(ah), Some(soh)) if ah > 0.0 && soh > 0.0 => {
            Some(ah * (soh / 100.0) * hardware.battery_nominal_voltage_v)
        }
        _ => None,
    };

    let charge_rate_w = derive_charge_rate_w(hardware);

    // PR-soc-chart-solar: fuse provider-level hourly forecasts into a
    // single length-48 array (kWh per hour), starting at midnight LOCAL
    // today. Empty when no provider supplied hourly data — the segment
    // walker falls back to the instantaneous battery_dc_power slope.
    let now_mono = Instant::now();
    let freshness_threshold = controller_params.freshness_forecast;
    let is_fresh_forecast = |_p: ForecastProvider, snap: &ForecastSnapshot| {
        now_mono.saturating_duration_since(snap.fetched_at) <= freshness_threshold
    };
    let hourly_kwh = fused_hourly_kwh(
        &world.typed_sensors,
        world.knobs.forecast_disagreement_strategy,
        is_fresh_forecast,
    );

    // PR-soc-chart-export-policy: build a base `SetpointInput` template
    // mirroring the live tick. The projection per-hour overrides
    // `battery_soc`, `mppt_power_*`, and `now` via `BalanceHypothetical`;
    // every other field tracks the current world state.
    let setpoint_template = build_setpoint_template_for_projection(world, hardware);

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
        hourly_kwh: &hourly_kwh,
        baseload_w: hardware.baseload_consumption_w,
        zappi_power_w,
        live_consumption_w,
        setpoint_template,
        hardware,
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

/// `max_grid_current_a * grid_nominal_voltage_v`, or None when the
/// hardware values aren't sane. This is the inverter-nameplate ceiling
/// for grid-sourced charge power; real rates may be lower if the
/// inverter throttles, the battery-side BMS limits accept current, or
/// the grid-import-limit knob caps it. We deliberately do NOT apply a
/// per-PR fixed cap on top — earlier versions used a 5000 W default
/// that was wrong for high-power inverters (e.g. MultiPlus-II 15 kVA
/// at 65 A × 230 V ≈ 15 kW).
fn derive_charge_rate_w(hardware: HardwareParams) -> Option<f64> {
    let raw = hardware.max_grid_current_a * hardware.grid_nominal_voltage_v;
    if raw.is_finite() && raw > 0.0 {
        Some(raw)
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
    /// PR-soc-chart-solar: per-hour energy estimates starting at midnight
    /// LOCAL today (length 48 when populated, empty when no hourly
    /// forecast available — fall back to instantaneous slope).
    hourly_kwh: &'a [f64],
    /// PR-soc-chart-solar: held-flat baseline consumption (W). Used as
    /// the fallback when the live `power_consumption` sensor isn't
    /// usable, AND for the `preserve_battery` gate inside
    /// `compute_battery_balance` (mirrors the live controller, which
    /// uses `baseload_consumption_w` for that gate's headroom math).
    baseload_w: f64,
    /// PR-soc-chart-solar: held-flat Zappi consumption (W).
    zappi_power_w: f64,
    /// PR-soc-chart-evening-consumption: held-flat live
    /// `power_consumption` sensor reading (already includes Zappi). Used
    /// as `input.power_consumption` so the projection's `to_be_consumed`
    /// math matches what the live setpoint controller computes —
    /// previously we used `baseload_w + zappi_power_w` which inflated
    /// the consumed-headroom estimate and tripped `preserve_battery`
    /// more aggressively than the live tick. Falls back to baseload
    /// when the sensor isn't Fresh.
    live_consumption_w: f64,
    /// PR-soc-chart-export-policy: live setpoint controller input
    /// (knobs, globals) used as the baseline for per-hour
    /// `compute_battery_balance` calls. The hour-loop overrides
    /// `battery_soc`, `mppt_power_*`, and `now` via `BalanceHypothetical`
    /// before each call.
    setpoint_template: SetpointInput,
    /// PR-soc-chart-export-policy: hardware params for
    /// `compute_battery_balance`.
    hardware: HardwareParams,
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

/// PR-soc-chart-export-policy: build a `SetpointInput` mirroring the
/// live tick. The projection per-hour overrides `battery_soc`,
/// `mppt_power_*`, and `now` via `BalanceHypothetical`; every other
/// field tracks the current world. We deliberately mirror
/// `process::build_setpoint_input` shape (it is `pub(crate)` and lives
/// in core) so the projection can never see a different knob layout
/// than the live controller.
fn build_setpoint_template_for_projection(
    world: &World,
    hardware: HardwareParams,
) -> SetpointInput {
    let _ = hardware; // capacity is read from sensors, not hardware.
    let k = &world.knobs;
    let bk = &world.bookkeeping;

    // Effective values: prefer the bookkeeping snapshot of the
    // controller's last derivation. When weather_soc-mode is in play,
    // `effective_export_soc_threshold` already reflects the current
    // policy. The projection accepts a sub-tick lag here — the chart is
    // a forecast, not a control signal.
    let export_soc_threshold = bk.effective_export_soc_threshold;
    let discharge_soc_target = bk.soc_end_of_day_target;

    SetpointInput {
        globals: SetpointInputGlobals {
            force_disable_export: k.force_disable_export,
            export_soc_threshold,
            discharge_soc_target,
            full_charge_export_soc_threshold: k.full_charge_export_soc_threshold,
            full_charge_discharge_soc_target: k.full_charge_discharge_soc_target,
            zappi_active: world.derived.zappi_active,
            allow_battery_to_car: k.allow_battery_to_car,
            discharge_time: k.discharge_time,
            debug_full_charge: k.debug_full_charge,
            pessimism_multiplier_modifier: k.pessimism_multiplier_modifier,
            next_full_charge: bk.next_full_charge,
            inverter_safe_discharge_enable: k.inverter_safe_discharge_enable,
        },
        // Use the live consumption when known so the
        // `preserve_battery` evening-discharge clamp uses a realistic
        // load. Default to baseload when missing.
        power_consumption: world
            .sensors
            .power_consumption
            .value
            .unwrap_or(hardware.baseload_consumption_w),
        // The compute_battery_balance helper overrides battery_soc via
        // BalanceHypothetical, so the value here doesn't matter; mirror
        // live for safety.
        battery_soc: world.sensors.battery_soc.value.unwrap_or(50.0),
        soh: world.sensors.battery_soh.value.unwrap_or(100.0),
        mppt_power_0: world.sensors.mppt_power_0.value.unwrap_or(0.0),
        mppt_power_1: world.sensors.mppt_power_1.value.unwrap_or(0.0),
        soltaro_power: world.sensors.soltaro_power.value.unwrap_or(0.0),
        evcharger_ac_power: world.sensors.evcharger_ac_power.value.unwrap_or(0.0),
        capacity: world
            .sensors
            .battery_installed_capacity
            .value
            .unwrap_or(0.0),
    }
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

    // PR-soc-chart-solar: anchor for hourly forecast indexing — local
    // midnight today (in the display TZ). Used by `solar_w_at` and to
    // generate hour-boundary events so each Natural-classified slice
    // gets its own per-hour solar slope.
    let local_midnight_ms = local_midnight_today_ms(inputs.now_ms, tz);

    // Build event list: now, horizon, all window starts/ends, plus
    // hour boundaries when we have hourly forecast data.
    let mut events: Vec<i64> = Vec::with_capacity(2 + windows.len() * 2 + 48);
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
    if !inputs.hourly_kwh.is_empty() {
        // Walk hour boundaries within [now, horizon]. Cap at 48 to bound
        // the loop independently of horizon_ms shenanigans.
        for h in 0..=48 {
            let edge = local_midnight_ms.saturating_add(i64::from(h) * HOUR_MS);
            if edge > inputs.now_ms && edge < horizon_ms {
                events.push(edge);
            }
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
        // PR-soc-chart-export-policy: outside any scheduled window, ask
        // the setpoint controller (via `compute_battery_balance`) what
        // the battery will do given the hypothetical (projected SoC,
        // forecast solar at this hour, hour-boundary clock). Inside a
        // window the existing schedule slope wins.
        //
        // When no hourly forecast is available, fall back to the
        // instantaneous battery_dc_power slope — without a solar
        // hypothesis we can't ask the helper anything useful.
        let (natural_net_w, branch_tag): (f64, Option<BatteryBalanceBranch>) =
            if active_window.is_some() || inputs.hourly_kwh.is_empty() {
                (net_power_w, None)
            } else {
                let solar_w = solar_w_at(midpoint, local_midnight_ms, inputs.hourly_kwh);
                let now_naive = epoch_ms_to_local_naive(midpoint, inputs.timezone_iana);
                // PR-soc-chart-evening-consumption: project consumption
                // as the live `power_consumption` sensor reading held
                // flat. This matches what `evaluate_setpoint` reads for
                // its `to_be_consumed` math; previously we used
                // baseload+zappi, which inflated the consumed-headroom
                // estimate and tripped `preserve_battery` more
                // aggressively than the live tick. Live consumption
                // already includes the EV branch (PR-rename-entities
                // doc-comment on `house.power.consumption`), so we
                // don't add zappi separately here.
                let mut input = inputs.setpoint_template;
                input.power_consumption = inputs.live_consumption_w;
                input.mppt_power_0 = solar_w / 2.0;
                input.mppt_power_1 = solar_w / 2.0;
                let bal = compute_battery_balance(
                    &input,
                    &inputs.hardware,
                    BalanceHypothetical {
                        battery_soc: soc,
                        mppt_power_total_w: solar_w,
                        now: now_naive,
                    },
                );
                (bal.net_battery_w, Some(bal.branch))
            };
        let (slope_w, kind, ceiling, floor) = classify_segment(
            active_window,
            natural_net_w,
            inputs.charge_rate_w,
            soc,
            !inputs.hourly_kwh.is_empty(),
            branch_tag,
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
///
/// PR-soc-chart-export-policy: outside any scheduled window we now
/// consult `compute_battery_balance` (passed in as `branch_tag` plus
/// the resulting `natural_net_power_w`) and map the branch tag 1:1 to
/// the wire kind. The PR-soc-chart-solar `SolarCharge`/`Drain`
/// classification is no longer produced by the projection (the variants
/// stay around for retained-payload back-compat).
///
/// `have_hourly_forecast` distinguishes the forecast-driven path from
/// the no-forecast fallback (where `branch_tag == None` and we emit
/// `Natural` so the operator can tell the projection is a flat
/// extrapolation, not a forecast-driven curve).
fn classify_segment(
    active: Option<Window>,
    natural_net_power_w: f64,
    charge_rate_w: Option<f64>,
    soc: f64,
    have_hourly_forecast: bool,
    branch_tag: Option<BatteryBalanceBranch>,
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
        return match charge_rate_w {
            Some(w) => (w, kind, win.soc_ceiling, floor),
            None => (natural_net_power_w, kind, win.soc_ceiling, floor),
        };
    }

    if let Some(tag) = branch_tag {
        // Forecast-driven path: map the helper's branch tag 1:1.
        let kind = match tag {
            BatteryBalanceBranch::ForcedNoExport => ModelKind::ForcedNoExport,
            BatteryBalanceBranch::PreserveForZappi => ModelKind::PreserveForZappi,
            BatteryBalanceBranch::BelowExportThreshold => ModelKind::BelowExportThreshold,
            BatteryBalanceBranch::EveningDischarge => ModelKind::EveningDischarge,
            BatteryBalanceBranch::BatteryFull => ModelKind::BatteryFull,
            BatteryBalanceBranch::Idle => ModelKind::Idle,
        };
        // Sub-threshold magnitude → flatten to Idle so the chart shows a
        // visibly horizontal trace (avoids drawing 1 px slopes from
        // numerical noise).
        if natural_net_power_w.abs() < SOC_IDLE_POWER_W {
            return (0.0, ModelKind::Idle, SOC_FULL_PCT, floor);
        }
        return (natural_net_power_w, kind, SOC_FULL_PCT, floor);
    }

    // No forecast available. Use the legacy classifier (battery_dc_power
    // snapshot held flat).
    if natural_net_power_w.abs() < SOC_IDLE_POWER_W {
        (0.0, ModelKind::Idle, SOC_FULL_PCT, floor)
    } else if !have_hourly_forecast {
        (natural_net_power_w, ModelKind::Natural, SOC_FULL_PCT, floor)
    } else if natural_net_power_w > 0.0 {
        (natural_net_power_w, ModelKind::SolarCharge, SOC_FULL_PCT, floor)
    } else {
        (natural_net_power_w, ModelKind::Drain, SOC_FULL_PCT, floor)
    }
}

/// PR-soc-chart-export-policy: convert `epoch_ms` to a local-clock
/// `NaiveDateTime` in the configured display TZ. The helper drops TZ
/// info because `compute_battery_balance` operates on local-clock hours
/// (matching the live setpoint controller's `clock.naive()` shape).
fn epoch_ms_to_local_naive(epoch_ms: i64, timezone_iana: &str) -> NaiveDateTime {
    let utc = Utc.timestamp_millis_opt(epoch_ms).single();
    let tz = chrono_tz::Tz::from_str(timezone_iana).unwrap_or(chrono_tz::UTC);
    match utc {
        Some(dt) => dt.with_timezone(&tz).naive_local(),
        None => NaiveDateTime::default(),
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

/// PR-soc-chart-solar: epoch-ms of midnight today in the configured
/// display TZ. Returns `now_ms` (clamped) when DST or out-of-range
/// arithmetic makes the resolution ambiguous — the chart is a forecast,
/// so a sub-hour skew during the DST fold is acceptable.
fn local_midnight_today_ms(now_ms: i64, tz: chrono_tz::Tz) -> i64 {
    let now_utc: DateTime<Utc> = match Utc.timestamp_millis_opt(now_ms).single() {
        Some(dt) => dt,
        None => return now_ms,
    };
    let now_local = now_utc.with_timezone(&tz);
    let date = now_local.date_naive();
    let local_midnight = match date.and_hms_opt(0, 0, 0) {
        Some(dt) => dt,
        None => return now_ms,
    };
    match tz.from_local_datetime(&local_midnight) {
        chrono::LocalResult::Single(dt) => dt.timestamp_millis(),
        chrono::LocalResult::Ambiguous(early, _late) => early.timestamp_millis(),
        chrono::LocalResult::None => now_ms,
    }
}

/// PR-soc-chart-solar: instantaneous solar W at `epoch_ms`, derived
/// from the fused hourly kWh array. The hourly value is treated as a
/// constant W average across the local-clock hour it covers; we resolve
/// the hour by `(epoch_ms - local_midnight_ms) / 3_600_000`.
///
/// Returns 0.0 when `epoch_ms` falls outside the array's coverage.
fn solar_w_at(epoch_ms: i64, local_midnight_ms: i64, hourly_kwh: &[f64]) -> f64 {
    if hourly_kwh.is_empty() {
        return 0.0;
    }
    let delta_ms = epoch_ms.saturating_sub(local_midnight_ms);
    if delta_ms < 0 {
        return 0.0;
    }
    let hour_idx = (delta_ms / HOUR_MS) as usize;
    if hour_idx >= hourly_kwh.len() {
        return 0.0;
    }
    // kWh per hour at constant power = (kWh × 1000) W averaged across
    // 1 hour.
    hourly_kwh[hour_idx] * 1000.0
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

    /// PR-soc-chart-solar: default ControllerParams (only `freshness_forecast`
    /// matters here — the test injects forecast snapshots with `Instant::now()`
    /// so the 12 h freshness window keeps them fresh trivially).
    fn cp() -> ControllerParams {
        ControllerParams::defaults()
    }

    /// PR-soc-chart-solar: install a single Open-Meteo hourly forecast on
    /// the world. `hourly` length should be 48 (24 today + 24 tomorrow).
    fn install_hourly_open_meteo(w: &mut World, hourly: Vec<f64>) {
        w.typed_sensors.forecast_open_meteo = Some(ForecastSnapshot {
            today_kwh: hourly.iter().take(24).sum(),
            tomorrow_kwh: hourly.iter().skip(24).sum(),
            fetched_at: Instant::now(),
            hourly_kwh: hourly,
        });
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
        let chart = compute_soc_chart(&w, &[], hw_50v(), cp(), TEST_NOW_MS);
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
        let chart = compute_soc_chart(&w, &[], hw_50v(), cp(), TEST_NOW_MS);
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
        let chart = compute_soc_chart(&w, &[], hw_50v(), cp(), TEST_NOW_MS);
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
        let chart = compute_soc_chart(&w, &[], hw_50v(), cp(), TEST_NOW_MS);
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
        let chart = compute_soc_chart(&w, &[], hw_50v(), cp(), TEST_NOW_MS);
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
        let chart = compute_soc_chart(&w, &[], hw_50v(), cp(), TEST_NOW_MS);
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
        let chart = compute_soc_chart(&w, &[], hw_50v(), cp(), TEST_NOW_MS);
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
        let chart = compute_soc_chart(&w, &history, hw_50v(), cp(), TEST_NOW_MS);
        assert_eq!(chart.history.len(), 2);
        assert_eq!(chart.history[0].epoch_ms, 100);
        assert!((chart.history[0].soc_pct - 47.0).abs() < 1e-9);
        assert_eq!(chart.now_epoch_ms, TEST_NOW_MS);
    }

    // Suppress warnings about unused `Actuated` import in case the
    // schedule installer path stops needing it.
    #[allow(dead_code)]
    fn _imports_used(_a: Actuated<ScheduleSpec>) {}

    // ------------------------------------------------------------------
    // PR-soc-chart-solar
    // ------------------------------------------------------------------

    /// Build a hand-crafted hourly profile for "today" (24 entries) and
    /// repeat it for "tomorrow" so tests are robust against the natural
    /// 24 h horizon walk crossing midnight.
    fn solar_profile_repeat(today: &[f64; 24]) -> Vec<f64> {
        let mut v = Vec::with_capacity(48);
        v.extend_from_slice(today);
        v.extend_from_slice(today);
        v
    }

    /// Force the "now" cursor to UTC midnight so hour 0 of the hourly
    /// array aligns to TEST_NOW_MS_MIDNIGHT.
    const TEST_NOW_MS_MIDNIGHT: i64 = 1_777_420_800_000; // 2026-04-25 00:00:00 UTC

    #[test]
    fn solar_curve_drives_morning_climb() {
        // PR-soc-chart-export-policy: kinds now reflect the controller's
        // export policy. Below the export threshold (soc < 80 by
        // default) the daytime branch is `BelowExportThreshold` which
        // routes all solar surplus to the battery. After sunset the
        // evening branch fires; its tag depends on SoC vs threshold and
        // the preserve_battery clamp — we just assert that SoC actually
        // moves through the day.
        //
        // 14 kWh capacity (200 Ah × 70 V), baseline 1200 W, soc 50.
        let mut hw = HardwareParams::defaults();
        hw.battery_nominal_voltage_v = 70.0; // 200 Ah * 70 V = 14000 Wh
        hw.baseload_consumption_w = 1200.0;
        let profile: [f64; 24] = [
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0,    // 00..06: night
            0.5, 1.0, 2.0, 3.0, 4.0, 5.0,    // 06..12: ramp
            5.0, 4.0, 3.0, 2.0, 1.0, 0.5,    // 12..18: ramp down
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0,    // 18..24: night
        ];
        let mut w = world_with_inputs(50.0, 0.0, 100.0, 200.0);
        w.timezone = "Etc/UTC".to_string();
        install_hourly_open_meteo(&mut w, solar_profile_repeat(&profile));
        let chart = compute_soc_chart(&w, &[], hw, cp(), TEST_NOW_MS_MIDNIGHT);
        let segs = &chart.projection.segments;
        assert!(!segs.is_empty(), "expected segments");

        // A `BelowExportThreshold` segment must appear midday (the
        // controller routes all surplus to battery while SoC < 80).
        let mut found_below = false;
        for s in segs {
            if matches!(s.kind, ModelKind::BelowExportThreshold) {
                let mid = (s.start_epoch_ms + s.end_epoch_ms) / 2;
                let hour = (mid - TEST_NOW_MS_MIDNIGHT) / HOUR_MS;
                if (10..=14).contains(&hour) {
                    found_below = true;
                    break;
                }
            }
        }
        assert!(
            found_below,
            "expected a BelowExportThreshold segment around noon, got {segs:?}"
        );

        // SoC must climb above the starting 50% at some point during
        // the day.
        let max_soc = segs
            .iter()
            .map(|s| s.end_soc_pct.max(s.start_soc_pct))
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            max_soc > 55.0,
            "SoC should climb during midday with 4-5 kWh hourly solar; max_soc={max_soc}"
        );
    }

    #[test]
    fn solar_empty_falls_back_to_natural_slope() {
        // hourly_kwh empty → must reproduce the pre-PR Natural-only
        // projection (depleting at -500 W from 50% into 10 kWh →
        // single Natural segment with Clamped tail at the floor).
        let w = world_with_inputs(50.0, -500.0, 100.0, 200.0);
        let chart = compute_soc_chart(&w, &[], hw_50v(), cp(), TEST_NOW_MS);
        let segs = &chart.projection.segments;
        // No SolarCharge / Drain — only Natural and Clamped.
        for s in segs {
            assert!(
                matches!(s.kind, ModelKind::Natural | ModelKind::Clamped),
                "expected only Natural/Clamped without forecast, got {:?} in {segs:?}",
                s.kind
            );
        }
        let first = &segs[0];
        assert_eq!(first.kind, ModelKind::Natural);
        assert!((first.start_soc_pct - 50.0).abs() < 1e-6);
    }

    #[test]
    fn solar_with_schedule_window_combines_correctly() {
        // schedule_0 02:00-05:00 UTC charges to 80%. Hourly forecast:
        // big midday solar. Expect ScheduledCharge segment 02:00-05:00
        // and SolarCharge segment(s) in the daylight band.
        let mut profile = [0.0_f64; 24];
        for v in profile.iter_mut().take(16).skip(9) {
            *v = 4.0; // 4 kWh/h → 4000 W
        }
        let mut hw = HardwareParams::defaults();
        hw.battery_nominal_voltage_v = 50.0;
        hw.baseload_consumption_w = 1200.0;
        let mut w = world_with_inputs(40.0, 0.0, 100.0, 200.0);
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
        install_hourly_open_meteo(&mut w, solar_profile_repeat(&profile));
        let chart = compute_soc_chart(&w, &[], hw, cp(), TEST_NOW_MS_MIDNIGHT);
        let segs = &chart.projection.segments;

        let has_sched = segs.iter().any(|s| s.kind == ModelKind::ScheduledCharge);
        // PR-soc-chart-export-policy: daytime non-schedule hours below
        // the 80% threshold produce `BelowExportThreshold` (controller
        // routes solar surplus into the battery), not the legacy
        // `SolarCharge`.
        let has_branch = segs
            .iter()
            .any(|s| s.kind == ModelKind::BelowExportThreshold);
        assert!(has_sched, "expected ScheduledCharge in {segs:?}");
        assert!(has_branch, "expected BelowExportThreshold in {segs:?}");

        // Check that the ScheduledCharge segment(s) live inside
        // [02:00, 05:00] UTC.
        for s in segs {
            if s.kind == ModelKind::ScheduledCharge {
                let start_hour = (s.start_epoch_ms - TEST_NOW_MS_MIDNIGHT) / HOUR_MS;
                let end_hour = (s.end_epoch_ms - TEST_NOW_MS_MIDNIGHT) / HOUR_MS;
                assert!(
                    start_hour >= 2 && end_hour <= 5,
                    "ScheduledCharge outside [02,05]: {s:?}"
                );
            }
        }
    }

    #[test]
    #[ignore = "PR-soc-chart-export-policy: replaced by compute_battery_balance unit tests in setpoint.rs; first-hour classification depends on the export-policy branch tree, not a simple net-power threshold"]
    fn solar_threshold_classifies_kinds() {
        // Use a 1-h hourly profile and check the classification for the
        // first hour by inspecting the produced segments.
        let mut hw = HardwareParams::defaults();
        hw.battery_nominal_voltage_v = 50.0;
        hw.baseload_consumption_w = 1000.0;

        // Case 1: solar_w = 1500 → net = +500 → SolarCharge.
        // 1.5 kWh/h * 1000 = 1500 W, baseline 1000 W → net 500 W.
        let mut prof = [0.0_f64; 24];
        prof[0] = 1.5;
        let mut w = world_with_inputs(50.0, 0.0, 100.0, 200.0);
        w.timezone = "Etc/UTC".to_string();
        install_hourly_open_meteo(&mut w, solar_profile_repeat(&prof));
        let chart = compute_soc_chart(&w, &[], hw, cp(), TEST_NOW_MS_MIDNIGHT);
        let first = &chart.projection.segments[0];
        assert_eq!(first.kind, ModelKind::SolarCharge);

        // Case 2: solar_w = 500 → net = -500 → Drain.
        let mut prof = [0.0_f64; 24];
        prof[0] = 0.5;
        let mut w = world_with_inputs(50.0, 0.0, 100.0, 200.0);
        w.timezone = "Etc/UTC".to_string();
        install_hourly_open_meteo(&mut w, solar_profile_repeat(&prof));
        let chart = compute_soc_chart(&w, &[], hw, cp(), TEST_NOW_MS_MIDNIGHT);
        let first = &chart.projection.segments[0];
        assert_eq!(first.kind, ModelKind::Drain);

        // Case 3: solar_w = 1030 → net = +30 → Idle (|net| < 50 W).
        let mut prof = [0.0_f64; 24];
        prof[0] = 1.030;
        let mut w = world_with_inputs(50.0, 0.0, 100.0, 200.0);
        w.timezone = "Etc/UTC".to_string();
        install_hourly_open_meteo(&mut w, solar_profile_repeat(&prof));
        let chart = compute_soc_chart(&w, &[], hw, cp(), TEST_NOW_MS_MIDNIGHT);
        let first = &chart.projection.segments[0];
        assert_eq!(first.kind, ModelKind::Idle);
    }

    // ------------------------------------------------------------------
    // PR-soc-chart-export-policy
    // ------------------------------------------------------------------

    #[test]
    fn projection_evening_discharge_drops_soc() {
        // High SoC, evening hour, no schedule window → expect at least
        // one EveningDischarge segment that drops SoC.
        let mut hw = HardwareParams::defaults();
        hw.battery_nominal_voltage_v = 50.0;
        hw.baseload_consumption_w = 800.0;
        let prof = [0.0_f64; 24];
        // Start at UTC 19:00 — evening discharge window.
        const EVENING_NOW_MS: i64 = 1_777_420_800_000 + 19 * HOUR_MS;
        let mut w = world_with_inputs(95.0, 0.0, 100.0, 800.0); // big battery
        w.timezone = "Etc/UTC".to_string();
        // Make sure the bookkeeping export threshold (template input) is
        // 70 so SoC=95 > threshold and the discharge branch fires.
        w.bookkeeping.effective_export_soc_threshold = 70.0;
        w.bookkeeping.soc_end_of_day_target = 25.0;
        install_hourly_open_meteo(&mut w, solar_profile_repeat(&prof));
        let chart = compute_soc_chart(&w, &[], hw, cp(), EVENING_NOW_MS);
        let segs = &chart.projection.segments;
        let has_disc = segs
            .iter()
            .any(|s| s.kind == ModelKind::EveningDischarge);
        assert!(
            has_disc,
            "expected an EveningDischarge segment, got {segs:?}"
        );
        let dropped = segs
            .iter()
            .any(|s| s.kind == ModelKind::EveningDischarge && s.end_soc_pct < s.start_soc_pct);
        assert!(dropped, "expected discharge to drop SoC, got {segs:?}");
    }

    #[test]
    fn projection_below_threshold_charges_only_from_solar_surplus() {
        // SoC just below threshold, midday solar 5 kW, baseline 1.2 kW
        // → expect BelowExportThreshold segment whose slope reflects
        // (5000 - max(1200, baseload+zappi)) / capacity.
        let mut hw = HardwareParams::defaults();
        hw.battery_nominal_voltage_v = 50.0;
        hw.baseload_consumption_w = 1200.0;
        let mut prof = [0.0_f64; 24];
        prof[12] = 5.0; // 5 kWh at hour 12 → 5000 W
        const NOON_NOW_MS: i64 = 1_777_420_800_000 + 12 * HOUR_MS;
        let mut w = world_with_inputs(65.0, 0.0, 100.0, 200.0); // 10 kWh
        w.timezone = "Etc/UTC".to_string();
        w.bookkeeping.effective_export_soc_threshold = 70.0;
        install_hourly_open_meteo(&mut w, solar_profile_repeat(&prof));
        let chart = compute_soc_chart(&w, &[], hw, cp(), NOON_NOW_MS);
        let segs = &chart.projection.segments;
        let first = &segs[0];
        assert_eq!(first.kind, ModelKind::BelowExportThreshold);
        // Drift-guard against the helper: the projection's first hour
        // must use the same net_battery_w as compute_battery_balance.
        let template = build_setpoint_template_for_projection(&w, hw);
        let mut input = template;
        input.power_consumption = hw.baseload_consumption_w; // zappi = 0
        input.mppt_power_0 = 5000.0 / 2.0;
        input.mppt_power_1 = 5000.0 / 2.0;
        let bal = compute_battery_balance(
            &input,
            &hw,
            BalanceHypothetical {
                battery_soc: 65.0,
                mppt_power_total_w: 5000.0,
                now: epoch_ms_to_local_naive(
                    NOON_NOW_MS + (first.end_epoch_ms - first.start_epoch_ms) / 2,
                    "Etc/UTC",
                ),
            },
        );
        // Expected: 5000 - 1200 = 3800 W.
        assert!(
            (bal.net_battery_w - 3800.0).abs() < 1e-6,
            "expected 3800 W net, got {}",
            bal.net_battery_w
        );
        assert_eq!(bal.branch, BatteryBalanceBranch::BelowExportThreshold);
    }

    #[test]
    fn projection_battery_full_clamps() {
        // SoC = 100 → BatteryFull segment; SoC stays at 100.
        let mut hw = HardwareParams::defaults();
        hw.battery_nominal_voltage_v = 50.0;
        hw.baseload_consumption_w = 800.0;
        let mut prof = [0.0_f64; 24];
        prof[12] = 5.0;
        const NOON_NOW_MS: i64 = 1_777_420_800_000 + 12 * HOUR_MS;
        let mut w = world_with_inputs(100.0, 0.0, 100.0, 200.0);
        w.timezone = "Etc/UTC".to_string();
        install_hourly_open_meteo(&mut w, solar_profile_repeat(&prof));
        let chart = compute_soc_chart(&w, &[], hw, cp(), NOON_NOW_MS);
        let segs = &chart.projection.segments;
        let first = &segs[0];
        // Branch fires for hour 12 with SoC=100 → BatteryFull. The
        // projection currently routes BatteryFull through the same
        // emission path as other branch tags; |net|=0 collapses to Idle
        // for visual smoothness. Either is acceptable as long as SoC
        // doesn't go above 100.
        assert!(
            matches!(first.kind, ModelKind::BatteryFull | ModelKind::Idle),
            "expected BatteryFull/Idle at SoC=100, got {:?}",
            first.kind
        );
        assert!((first.start_soc_pct - 100.0).abs() < 1e-6);
        assert!(first.end_soc_pct <= 100.0 + 1e-6);
    }

    #[test]
    fn drift_guard_projection_matches_helper() {
        // Load-bearing regression check: build identical worlds and
        // compute the projection's per-hour `compute_battery_balance`
        // call versus a direct call. They must agree exactly.
        let mut hw = HardwareParams::defaults();
        hw.battery_nominal_voltage_v = 50.0;
        hw.baseload_consumption_w = 1200.0;
        let mut prof = [0.0_f64; 24];
        prof[12] = 4.0;
        let mut w = world_with_inputs(60.0, 0.0, 100.0, 200.0);
        w.timezone = "Etc/UTC".to_string();
        w.bookkeeping.effective_export_soc_threshold = 70.0;
        install_hourly_open_meteo(&mut w, solar_profile_repeat(&prof));

        // Direct call.
        const NOON_NOW_MS: i64 = 1_777_420_800_000 + 12 * HOUR_MS;
        let template = build_setpoint_template_for_projection(&w, hw);
        let mut input = template;
        input.power_consumption = hw.baseload_consumption_w;
        input.mppt_power_0 = 4000.0 / 2.0;
        input.mppt_power_1 = 4000.0 / 2.0;
        let direct = compute_battery_balance(
            &input,
            &hw,
            BalanceHypothetical {
                battery_soc: 60.0,
                mppt_power_total_w: 4000.0,
                now: epoch_ms_to_local_naive(
                    NOON_NOW_MS + 30 * 60 * 1000,
                    "Etc/UTC",
                ),
            },
        );

        // Projection call: first hour starts at NOON_NOW_MS.
        let chart = compute_soc_chart(&w, &[], hw, cp(), NOON_NOW_MS);
        let first = &chart.projection.segments[0];
        // Slope reconstruction: chart used capacity = 200*1.0*50 = 10000 Wh.
        let dur_h = (first.end_epoch_ms - first.start_epoch_ms) as f64 / HOUR_MS as f64;
        let observed_slope_pct_per_hour =
            (first.end_soc_pct - first.start_soc_pct) / dur_h;
        let expected_slope_pct_per_hour = direct.net_battery_w / 10_000.0 * 100.0;
        assert!(
            (observed_slope_pct_per_hour - expected_slope_pct_per_hour).abs() < 1e-6,
            "drift: projection slope {observed_slope_pct_per_hour} vs helper {expected_slope_pct_per_hour}"
        );
    }
}
