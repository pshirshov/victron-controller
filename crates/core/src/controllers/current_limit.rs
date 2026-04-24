//! Current-limit controller. 1:1 port of the `compute limit` function from
//! the Node-RED "Current limit" tab (see
//! `legacy/debug/20260421-120100-functions.txt`).
//!
//! Writes `/Ac/In/1/CurrentLimit` on `com.victronenergy.vebus`. The value
//! is a gross AC-input current cap in amps, clamped to `[0, 65]`.
//!
//! Three interacting sub-problems:
//!
//! 1. Classify whether the Zappi is actively drawing power (`zappi_active`).
//! 2. Compute a "fitted current" that leaves the Zappi its configured
//!    target amps plus an optional startup margin.
//! 3. Branch on tariff band to choose the final cap: Boost /
//!    NightExtended-if-enabled / Day / Evening.
//!
//! Several values the NR version recorded in `msg.payload.debug` are
//! preserved in [`CurrentLimitDebug`] for the dashboard + golden replay.

use crate::Clock;
use crate::controllers::tariff_band::{TariffBand, tariff_band};
use crate::myenergi::{ZappiMode, ZappiPlugState, ZappiState};
use crate::types::Decision;

// --- Constants ---

/// House service's main-breaker headroom in amps.
const MAX_GRID_CURRENT_A: f64 = 65.0;
/// Never cap below this — the system always gets at least this many amps.
const MIN_SYSTEM_CURRENT_A: f64 = 10.0;
/// Waiting-for-EV timeout (minutes) after which we treat Zappi as inactive.
const WAIT_TIMEOUT_MIN: f64 = 5.0;
/// Margin of the `zappi_amps > N` fallback that triggers `zappi_active`
/// even when the state machine disagrees. Matches legacy NR flow.
const ZAPPI_AMPS_FALLBACK_THRESHOLD: f64 = 1.0;
/// Lower sanity bound on grid voltage. EN 50160 caps legitimate readings
/// at -10% of nominal (207 V). Below this we treat the measurement as
/// grid loss / sensor glitch / NaN and fall back to [`NOMINAL_GRID_V`].
const MIN_SENSIBLE_GRID_V: f64 = 207.0;
/// Upper sanity bound on grid voltage. EN 50160 caps legitimate
/// readings at +10% of nominal (253 V) but we allow a 7 V headroom
/// band above that for benign sensor noise and short transient
/// surges (observed on the user's ET340 during kettle-start spikes).
/// Anything > 260 is treated as a glitch and falls back to
/// NOMINAL_GRID_V. PR-02-D08: the 260 ceiling vs 253 SPEC spec was
/// previously a comment/code mismatch — now explained explicitly.
const MAX_SENSIBLE_GRID_V: f64 = 260.0;
/// UK nominal mains voltage — used when the measured value is unusable.
const NOMINAL_GRID_V: f64 = 230.0;

/// Sanity gate for grid-voltage-based arithmetic.
/// Returns (effective_voltage, fell_back).
fn effective_grid_v(measured: f64) -> (f64, bool) {
    if !measured.is_finite() || !(MIN_SENSIBLE_GRID_V..=MAX_SENSIBLE_GRID_V).contains(&measured) {
        (NOMINAL_GRID_V, true)
    } else {
        (measured, false)
    }
}

/// Inputs — all D-Bus sensor values plus cross-cutting globals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurrentLimitInput {
    pub globals: CurrentLimitInputGlobals,
    pub consumption_power: f64,
    pub offgrid_power: f64,
    pub offgrid_current: f64,
    pub grid_voltage: f64,
    pub grid_power: f64,
    pub mppt_power_0: f64,
    pub mppt_power_1: f64,
    pub soltaro_power: f64,
    pub zappi_current: f64,
    pub ess_state: i32,
    pub battery_power: f64,
    pub battery_soc: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct CurrentLimitInputGlobals {
    pub zappi_current_target: f64,
    pub zappi_emergency_margin: f64,
    pub zappi_state: ZappiState,
    /// PR-DAG-B: read from `world.derived.zappi_active`, which
    /// `ZappiActiveCore` writes once per tick via
    /// [`crate::controllers::zappi_active::classify_zappi_active`] —
    /// the single source of truth every consumer shares.
    pub zappi_active: bool,
    pub extended_charge_required: bool,
    pub disable_night_grid_discharge: bool,
    pub battery_soc_target: f64,
    pub prev_ess_state: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurrentLimitOutput {
    /// Target value to write to `/Ac/In/1/CurrentLimit`.
    pub input_current_limit: f64,
    pub debug: CurrentLimitDebug,
    pub bookkeeping: CurrentLimitBookkeeping,
    pub decision: Decision,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct CurrentLimitDebug {
    pub tariff: TariffBand,
    pub battery_charged: bool,
    pub battery_charging: bool,
    pub zappi_active: bool,
    pub zappi_wait_timeout: bool,
    pub zappi_amps: f64,
    pub zappi_overuse: f64,
    pub zappi_underuse: f64,
    pub grid_current: f64,
    pub grid_underuse: f64,
    pub available_pv_power: f64,
    pub available_pv_power_as_gridside_amps: f64,
    pub gridside_consumption_power: f64,
    pub gridside_consumption_current: f64,
    pub gridside_consumption_no_zappi: f64,
    pub fitted_target: f64,
    pub max_system_current: f64,
    pub prev_ess_state: Option<i32>,
    pub ess_state: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurrentLimitBookkeeping {
    /// Updated prev_ess_state (unchanged unless ess_state != 9 and changed).
    pub prev_ess_state: Option<i32>,
}

/// Round a value to 2 decimal places — matches the legacy `tools.round2`.
fn round2(f: f64) -> f64 {
    (f * 100.0).round() / 100.0
}

/// Evaluate the input-current-limit for the current moment.
#[must_use]
pub fn evaluate_current_limit(
    input: &CurrentLimitInput,
    clock: &dyn Clock,
) -> CurrentLimitOutput {
    let g = &input.globals;
    let now = clock.naive();
    let now_mono = clock.monotonic();

    let mppt_power = input.mppt_power_0 + input.mppt_power_1;
    let soltaro_power = input.soltaro_power;
    let zappi_amps = input.zappi_current;

    // prev_ess_state update. Legacy: only update when ess_state changed AND
    // the new state isn't 9 (KeepBatteriesCharged — this is the state we'd
    // have forced, so don't "remember" it as the pre-override state).
    let ess_state = input.ess_state;
    let prev_ess_state = if Some(ess_state) != g.prev_ess_state && ess_state != 9 {
        Some(ess_state)
    } else {
        g.prev_ess_state
    };

    let battery_soc = input.battery_soc;
    let battery_charging = input.battery_power > 0.0;
    let battery_charged = battery_soc >= g.battery_soc_target - 1.0;

    // NOTE: we deliberately derive `grid_current` from `grid_power / grid_voltage`
    // rather than consuming the direct `/Ac/L1/Current` reading from the ET112
    // meter. The ET112 current sensor on this install is known to report
    // phantom current (non-zero A with near-zero real power), so the derived
    // form is the trusted source. `grid_power` comes from the system-aggregate
    // `/Ac/Grid/L1/Power` which is reliable. `v_eff` is the sanity-gated
    // grid voltage (see `effective_grid_v`) — bias-to-safety 230 V nominal
    // if the meter reports something physically implausible. If you are
    // tempted to "simplify" by using the direct grid_current sensor, DON'T:
    // it will silently report ghost amps and starve the controller.
    let (v_eff, grid_v_fell_back) = effective_grid_v(input.grid_voltage);
    let grid_current = input.grid_power / v_eff;
    let grid_underuse = (MAX_GRID_CURRENT_A - grid_current).ceil().max(0.0);

    // --- Zappi activity classification ---
    // PR-DAG-B: `zappi_active` flows in via
    // [`CurrentLimitInputGlobals::zappi_active`], sourced from
    // `world.derived.zappi_active` which `ZappiActiveCore` wrote at the
    // top of the tick — single source of truth for every consumer.
    let ZappiState {
        zappi_mode,
        zappi_plug_state,
        zappi_last_change_signature,
        ..
    } = g.zappi_state;
    let zappi_active = g.zappi_active;

    // `zappi_wait_timeout` stays a local computation — it's surfaced
    // in the debug tuple for the dashboard but is not part of the
    // canonical classifier's contract. A-04/A-24: delta is measured on
    // monotonic `Instant`s stamped by the poller.
    let time_in_state_min =
        now_mono.duration_since(zappi_last_change_signature).as_secs_f64() / 60.0;
    let zappi_wait_timeout = time_in_state_min > WAIT_TIMEOUT_MIN
        && zappi_plug_state == ZappiPlugState::WaitingForEv;

    let zappi_overuse = (zappi_amps - g.zappi_current_target).max(0.0);
    let zappi_underuse = (g.zappi_current_target - zappi_amps).max(0.0);

    // --- PV availability ---
    let full_pv_power = mppt_power + soltaro_power;
    let available_pv_power = (full_pv_power - input.offgrid_power).max(0.0);
    let available_pv_current = round2(available_pv_power / v_eff);
    let available_pv_power_as_gridside_amps = available_pv_current;

    // --- Grid-side load accounting ---
    // Soltaro charging from grid counts as extra load on the grid side.
    let soltaro_inflow_power = if soltaro_power < 0.0 {
        -soltaro_power
    } else {
        0.0
    };
    let gridside_consumption_power =
        input.consumption_power - input.offgrid_power + soltaro_inflow_power;
    let gridside_consumption_current = round2(gridside_consumption_power / v_eff);

    let tariff = tariff_band(now);
    let is_boost = tariff == TariffBand::BOOST;
    let is_extended_charge = tariff == TariffBand::NIGHT_EXTENDED;
    let is_enabled_extended_charge = is_extended_charge && g.extended_charge_required;

    let gridside_consumption_no_zappi = gridside_consumption_current - zappi_amps;

    // --- fit_current() ---
    let (fitted_target, max_system_current) = fit_current(
        zappi_active,
        zappi_mode,
        zappi_amps,
        g.zappi_current_target,
        g.zappi_emergency_margin,
        gridside_consumption_current,
        gridside_consumption_no_zappi,
        grid_underuse,
    );

    // --- compute_limit() ---
    let (target, branch): (f64, &'static str) = if is_boost || is_enabled_extended_charge {
        if battery_charging {
            (fitted_target, "boost/extended window + battery charging → fitted current")
        } else if zappi_active {
            (input.offgrid_current, "boost/extended + Zappi active but battery not charging → cap at offgrid current")
        } else if g.disable_night_grid_discharge {
            (input.offgrid_current, "boost/extended + disable_night_grid_discharge → cap at offgrid current")
        } else {
            (MAX_GRID_CURRENT_A, "boost/extended + idle → full grid (65 A)")
        }
    } else if zappi_active {
        (available_pv_power_as_gridside_amps, "outside charge window + Zappi active → cap at PV availability")
    } else if g.disable_night_grid_discharge && is_extended_charge {
        (input.offgrid_current, "extended window + disable_night_grid_discharge → cap at offgrid current")
    } else {
        (MAX_GRID_CURRENT_A, "idle / default → full grid (65 A)")
    };

    let input_current_limit = target.clamp(0.0, MAX_GRID_CURRENT_A);

    let mut decision = Decision::new(branch)
        .with_factor("tariff", format!("{tariff:?}"))
        .with_factor("battery_charging", format!("{battery_charging}"))
        .with_factor("zappi_active", format!("{zappi_active}"))
        .with_factor("extended_charge_required", format!("{}", g.extended_charge_required))
        .with_factor("disable_night_grid_discharge", format!("{}", g.disable_night_grid_discharge))
        .with_factor("offgrid_current_A", format!("{:.2}", input.offgrid_current))
        .with_factor("available_pv_A", format!("{available_pv_power_as_gridside_amps:.2}"))
        .with_factor("fitted_target_A", format!("{fitted_target:.2}"))
        .with_factor("final_limit_A", format!("{input_current_limit:.2}"));
    if grid_v_fell_back {
        decision = decision.with_factor(
            "grid_v_fallback",
            format!("{:.2}V → {NOMINAL_GRID_V:.2}V", input.grid_voltage),
        );
    }

    CurrentLimitOutput {
        input_current_limit,
        debug: CurrentLimitDebug {
            tariff,
            battery_charged,
            battery_charging,
            zappi_active,
            zappi_wait_timeout,
            zappi_amps,
            zappi_overuse,
            zappi_underuse,
            grid_current,
            grid_underuse,
            available_pv_power,
            available_pv_power_as_gridside_amps,
            gridside_consumption_power,
            gridside_consumption_current,
            gridside_consumption_no_zappi,
            fitted_target,
            max_system_current,
            prev_ess_state,
            ess_state,
        },
        bookkeeping: CurrentLimitBookkeeping {
            prev_ess_state,
        },
        decision,
    }
}

/// Port of the legacy `fit_current()` inner function.
#[allow(clippy::too_many_arguments)]
fn fit_current(
    zappi_active: bool,
    zappi_mode: ZappiMode,
    zappi_amps: f64,
    zappi_current_target: f64,
    zappi_emergency_margin: f64,
    gridside_consumption_current: f64,
    gridside_consumption_no_zappi: f64,
    grid_underuse: f64,
) -> (f64, f64) {
    let (max_system_current, out_limit) = if (zappi_active && zappi_mode == ZappiMode::Fast)
        || zappi_amps > ZAPPI_AMPS_FALLBACK_THRESHOLD
    {
        let max_sys = MAX_GRID_CURRENT_A - zappi_current_target;
        let mut ol = round2(MAX_GRID_CURRENT_A - gridside_consumption_no_zappi - zappi_current_target);
        // Additional margin when zappi hasn't reached its target yet.
        if zappi_amps <= zappi_current_target - 1.0 {
            ol -= zappi_emergency_margin;
        }
        (max_sys, ol)
    } else {
        (
            MAX_GRID_CURRENT_A,
            round2(MAX_GRID_CURRENT_A - gridside_consumption_current),
        )
    };

    let relaxed_limit = out_limit + grid_underuse;
    let target = relaxed_limit.clamp(MIN_SYSTEM_CURRENT_A, max_system_current);
    (target, max_system_current)
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use crate::myenergi::ZappiStatus;
    use chrono::NaiveDate;
    use std::time::{Duration as StdDuration, Instant};

    /// A fixed monotonic anchor for tests — far enough from `Instant::now()`
    /// that we can safely subtract large durations without going negative.
    fn fixed_mono_anchor() -> Instant {
        Instant::now() + StdDuration::from_secs(86_400)
    }

    fn clock_at(h: u32, m: u32) -> FixedClock {
        let nt = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap();
        FixedClock::new(fixed_mono_anchor(), nt)
    }

    fn base_zappi_state() -> ZappiState {
        ZappiState {
            zappi_mode: ZappiMode::Off,
            zappi_plug_state: ZappiPlugState::EvDisconnected,
            zappi_status: ZappiStatus::Paused,
            // 1 min ago — arbitrary pre-tick stamp.
            zappi_last_change_signature: fixed_mono_anchor()
                .checked_sub(StdDuration::from_secs(60))
                .unwrap(),
        }
    }

    fn base_input() -> CurrentLimitInput {
        CurrentLimitInput {
            globals: CurrentLimitInputGlobals {
                zappi_current_target: 9.5,
                zappi_emergency_margin: 5.0,
                zappi_state: base_zappi_state(),
                zappi_active: false,
                extended_charge_required: false,
                disable_night_grid_discharge: false,
                battery_soc_target: 80.0,
                prev_ess_state: Some(10),
            },
            consumption_power: 500.0,
            offgrid_power: 500.0,
            offgrid_current: 2.0,
            grid_voltage: 230.0,
            grid_power: 0.0,
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            zappi_current: 0.0,
            ess_state: 10,
            battery_power: 0.0,
            battery_soc: 80.0,
        }
    }

    // ------------------------------------------------------------------
    // zappi_active propagation
    //
    // Classification itself lives in `controllers::zappi_active`; these
    // tests only verify that the flag passed in via globals reaches
    // the debug/bookkeeping outputs and drives the branch correctly.
    // ------------------------------------------------------------------

    #[test]
    fn zappi_active_true_propagates_to_debug() {
        let mut input = base_input();
        input.globals.zappi_active = true;
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!(out.debug.zappi_active);
    }

    #[test]
    fn zappi_wait_timeout_still_surfaced_in_debug() {
        let mut input = base_input();
        let clk = clock_at(12, 0);
        let six_min_ago = clk
            .monotonic
            .checked_sub(StdDuration::from_secs(6 * 60))
            .unwrap();
        input.globals.zappi_state = ZappiState {
            zappi_mode: ZappiMode::Eco,
            zappi_plug_state: ZappiPlugState::WaitingForEv,
            zappi_status: ZappiStatus::Paused,
            zappi_last_change_signature: six_min_ago,
        };
        // zappi_active is independently supplied; debug should still
        // report the wait-timeout derived locally from state.
        input.globals.zappi_active = false;
        let out = evaluate_current_limit(&input, &clk);
        assert!(out.debug.zappi_wait_timeout);
    }

    // ------------------------------------------------------------------
    // Tariff branches
    // ------------------------------------------------------------------

    #[test]
    fn daytime_with_no_zappi_allows_full_grid() {
        let input = base_input();
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!((out.input_current_limit - 65.0).abs() < f64::EPSILON);
    }

    #[test]
    fn daytime_with_zappi_active_caps_to_pv_current() {
        let mut input = base_input();
        input.mppt_power_0 = 1500.0;
        input.mppt_power_1 = 1500.0;
        input.offgrid_power = 500.0;
        input.grid_voltage = 230.0;
        input.zappi_current = 5.0;
        input.globals.zappi_active = true;
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        // available_pv = 3000 - 500 = 2500; /230 ≈ 10.87; clamped [0,65]
        assert!(out.input_current_limit > 0.0);
        assert!(out.input_current_limit < MAX_GRID_CURRENT_A);
        assert!(out.debug.zappi_active);
    }

    #[test]
    fn boost_window_with_battery_charging_uses_fitted_current() {
        let mut input = base_input();
        input.battery_power = 1000.0; // charging
        let out = evaluate_current_limit(&input, &clock_at(3, 0));
        assert_eq!(out.debug.tariff, TariffBand::BOOST);
        assert!(out.input_current_limit > 0.0);
        assert!(out.input_current_limit <= MAX_GRID_CURRENT_A);
    }

    #[test]
    fn boost_with_zappi_active_but_battery_not_charging_caps_to_offgrid() {
        let mut input = base_input();
        input.battery_power = 0.0; // not charging
        input.offgrid_current = 3.0;
        input.zappi_current = 5.0;
        input.globals.zappi_active = true;
        let out = evaluate_current_limit(&input, &clock_at(3, 0));
        assert!((out.input_current_limit - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn boost_with_disable_night_grid_discharge_caps_to_offgrid() {
        let mut input = base_input();
        input.battery_power = 0.0;
        input.globals.disable_night_grid_discharge = true;
        input.offgrid_current = 4.0;
        let out = evaluate_current_limit(&input, &clock_at(3, 30));
        assert!((out.input_current_limit - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn extended_night_disabled_allows_full_grid() {
        let input = base_input();
        let out = evaluate_current_limit(&input, &clock_at(6, 30));
        assert_eq!(out.debug.tariff, TariffBand::NIGHT_EXTENDED);
        assert!((out.input_current_limit - 65.0).abs() < f64::EPSILON);
    }

    #[test]
    fn extended_night_enabled_with_battery_charging_uses_fitted() {
        let mut input = base_input();
        input.globals.extended_charge_required = true;
        input.battery_power = 500.0; // charging
        let out = evaluate_current_limit(&input, &clock_at(6, 30));
        assert!(out.input_current_limit > 0.0);
        assert!(out.input_current_limit <= MAX_GRID_CURRENT_A);
    }

    #[test]
    fn extended_night_with_disable_night_grid_discharge_uses_offgrid() {
        let mut input = base_input();
        input.globals.disable_night_grid_discharge = true;
        input.offgrid_current = 5.0;
        let out = evaluate_current_limit(&input, &clock_at(6, 0));
        assert!((out.input_current_limit - 5.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // Clamping and bookkeeping
    // ------------------------------------------------------------------

    #[test]
    fn result_always_clamped_to_0_65() {
        let mut input = base_input();
        // Offgrid_current of 1000 to try to push limit out of range
        input.offgrid_current = 1000.0;
        input.globals.disable_night_grid_discharge = true;
        let out = evaluate_current_limit(&input, &clock_at(6, 0));
        assert!(out.input_current_limit >= 0.0);
        assert!(out.input_current_limit <= MAX_GRID_CURRENT_A);
    }

    #[test]
    fn prev_ess_state_updates_on_change_ignoring_9() {
        let mut input = base_input();
        // First call: ess_state moves from 10 → 10 (no change, keeps prev 10)
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert_eq!(out.bookkeeping.prev_ess_state, Some(10));

        // Second call: ess_state moves 10 → 9 (override state — ignored,
        // prev should stay at 10)
        input.ess_state = 9;
        input.globals.prev_ess_state = Some(10);
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert_eq!(out.bookkeeping.prev_ess_state, Some(10));

        // Third call: ess_state moves 9 → 5 (new non-9 value captured)
        input.ess_state = 5;
        input.globals.prev_ess_state = Some(10);
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert_eq!(out.bookkeeping.prev_ess_state, Some(5));
    }

    #[test]
    fn zappi_active_is_surfaced_in_debug() {
        // PR-DAG-B: `zappi_active` is no longer part of the controller's
        // bookkeeping output (that field was hosted by `world.bookkeeping`
        // and is now owned by `ZappiActiveCore` via `world.derived`).
        // The debug field is still populated for dashboard parity.
        let mut input = base_input();
        input.globals.zappi_active = true;
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!(out.debug.zappi_active);
    }

    // ------------------------------------------------------------------
    // fit_current behavior
    // ------------------------------------------------------------------

    #[test]
    fn fit_current_in_fast_mode_subtracts_zappi_target() {
        let mut input = base_input();
        let clk = clock_at(3, 0);
        input.globals.zappi_state = ZappiState {
            zappi_mode: ZappiMode::Fast,
            zappi_plug_state: ZappiPlugState::Charging,
            zappi_status: ZappiStatus::DivertingOrCharging,
            zappi_last_change_signature: clk
                .monotonic
                .checked_sub(StdDuration::from_secs(60))
                .unwrap(),
        };
        input.zappi_current = 9.5;
        input.globals.zappi_active = true;
        input.battery_power = 1000.0; // charging, so fitted target is used
        let out = evaluate_current_limit(&input, &clk);
        // max_system_current becomes 65 - 9.5 = 55.5
        assert!((out.debug.max_system_current - 55.5).abs() < 0.01);
        assert!(out.input_current_limit <= 55.5);
    }

    #[test]
    fn fit_current_adds_emergency_margin_when_zappi_ramping() {
        let mut input = base_input();
        let clk = clock_at(3, 0);
        input.globals.zappi_state = ZappiState {
            zappi_mode: ZappiMode::Fast,
            zappi_plug_state: ZappiPlugState::Charging,
            zappi_status: ZappiStatus::DivertingOrCharging,
            zappi_last_change_signature: clk
                .monotonic
                .checked_sub(StdDuration::from_secs(60))
                .unwrap(),
        };
        input.zappi_current = 2.0; // well below zappi_current_target-1 = 8.5
        input.globals.zappi_active = true;
        input.globals.zappi_emergency_margin = 5.0;
        input.battery_power = 1000.0;
        input.consumption_power = 1000.0;
        input.offgrid_power = 0.0;
        let out_ramping = evaluate_current_limit(&input, &clk);

        // Change to zappi_amps above threshold to disable margin
        input.zappi_current = 9.5;
        let out_steady = evaluate_current_limit(&input, &clk);

        assert!(
            out_ramping.input_current_limit <= out_steady.input_current_limit + 0.001,
            "ramping should have lower (or equal) limit due to emergency margin"
        );
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    // ------------------------------------------------------------------
    // Grid-voltage ÷0 guard (defect A-03)
    // ------------------------------------------------------------------

    fn has_factor(dec: &Decision, name: &str) -> bool {
        dec.factors.iter().any(|f| f.name == name)
    }

    #[test]
    fn current_limit_grid_v_fallback_on_grid_loss() {
        let mut input = base_input();
        input.grid_voltage = 0.0;
        input.grid_power = 1000.0;
        input.mppt_power_0 = 1500.0;
        input.mppt_power_1 = 1500.0;
        input.offgrid_power = 500.0;
        input.zappi_current = 5.0;
        input.globals.zappi_active = true; // force PV branch
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!(out.input_current_limit.is_finite());
        assert!(out.debug.grid_current.is_finite());
        assert!(out.debug.available_pv_power_as_gridside_amps.is_finite());
        assert!(out.debug.gridside_consumption_current.is_finite());
        assert!(has_factor(&out.decision, "grid_v_fallback"));
        let expected_current = input.grid_power / 230.0;
        assert!(
            (out.debug.grid_current - expected_current).abs() < f64::EPSILON,
            "fallback must use NOMINAL_GRID_V (230.0V)"
        );
    }

    #[test]
    fn current_limit_no_grid_v_fallback_on_nominal() {
        let mut input = base_input();
        input.grid_voltage = 240.0;
        input.grid_power = 2400.0;
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!(!has_factor(&out.decision, "grid_v_fallback"));
        // Pre-PR arithmetic: grid_current = 2400 / 240 = 10.0
        assert!((out.debug.grid_current - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn current_limit_grid_v_fallback_on_nan() {
        let mut input = base_input();
        input.grid_voltage = f64::NAN;
        input.grid_power = 1000.0;
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!(out.input_current_limit.is_finite());
        assert!(has_factor(&out.decision, "grid_v_fallback"));
    }

    #[test]
    fn current_limit_grid_v_fallback_just_below_threshold() {
        // PR-02-D09: tightened from 179 → 205 so the test name
        // ("just below threshold") matches the post-PR-02 207 V floor.
        let mut input = base_input();
        input.grid_voltage = 205.0;
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!(has_factor(&out.decision, "grid_v_fallback"));
    }

    #[test]
    fn current_limit_no_grid_v_fallback_at_exact_min_threshold() {
        // 207.0 V exactly — guard is `< MIN_SENSIBLE_GRID_V`; 207 should be accepted.
        let mut input = base_input();
        input.grid_voltage = 207.0;
        input.grid_power = 2070.0;
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!(!has_factor(&out.decision, "grid_v_fallback"));
        // grid_current = 2070 / 207.0 = 10.0
        assert!((out.debug.grid_current - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn current_limit_no_grid_v_fallback_at_exact_max_threshold() {
        // 260.0 V exactly — guard inclusive-on-max; 260 should be accepted.
        let mut input = base_input();
        input.grid_voltage = 260.0;
        input.grid_power = 2600.0;
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!(!has_factor(&out.decision, "grid_v_fallback"));
        assert!((out.debug.grid_current - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn current_limit_grid_v_fallback_on_over_voltage() {
        // 270.0 V surge glitch → fallback engaged.
        let mut input = base_input();
        input.grid_voltage = 270.0;
        input.grid_power = 2300.0;
        let out = evaluate_current_limit(&input, &clock_at(12, 0));
        assert!(has_factor(&out.decision, "grid_v_fallback"));
        // grid_current uses 230.0 V nominal
        let expected_current = input.grid_power / 230.0;
        assert!((out.debug.grid_current - expected_current).abs() < f64::EPSILON);
    }

    #[test]
    fn round2_works() {
        assert!((round2(1.234_567) - 1.23).abs() < f64::EPSILON);
        assert!((round2(1.235) - 1.24).abs() < f64::EPSILON);
        assert!((round2(-1.235) - -1.24).abs() < f64::EPSILON);
    }
}
