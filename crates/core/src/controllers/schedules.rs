//! BatteryLife schedule controller. 1:1 port of the legacy NR 'extended
//! sched' function (see `legacy/debug/20260421-120100-functions.txt`
//! lines 244-330).
//!
//! Writes two Victron ESS schedules:
//!
//! - **Schedule 0** — Boost (02:00–05:00). Always enabled (`days=7`),
//!   `discharge=0`, `soc=battery_selected_soc_target`.
//! - **Schedule 1** — NightExtended (05:00–08:00). Conditional: enabled
//!   only when the user wants extended charging and the SoC target hasn't
//!   been reached today; otherwise disabled (`days=-7`).
//!
//! Also computes `battery_selected_soc_target` — the effective SoC target
//! (battery_soc_target, or 100 if a weekly full-charge is required) —
//! which the current-limit controller reads for its `battery_charged`
//! decision.

use chrono::{NaiveDate, NaiveDateTime};

use crate::Clock;
use crate::controllers::tariff_band::{TariffBand, tariff_band};
use crate::types::Decision;

/// Victron schedule `Day` encoding used by the legacy flow.
///
/// The Victron `BatteryLife/Schedule/Charge/N/Day` DBus value is an integer
/// where:
/// - `7`  = every day (schedule enabled).
/// - `-7` = every day (schedule disabled / skip).
///
/// Higher-level semantics are not exercised by the port — we just forward
/// whatever the legacy decision tree produced.
pub const DAYS_ENABLED: i32 = 7;
pub const DAYS_DISABLED: i32 = -7;

/// A single BatteryLife schedule slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScheduleSpec {
    /// Start time, seconds from midnight.
    pub start_s: i32,
    /// Duration, seconds.
    pub duration_s: i32,
    /// `0` = don't allow discharge; `1` = allow.
    pub discharge: i32,
    /// SoC target (%).
    pub soc: f64,
    /// Day mask (see constants above).
    pub days: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchedulesInput {
    pub globals: SchedulesInputGlobals,
    pub battery_soc: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct SchedulesInputGlobals {
    pub charge_battery_extended: bool,
    pub charge_car_extended: bool,
    pub charge_to_full_required: bool,
    pub disable_night_grid_discharge: bool,
    pub zappi_active: bool,
    pub above_soc_date: Option<NaiveDate>,
    pub battery_soc_target: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SchedulesOutput {
    pub schedule_0: ScheduleSpec,
    pub schedule_1: ScheduleSpec,
    pub bookkeeping: SchedulesBookkeeping,
    pub debug: SchedulesDebug,
    /// Decision for Schedule 0 (the unconditional boost window) —
    /// always "enabled". Kept distinct from `schedule_1_decision`
    /// because the dashboard row for each schedule shows a Decision;
    /// sharing one explanation across both rows confused operators
    /// into reading "Schedule 1 disabled" as "schedule_0 disabled".
    pub schedule_0_decision: Decision,
    /// Decision for Schedule 1 (the conditional NightExtended window).
    /// Text is branch-specific.
    pub schedule_1_decision: Decision,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchedulesBookkeeping {
    /// The effective SoC target (100 when charge_to_full_required).
    pub battery_selected_soc_target: f64,
    /// New `above_soc_date` when the above-SoC latch fires; None leaves it
    /// unchanged (the caller preserves the previous value in that case).
    pub new_above_soc_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct SchedulesDebug {
    pub above_soc: bool,
    pub above_soc_date: Option<NaiveDate>,
    pub now: NaiveDateTime,
    pub battery_soc: f64,
    pub battery_soc_target: f64,
    pub battery_selected_soc_target: f64,
    pub disable_night_grid_discharge: bool,
    pub charge_to_full_required: bool,
    pub charge_battery_extended: bool,
    pub charge_car_extended: bool,
    pub is_extended_charge_time: bool,
}

/// Evaluate the ESS schedules for the current moment.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn evaluate_schedules(input: &SchedulesInput, clock: &dyn Clock) -> SchedulesOutput {
    let g = &input.globals;
    let now = clock.naive();

    let battery_selected_soc_target = if g.charge_to_full_required {
        100.0
    } else {
        g.battery_soc_target
    };

    let enable_night_grid_discharge = !g.disable_night_grid_discharge;
    let is_extended_charge_time = tariff_band(now) == TariffBand::NIGHT_EXTENDED;

    let battery_soc = input.battery_soc;
    let above_soc = battery_soc >= battery_selected_soc_target;

    // Schedule 0 — Boost 02:00–05:00, always-on.
    let schedule_0 = ScheduleSpec {
        start_s: 2 * 3600,
        duration_s: 3 * 3600,
        discharge: 0,
        soc: battery_selected_soc_target,
        days: DAYS_ENABLED,
    };

    // Schedule 1 — NightExtended 05:00–08:00, conditional.
    let mut schedule_1 = ScheduleSpec {
        start_s: 5 * 3600,
        duration_s: 3 * 3600,
        discharge: 0,
        soc: battery_selected_soc_target,
        days: DAYS_DISABLED, // placeholder, overridden below
    };

    let mut new_above_soc_date: Option<NaiveDate> = None;
    let branch: &'static str;

    if g.charge_battery_extended {
        // Decision tree:
        //   If during NightExtended with grid-discharge enabled AND SoC
        //   target already reached today → disable for rest of day, and
        //   latch the date so we stay disabled even if SoC later dips.
        let latch_hit_today = matches!(
            g.above_soc_date,
            Some(d) if d == now.date()
        );
        let in_extended_window_with_discharge =
            is_extended_charge_time && enable_night_grid_discharge;

        if in_extended_window_with_discharge && (above_soc || latch_hit_today) {
            schedule_1.days = DAYS_DISABLED;
            new_above_soc_date = Some(now.date());
            branch = "charge_battery_extended, but SoC hit target today during extended window → disable Schedule 1 for rest of day";
        } else {
            schedule_1.days = DAYS_ENABLED;
            branch = "charge_battery_extended active → Schedule 1 enabled";
        }
    } else if is_extended_charge_time && !g.zappi_active {
        // Zappi finished charging during extended time — stop charging battery.
        schedule_1.days = DAYS_DISABLED;
        branch = "extended window + Zappi finished → disable Schedule 1 (no more charging needed)";
    } else if g.charge_car_extended {
        // Car extended charging on but battery extended off — keep
        // schedule enabled with a nominal SoC=10 so ESS stays in the right
        // mode without actually drawing for the battery.
        schedule_1.soc = 10.0;
        schedule_1.days = DAYS_ENABLED;
        schedule_1.discharge = 0;
        branch = "charge_car_extended only → Schedule 1 enabled with nominal soc=10 (ESS mode only)";
    } else {
        schedule_1.days = DAYS_DISABLED;
        branch = "no extended-charge flags set → Schedule 1 disabled";
    }

    // Schedule 0: unconditionally enabled. Its "decision" is a statement
    // of that invariant, not a branch outcome. Include the SoC target so
    // operators can verify the boost window is writing the right number.
    let schedule_0_decision = Decision::new(
        "Schedule 0 = Boost window 02:00–05:00, unconditionally ENABLED (days=7)",
    )
    .with_factor("soc_target", format!("{battery_selected_soc_target:.0}%"))
    .with_factor("charge_to_full_required", format!("{}", g.charge_to_full_required));

    // Schedule 1: the branch-specific explanation.
    let schedule_1_decision = Decision::new(branch)
        .with_factor("charge_to_full_required", format!("{}", g.charge_to_full_required))
        .with_factor("charge_battery_extended", format!("{}", g.charge_battery_extended))
        .with_factor("charge_car_extended", format!("{}", g.charge_car_extended))
        .with_factor("zappi_active", format!("{}", g.zappi_active))
        .with_factor("battery_soc", format!("{battery_soc:.1}%"))
        .with_factor("battery_soc_target", format!("{battery_selected_soc_target:.0}%"))
        .with_factor("above_soc_today", format!("{above_soc}"))
        .with_factor("is_extended_charge_time", format!("{is_extended_charge_time}"));

    SchedulesOutput {
        schedule_0,
        schedule_1,
        bookkeeping: SchedulesBookkeeping {
            battery_selected_soc_target,
            new_above_soc_date,
        },
        schedule_0_decision,
        schedule_1_decision,
        debug: SchedulesDebug {
            above_soc,
            above_soc_date: g.above_soc_date,
            now,
            battery_soc,
            battery_soc_target: g.battery_soc_target,
            battery_selected_soc_target,
            disable_night_grid_discharge: g.disable_night_grid_discharge,
            charge_to_full_required: g.charge_to_full_required,
            charge_battery_extended: g.charge_battery_extended,
            charge_car_extended: g.charge_car_extended,
            is_extended_charge_time,
        },
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;

    fn clock_at(h: u32, m: u32) -> FixedClock {
        let nt = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap();
        FixedClock::at(nt)
    }

    fn base_input() -> SchedulesInput {
        SchedulesInput {
            globals: SchedulesInputGlobals {
                charge_battery_extended: false,
                charge_car_extended: false,
                charge_to_full_required: false,
                disable_night_grid_discharge: false,
                zappi_active: false,
                above_soc_date: None,
                battery_soc_target: 80.0,
            },
            battery_soc: 75.0,
        }
    }

    // ------------------------------------------------------------------
    // Schedule 0 is always on
    // ------------------------------------------------------------------

    #[test]
    fn schedule_0_is_always_boost_window_enabled() {
        let input = base_input();
        let out = evaluate_schedules(&input, &clock_at(12, 0));
        assert_eq!(out.schedule_0.start_s, 2 * 3600);
        assert_eq!(out.schedule_0.duration_s, 3 * 3600);
        assert_eq!(out.schedule_0.discharge, 0);
        assert_eq!(out.schedule_0.days, DAYS_ENABLED);
    }

    // ------------------------------------------------------------------
    // Full-charge promotes SoC target to 100
    // ------------------------------------------------------------------

    #[test]
    fn full_charge_required_promotes_soc_target_to_100() {
        let mut input = base_input();
        input.globals.charge_to_full_required = true;
        let out = evaluate_schedules(&input, &clock_at(12, 0));
        assert!((out.bookkeeping.battery_selected_soc_target - 100.0).abs() < f64::EPSILON);
        assert!((out.schedule_0.soc - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn full_charge_not_required_uses_battery_soc_target() {
        let mut input = base_input();
        input.globals.charge_to_full_required = false;
        input.globals.battery_soc_target = 80.0;
        let out = evaluate_schedules(&input, &clock_at(12, 0));
        assert!((out.bookkeeping.battery_selected_soc_target - 80.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // Schedule 1 decision tree
    // ------------------------------------------------------------------

    #[test]
    fn no_flags_set_disables_schedule_1() {
        let input = base_input();
        let out = evaluate_schedules(&input, &clock_at(12, 0));
        assert_eq!(out.schedule_1.days, DAYS_DISABLED);
    }

    #[test]
    fn charge_battery_extended_below_target_enables_schedule_1() {
        let mut input = base_input();
        input.globals.charge_battery_extended = true;
        input.battery_soc = 70.0; // below 80
        let out = evaluate_schedules(&input, &clock_at(12, 0));
        assert_eq!(out.schedule_1.days, DAYS_ENABLED);
    }

    #[test]
    fn charge_battery_extended_above_target_during_extended_disables_schedule_1() {
        let mut input = base_input();
        input.globals.charge_battery_extended = true;
        input.battery_soc = 85.0; // above 80
        let out = evaluate_schedules(&input, &clock_at(6, 30)); // NightExtended
        assert_eq!(out.schedule_1.days, DAYS_DISABLED);
        // And the above-soc latch is set.
        assert!(out.bookkeeping.new_above_soc_date.is_some());
    }

    #[test]
    fn charge_battery_extended_above_target_outside_extended_leaves_schedule_1_enabled() {
        let mut input = base_input();
        input.globals.charge_battery_extended = true;
        input.battery_soc = 85.0; // above 80
        let out = evaluate_schedules(&input, &clock_at(12, 0)); // daytime
        // Not in extended charge window → the latch doesn't fire, keep enabled.
        assert_eq!(out.schedule_1.days, DAYS_ENABLED);
        assert!(out.bookkeeping.new_above_soc_date.is_none());
    }

    #[test]
    fn above_soc_latch_persists_once_set() {
        let mut input = base_input();
        input.globals.charge_battery_extended = true;
        input.battery_soc = 75.0; // now BELOW target — but the latch fires
        let today = NaiveDate::from_ymd_opt(2026, 4, 21).unwrap();
        input.globals.above_soc_date = Some(today);
        let out = evaluate_schedules(&input, &clock_at(6, 30)); // extended window
        assert_eq!(
            out.schedule_1.days, DAYS_DISABLED,
            "above_soc_date from today keeps schedule disabled even when SoC dips back"
        );
    }

    #[test]
    fn above_soc_latch_from_yesterday_does_not_apply() {
        let mut input = base_input();
        input.globals.charge_battery_extended = true;
        input.battery_soc = 75.0;
        let yesterday = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
        input.globals.above_soc_date = Some(yesterday);
        let out = evaluate_schedules(&input, &clock_at(6, 30));
        assert_eq!(
            out.schedule_1.days, DAYS_ENABLED,
            "yesterday's latch doesn't apply today"
        );
    }

    #[test]
    fn charge_battery_extended_with_disabled_night_grid_discharge_skips_latch() {
        let mut input = base_input();
        input.globals.charge_battery_extended = true;
        input.globals.disable_night_grid_discharge = true;
        input.battery_soc = 85.0;
        let out = evaluate_schedules(&input, &clock_at(6, 30));
        // The legacy condition is `isExtendedChargeTime && enable_night_grid_discharge`
        // — with night grid discharge DISABLED that AND is false, so the latch
        // branch is skipped and the schedule stays enabled.
        assert_eq!(out.schedule_1.days, DAYS_ENABLED);
    }

    #[test]
    fn extended_window_with_zappi_inactive_and_no_extended_flag_disables() {
        let mut input = base_input();
        input.globals.zappi_active = false;
        let out = evaluate_schedules(&input, &clock_at(6, 30));
        assert_eq!(out.schedule_1.days, DAYS_DISABLED);
    }

    #[test]
    fn extended_window_with_zappi_active_and_no_flags_is_still_disabled() {
        // Zappi-active outside the charge_battery_extended branch takes the
        // final `else` → DAYS_DISABLED.
        let mut input = base_input();
        input.globals.zappi_active = true;
        let out = evaluate_schedules(&input, &clock_at(6, 30));
        assert_eq!(out.schedule_1.days, DAYS_DISABLED);
    }

    #[test]
    fn charge_car_extended_only_overrides_soc_to_10() {
        let mut input = base_input();
        input.globals.charge_car_extended = true;
        let out = evaluate_schedules(&input, &clock_at(12, 0));
        assert_eq!(out.schedule_1.days, DAYS_ENABLED);
        assert!((out.schedule_1.soc - 10.0).abs() < f64::EPSILON);
        assert_eq!(out.schedule_1.discharge, 0);
    }

    #[test]
    fn charge_battery_extended_wins_over_charge_car_extended() {
        let mut input = base_input();
        input.globals.charge_battery_extended = true;
        input.globals.charge_car_extended = true;
        input.battery_soc = 70.0;
        let out = evaluate_schedules(&input, &clock_at(12, 0));
        // The battery branch wins — keep schedule enabled with battery SoC target.
        assert_eq!(out.schedule_1.days, DAYS_ENABLED);
        assert!((out.schedule_1.soc - 80.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // Debug struct
    // ------------------------------------------------------------------

    #[test]
    fn debug_reports_tariff_window_and_soc_check() {
        let mut input = base_input();
        input.battery_soc = 85.0;
        let out_day = evaluate_schedules(&input, &clock_at(12, 0));
        assert!(!out_day.debug.is_extended_charge_time);
        assert!(out_day.debug.above_soc);

        let out_ext = evaluate_schedules(&input, &clock_at(6, 30));
        assert!(out_ext.debug.is_extended_charge_time);
    }
}
