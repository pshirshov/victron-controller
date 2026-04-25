//! Setpoint controller. 1:1 port of `compute_payload` in
//! `legacy/setpoint-node-red-ts/src/index.ts`.
//!
//! The function is intentionally structured the same as the TS version so
//! that corresponding branches are easy to diff. Variable names match as
//! closely as Rust permits. Magic constants are named at the top of
//! [`evaluate_setpoint`] so they can be surfaced as config later without
//! disturbing the algorithm.
//!
//! Outputs: a proposed grid setpoint (integer W) plus the full debug tuple
//! that used to flow to Node-RED debug nodes, plus bookkeeping values that
//! the outer `process()` must persist (`next_full_charge` etc.).

use chrono::{Datelike, NaiveDateTime, TimeDelta, Timelike};

use crate::Clock;
use crate::knobs::{DebugFullCharge, DischargeTime};
use crate::topology::HardwareParams;
use crate::types::Decision;

/// Inputs for the setpoint controller — mirrors `SetPointInput` in the
/// legacy TS module.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetpointInput {
    pub globals: SetpointInputGlobals,
    pub power_consumption: f64,
    pub battery_soc: f64,
    pub soh: f64,
    pub mppt_power_0: f64,
    pub mppt_power_1: f64,
    pub soltaro_power: f64,
    /// Signed power flow at the EV-branch CT clamp (`com.victronenergy.
    /// evcharger`'s `/Ac/Power` — the branch meter, not the Zappi
    /// proper). Positive when the branch imports (Zappi charging),
    /// negative when the branch exports (Hoymiles panels on the EV
    /// circuit pushing onto our grid). A-17: max(0, -evcharger) is
    /// the Hoymiles export contribution to `solar_export` per SPEC
    /// §5.8.
    pub evcharger_ac_power: f64,
    pub capacity: f64,
}

/// Subset of the world consumed by the setpoint controller that comes from
/// cross-cutting globals. In the TASS shell these fields live either in
/// [`crate::knobs::Knobs`] or in bookkeeping; they are assembled into this
/// struct at the call site of [`evaluate_setpoint`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct SetpointInputGlobals {
    pub force_disable_export: bool,
    pub export_soc_threshold: f64,
    pub discharge_soc_target: f64,
    pub full_charge_export_soc_threshold: f64,
    pub full_charge_discharge_soc_target: f64,
    pub zappi_active: bool,
    /// When `true` and `zappi_active`, the Zappi-specific branch (which
    /// pins setpoint to `-solar_export` to forbid battery → EV flow) is
    /// bypassed: the usual time-of-day controller runs, allowing the
    /// evening discharge branch to export battery into the grid-side
    /// branch the Zappi is drawing from.
    pub allow_battery_to_car: bool,
    pub discharge_time: DischargeTime,
    pub debug_full_charge: DebugFullCharge,
    pub pessimism_multiplier_modifier: f64,
    pub next_full_charge: Option<NaiveDateTime>,
    /// PR-inverter-safe-discharge-knob. When `true`, the legacy 4020 W
    /// "inverter safe discharge" margin is applied to the `max_discharge`
    /// floor (preserving Node-RED's behaviour). When `false` (the new
    /// default), the inverter discharges at the full hardware ceiling
    /// `inverter_max_discharge_w`. The user's MultiPlus firmware does
    /// not reproduce the legacy "forced grid charge during 4.8 kW+
    /// discharge" glitch, so the margin is OFF by default; affected
    /// firmware users can flip the knob to `true`.
    pub inverter_safe_discharge_enable: bool,
}

/// Full output of the setpoint controller. The shell turns this into a
/// single `WriteDbus` effect for `GridSetpoint` and (separately) persists
/// the `bookkeeping` fields to retained MQTT.
#[derive(Debug, Clone, PartialEq)]
pub struct SetpointOutput {
    /// The target value to write to `/Settings/CGwacs/AcPowerSetPoint`.
    pub setpoint_target: i32,
    pub debug: SetpointDebug,
    pub bookkeeping: SetpointBookkeeping,
    /// Human-readable explanation of which branch fired and which
    /// factors drove the chosen target. Published into the world
    /// snapshot so the dashboard can show it next to the target.
    pub decision: Decision,
}

/// Full debug tuple — mirrors `SetPointOutputDebug` verbatim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetpointDebug {
    pub total_capacity: f64,
    pub current_capacity: f64,
    pub end_of_day_target: f64,
    pub current_target: f64,
    pub consumption: f64,
    pub hours_remaining: f64,
    pub soltaro_power: f64,
    pub mppt_power: f64,
    pub exportable_capacity: f64,
    pub pv_multiplier: f64,
    pub charge_to_full_required: bool,
    pub next_full_charge: NaiveDateTime,
    pub to_be_consumed: f64,
    pub now: NaiveDateTime,
    pub max_discharge: f64,
    pub solar_export: f64,
    pub soltaro_export: f64,
    pub pessimism_multiplier: f64,
    pub preserve_battery: bool,
    pub soc_end_of_day_target: f64,
    pub export_soc_threshold: f64,
    pub debug_full_charge: DebugFullCharge,
    pub remaining_current_consumption: f64,
    pub zappi_active: bool,
}

/// Bookkeeping updates that survive across invocations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetpointBookkeeping {
    pub next_full_charge: NaiveDateTime,
    pub charge_to_full_required: bool,
    pub soc_end_of_day_target: f64,
    pub export_soc_threshold: f64,
}

// --- Constants (now sourced from `HardwareParams`; see `crate::topology`) ---
//
// Previously these lived as module-level `const`s
// (`nominal_voltage_v` / `baseload_consumption_w` /
// `battery_discharge_limit_w` / `battery_side_max_discharge_w` /
// `idle_setpoint_w`). They are now deploy-time hardware parameters
// threaded into the controller from `Topology::hardware`.

// -----------------------------------------------------------------------------
// Main controller
// -----------------------------------------------------------------------------

/// Evaluate the grid setpoint for the current moment.
///
/// Mirrors the branching structure of `compute_payload` in the TS source.
#[allow(clippy::too_many_lines)]
#[allow(clippy::float_cmp)]
#[must_use]
pub fn evaluate_setpoint(
    input: &SetpointInput,
    clock: &dyn Clock,
    hw: &HardwareParams,
) -> SetpointOutput {
    // Local rebind so the body reads identically to the legacy form.
    let nominal_voltage_v = hw.battery_nominal_voltage_v;
    let baseload_consumption_w = hw.baseload_consumption_w;
    let battery_discharge_limit_w = hw.inverter_safe_discharge_w;
    let battery_side_max_discharge_w = hw.inverter_max_discharge_w;
    let idle_setpoint_w = hw.idle_setpoint_w;
    let g = &input.globals;
    let now = clock.naive();

    let next_full_charge_defined = g.next_full_charge.is_some();

    let mut next_full_charge = g
        .next_full_charge
        .unwrap_or_else(|| get_next_charge_date_to_sunday_5pm(now, 0, false));

    let charge_to_full_required = match g.debug_full_charge {
        DebugFullCharge::Forbid => false,
        DebugFullCharge::Force => true,
        DebugFullCharge::None => next_full_charge <= now,
    };

    let soc_end_of_day_target = if charge_to_full_required {
        g.full_charge_discharge_soc_target.max(g.discharge_soc_target)
    } else {
        g.discharge_soc_target
    };

    let export_soc_threshold = if charge_to_full_required {
        g.full_charge_export_soc_threshold
    } else {
        g.export_soc_threshold
    };

    let current_consumption = input.power_consumption;
    let mut remaining_current_consumption = current_consumption;
    let battery_soc = input.battery_soc;
    let battery_soh = input.soh;
    let mppt_power = input.mppt_power_0 + input.mppt_power_1;
    let soltaro_power = input.soltaro_power;
    let battery_capacity = input.capacity;

    let total_capacity = battery_capacity * (battery_soh / 100.0) * nominal_voltage_v;
    let current_capacity = total_capacity * (battery_soc / 100.0);
    let end_of_day_target = total_capacity * (soc_end_of_day_target / 100.0);
    let mut current_target = end_of_day_target;

    // SPEC §5.8 / A-17: fold the Hoymiles export on the EV branch into
    // solar_export. The `evcharger` CT sees the Zappi + Hoymiles branch
    // combined: positive when the branch imports (Zappi charging on the
    // car), negative when the branch exports (Hoymiles panels pushing
    // onto our grid). `max(0, -evcharger_ac_power)` is the net export
    // contribution. Without this term the setpoint controller under-
    // exports by the Hoymiles kW in the evening, leaving that power to
    // sail past the max_discharge cap and onto the grid unaccounted.
    let hoymiles_export = (-input.evcharger_ac_power).max(0.0);
    let solar_export =
        (mppt_power.max(0.0) + soltaro_power.max(0.0) + hoymiles_export).floor();
    let soltaro_export = if soltaro_power > 400.0 {
        (10.0 + soltaro_power - current_consumption).max(0.0)
    } else {
        0.0
    };

    // `max_discharge = max(-5000, -max(0, battery_discharge_limit + solar_export))`
    //
    // PR-inverter-safe-discharge-knob: gated on `inverter_safe_discharge_enable`.
    // When `true`, apply the legacy 4020 W safety margin (calibrated for an
    // observed "forced grid charge during 4.8 kW+ discharge" glitch on some
    // MultiPlus firmware). When `false` (default), the inverter discharges
    // at the full hardware ceiling `inverter_max_discharge_w` — the user's
    // firmware does not reproduce the glitch.
    let max_discharge = if g.inverter_safe_discharge_enable {
        battery_side_max_discharge_w
            .max(-((battery_discharge_limit_w + solar_export).max(0.0)))
    } else {
        // Knob off — the inverter glitch margin doesn't apply.
        // max_discharge is just the hardware ceiling.
        battery_side_max_discharge_w
    };

    let mut hours_remaining: f64 = -1.0;
    let mut exportable_capacity: f64 = -1.0;
    let mut to_be_consumed: f64 = -1.0;
    let mut pv_multiplier: f64 = -1.0;
    // Placeholder; every branch below assigns a real value. `mut` is needed
    // because the evening sub-branch reassigns after its initial write.
    #[allow(unused_assignments)]
    let mut setpoint_target: f64 = idle_setpoint_w;
    let mut pessimism_multiplier: f64 = 1.0;
    let mut preserve_battery: bool = false;

    // SoC ≥ 99.99 rolls next_full_charge forward by a week. Strict
    // `== 100.0` was brittle: Victron's /Soc is an f64 and any future
    // firmware that reports 99.95 / 100.01 would silently skip a
    // weekly rollover. 99.99 keeps the "we hit full" semantic without
    // the equality risk.
    if battery_soc >= 99.99 {
        next_full_charge = get_next_charge_date_to_sunday_5pm(now, 1, next_full_charge_defined);
    }

    let hour = now.hour();
    let minute = now.minute();

    // Branches match the TS version's order exactly, with one addition:
    // the zappi_active branch is gated on `!allow_battery_to_car` so the
    // new knob (SPEC §5.9) can bypass the PV-only export clamp and let
    // the regular time-of-day controller run (which may discharge battery
    // through the grid into the EV).
    let mut decision: Decision;
    if g.force_disable_export {
        setpoint_target = idle_setpoint_w;
        decision = Decision::new("Export killed by force_disable_export → idle 10 W")
            .with_factor("force_disable_export", "true");
    } else if g.zappi_active && !g.allow_battery_to_car {
        if (2..8).contains(&hour) {
            setpoint_target = idle_setpoint_w - soltaro_export;
            decision = Decision::new(
                "Zappi active in early-morning window — dump Soltaro surplus only, preserve battery"
            )
            .with_factor("hour", format!("{hour:02}:{minute:02}"))
            .with_factor("zappi_active", "true")
            .with_factor("allow_battery_to_car", "false")
            .with_factor("soltaro_power_W", format!("{soltaro_power:.0}"))
            .with_factor("soltaro_export_W", format!("{soltaro_export:.0}"));
        } else {
            setpoint_target = -solar_export;
            decision = Decision::new(
                "Zappi active during the day — export PV only, do not discharge battery into car",
            )
            .with_factor("zappi_active", "true")
            .with_factor("allow_battery_to_car", "false")
            .with_factor("solar_export_W", format!("{solar_export:.0}"))
            .with_factor("setpoint_W (pre-clamp)", format!("{setpoint_target:.0}"));
        }
    } else if hour == 23 && minute >= 55 {
        // "qendercore protection window" — avoid feeding Soltaro during the
        // 23:59–00:00 grid quirk; start 5 min early for export rampdown.
        setpoint_target = idle_setpoint_w;
        decision = Decision::new(
            "23:55-00:00 Soltaro-feed protection window → idle 10 W",
        )
        .with_factor("hour", format!("{hour:02}:{minute:02}"));
    } else if !(2..17).contains(&hour) {
        // Evening discharge controller.
        let early_discharge = g.discharge_time == DischargeTime::At2300;

        let mut discharge_end_time = now;
        if early_discharge {
            if hour < 2 {
                discharge_end_time -= TimeDelta::days(1);
            }
            discharge_end_time = discharge_end_time
                .date()
                .and_hms_opt(23, 0, 0)
                .expect("valid time-of-day");
        } else {
            if hour > 2 {
                discharge_end_time += TimeDelta::days(1);
            }
            discharge_end_time = discharge_end_time
                .date()
                .and_hms_opt(2, 0, 0)
                .expect("valid time-of-day");
        }

        let hour_before = discharge_end_time - TimeDelta::hours(1);

        // A-63: `num_milliseconds()` returns i64 and cannot saturate
        // here — `discharge_end_time - now` is bounded to at most one
        // calendar day (< 9e7 ms), well within f64's 53-bit integer
        // range. The clippy lint fires because `as f64` could in
        // principle lose precision; allow is correct for this usage.
        // If clock skew ever made these deltas pathological (days
        // away), the subsequent `<= 0.0` branches still behave
        // correctly and the setpoint falls through to `idle 10 W`.
        #[allow(clippy::cast_precision_loss)]
        let millis_remaining_1hour = (hour_before - now).num_milliseconds() as f64;
        #[allow(clippy::cast_precision_loss)]
        let millis_remaining_end = (discharge_end_time - now).num_milliseconds() as f64;
        let millis_remaining = if millis_remaining_1hour <= 0.0 {
            millis_remaining_end
        } else {
            millis_remaining_1hour
        };

        current_target = if millis_remaining_1hour <= 0.0 {
            end_of_day_target
        } else {
            end_of_day_target + 3000.0
        };

        if millis_remaining <= 0.0 {
            setpoint_target = idle_setpoint_w;
            decision = Decision::new("Evening window past discharge-end time → idle 10 W")
                .with_factor("hour", format!("{hour:02}:{minute:02}"))
                .with_factor("discharge_time_knob", format!("{:?}", g.discharge_time));
        } else {
            hours_remaining = millis_remaining / (1000.0 * 60.0 * 60.0);
            pessimism_multiplier = 1.8_f64.min(
                g.pessimism_multiplier_modifier
                    * (((hours_remaining / 10.0 + 1.0) * 80.0).round() / 80.0),
            );

            remaining_current_consumption = current_consumption * hours_remaining;
            to_be_consumed = 0.0_f64.max(remaining_current_consumption * pessimism_multiplier);
            exportable_capacity = 0.0_f64.max(current_capacity - to_be_consumed - current_target);

            let soltaro_exports = soltaro_power > 400.0;
            let battery_export = exportable_capacity / hours_remaining;
            let exportable_power = if soltaro_exports {
                10.0 + battery_export + solar_export
            } else if battery_soc >= export_soc_threshold {
                battery_export + solar_export
            } else {
                battery_export
            };
            let export_power = exportable_power - current_consumption;

            let remaining_baseload_consumption = baseload_consumption_w * hours_remaining;
            let baseload_to_be_consumed =
                0.0_f64.max(remaining_baseload_consumption * pessimism_multiplier);

            preserve_battery = export_soc_threshold == 100.0
                || current_capacity - current_target < baseload_to_be_consumed;

            let min_pre: f64 = if preserve_battery { 10.0 } else { -200.0 };
            setpoint_target = min_pre.min(-export_power);
            decision = Decision::new(if preserve_battery {
                "Evening discharge — baseload would drain below target, preserving battery at idle"
            } else {
                "Evening discharge — exporting to hit end-of-day target"
            })
            .with_factor("hour", format!("{hour:02}:{minute:02}"))
            .with_factor("battery_soc", format!("{battery_soc:.1}%"))
            .with_factor("export_soc_threshold", format!("{export_soc_threshold:.0}%"))
            .with_factor("hours_remaining", format!("{hours_remaining:.2}"))
            .with_factor("pessimism_multiplier", format!("{pessimism_multiplier:.2}"))
            .with_factor("current_capacity_Wh", format!("{current_capacity:.0}"))
            .with_factor("current_target_Wh", format!("{current_target:.0}"))
            .with_factor("exportable_capacity_Wh", format!("{exportable_capacity:.0}"))
            .with_factor("export_power_W", format!("{export_power:.0}"))
            .with_factor("preserve_battery", format!("{preserve_battery}"));
        }
    } else if (8..17).contains(&hour) {
        // Daytime PV-multiplier controller.
        if export_soc_threshold == 100.0 {
            setpoint_target = idle_setpoint_w;
            decision = Decision::new(
                "Daytime but export_soc_threshold=100 → hold at idle 10 W",
            )
            .with_factor("hour", format!("{hour:02}:{minute:02}"))
            .with_factor("export_soc_threshold", "100%");
        } else {
            let bad_weather = solar_export <= 1100.0;
            let min_setpoint: f64 = if bad_weather { 10.0 } else { -200.0 };
            let balance_soc = export_soc_threshold + 3.0;

            // A-33: widen the float-equality rungs to half-open ranges.
            // Previously `battery_soc == balance_soc` etc. fell through
            // to `0.0` (PV-multiplier off) on any ε noise from MQTT-
            // retained SoC deserialise (e.g. 80.0000001). ±0.5 slop on
            // each rung covers realistic noise without overlapping the
            // neighbouring `>=` rung above it.
            pv_multiplier = if export_soc_threshold <= 67.0 {
                if battery_soc >= balance_soc + 20.0 {
                    5.0
                } else if battery_soc >= balance_soc + 15.0 {
                    2.5
                } else if battery_soc >= balance_soc + 10.0 {
                    2.0
                } else if battery_soc >= balance_soc + 5.0 {
                    1.5
                } else if battery_soc > balance_soc + 0.5 {
                    1.1
                } else if battery_soc >= balance_soc - 0.5 {
                    1.0
                } else if battery_soc >= balance_soc - 1.5 {
                    0.9
                } else if battery_soc >= balance_soc - 2.5 {
                    0.8
                } else {
                    0.0
                }
            } else if battery_soc >= balance_soc + 15.0 {
                5.0
            } else if battery_soc >= balance_soc + 10.0 {
                3.0
            } else if battery_soc >= balance_soc + 2.0 {
                2.5
            } else if battery_soc > balance_soc + 0.5 {
                1.1
            } else if battery_soc >= balance_soc - 0.5 {
                1.0
            } else if battery_soc >= balance_soc - 1.5 {
                0.9
            } else if battery_soc >= balance_soc - 2.5 {
                0.8
            } else {
                0.0
            };

            let bad_mult = if bad_weather { 0.0 } else { 1.0 };
            let export_power = 0.0_f64
                .max(solar_export * pv_multiplier * bad_mult - current_consumption);
            let balance_margin: f64 = if battery_soc < balance_soc { 100.0 } else { 0.0 };

            let day_branch: &'static str;
            if battery_soc < export_soc_threshold {
                setpoint_target = idle_setpoint_w;
                day_branch = "SoC below export threshold — hold at idle";
            } else if battery_soc == export_soc_threshold {
                setpoint_target = min_setpoint;
                day_branch = "SoC at threshold — bleed the minimum export step";
            } else {
                setpoint_target = min_setpoint.min(balance_margin - export_power);
                day_branch = "SoC above threshold — PV-multiplier export";
            }
            decision = Decision::new(format!("Daytime — {day_branch}"))
                .with_factor("hour", format!("{hour:02}:{minute:02}"))
                .with_factor("battery_soc", format!("{battery_soc:.1}%"))
                .with_factor("export_soc_threshold", format!("{export_soc_threshold:.0}%"))
                .with_factor("balance_soc", format!("{balance_soc:.1}%"))
                .with_factor("solar_export_W", format!("{solar_export:.0}"))
                .with_factor("bad_weather", format!("{bad_weather}"))
                .with_factor("pv_multiplier", format!("{pv_multiplier:.2}"))
                .with_factor("export_power_W", format!("{export_power:.0}"));
        }
    } else if (2..5).contains(&hour) {
        // Boost window — the (2..5) and final-else branches both set
        // the same setpoint (idle_setpoint_w), but they emit different
        // Decision text so the operator dashboard can tell which window
        // produced "idle 10 W" today. Not mechanically redundant; the
        // separation is for decision-log honesty (A-64).
        setpoint_target = idle_setpoint_w;
        decision = Decision::new("Boost window (02:00–05:00) → idle 10 W")
            .with_factor("hour", format!("{hour:02}:{minute:02}"));
    } else {
        // 05:00–08:00 extended night.
        setpoint_target = idle_setpoint_w;
        decision = Decision::new("Extended night (05:00–08:00) → idle 10 W")
            .with_factor("hour", format!("{hour:02}:{minute:02}"));
    }

    let final_setpoint = prepare_setpoint(max_discharge, setpoint_target, idle_setpoint_w);

    // Record the post-processing facts that affected the final number.
    // PR-inverter-safe-discharge-knob: surface the knob state next to
    // `max_discharge` so the dashboard shows whether the legacy margin
    // was in play for this tick (honesty invariant).
    decision = decision
        .with_factor("pre_clamp_setpoint_W", format!("{setpoint_target:.0}"))
        .with_factor("max_discharge_W", format!("{max_discharge:.0}"))
        .with_factor(
            "inverter_safe_discharge_enable",
            format!("{}", g.inverter_safe_discharge_enable),
        )
        .with_factor("final_setpoint_W", format!("{final_setpoint}"));

    // `mppt_power`, `soltaro_power`, `battery_soh` are used in the debug
    // struct below; keep them bound so they're not "unused" lints.
    let _ = battery_soh;

    SetpointOutput {
        setpoint_target: final_setpoint,
        debug: SetpointDebug {
            total_capacity,
            current_capacity,
            end_of_day_target,
            current_target,
            consumption: current_consumption,
            hours_remaining,
            soltaro_power,
            mppt_power,
            exportable_capacity,
            pv_multiplier,
            charge_to_full_required,
            next_full_charge,
            to_be_consumed,
            now,
            max_discharge,
            solar_export,
            soltaro_export,
            pessimism_multiplier,
            preserve_battery,
            soc_end_of_day_target,
            export_soc_threshold,
            debug_full_charge: g.debug_full_charge,
            remaining_current_consumption,
            zappi_active: g.zappi_active,
        },
        bookkeeping: SetpointBookkeeping {
            next_full_charge,
            charge_to_full_required,
            soc_end_of_day_target,
            export_soc_threshold,
        },
        decision,
    }
}

// -----------------------------------------------------------------------------
// Internals
// -----------------------------------------------------------------------------

/// Post-process a raw proposed setpoint:
/// - clamp to `max_discharge` on the negative side,
/// - floor,
/// - round to nearest 50 W,
/// - promote any non-negative value to `idle_setpoint_w`.
///
/// Matches `_prepare_setpoint` in the TS source.
#[must_use]
pub fn prepare_setpoint(max_discharge: f64, setpoint_target: f64, idle_setpoint_w: f64) -> i32 {
    let mut x = max_discharge.max(setpoint_target.floor());
    x = (x / 50.0).round() * 50.0;
    if x >= 0.0 {
        #[allow(clippy::cast_possible_truncation)]
        {
            idle_setpoint_w as i32
        }
    } else {
        #[allow(clippy::cast_possible_truncation)]
        {
            x as i32
        }
    }
}

/// `getNextChargeDateToSunday5pmAfterNWeeks(weeks)` from the TS source.
///
/// Returns a Sunday 17:00 datetime N weeks from `now`:
/// - if `weeks == 0` and there is an existing `next_full_charge`, and the
///   current weekday is Thu/Fri/Sat, push to next week's Sunday;
/// - otherwise land on this week's Sunday.
/// - if the result is before `now`, push forward one week.
fn get_next_charge_date_to_sunday_5pm(
    now: NaiveDateTime,
    weeks: i64,
    next_full_charge_defined: bool,
) -> NaiveDateTime {
    let mut d = now + TimeDelta::days(weeks * 7);
    d = d
        .date()
        .and_hms_opt(17, 0, 0)
        .expect("hms constants are always valid");
    let dow = i64::from(d.weekday().num_days_from_sunday()); // 0 = Sunday
    if dow > 3 && next_full_charge_defined {
        d += TimeDelta::days(7 - dow);
    } else {
        d -= TimeDelta::days(dow);
    }
    if d < now {
        d += TimeDelta::days(7);
    }
    d
}

// -----------------------------------------------------------------------------
// Tests — ported from legacy/setpoint-node-red-ts/src/__tests__/index.test.ts
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use chrono::NaiveDate;

    /// Default `HardwareParams` for tests — matches the legacy
    /// hard-coded const values, so existing assertions keep holding.
    fn hw() -> HardwareParams {
        HardwareParams::defaults()
    }

    /// Equivalent to the Jest `createBaseInput()` helper.
    fn base_input() -> SetpointInput {
        SetpointInput {
            globals: SetpointInputGlobals {
                force_disable_export: false,
                export_soc_threshold: 70.0,
                discharge_soc_target: 25.0,
                full_charge_export_soc_threshold: 85.0,
                full_charge_discharge_soc_target: 30.0,
                zappi_active: false,
                allow_battery_to_car: false,
                discharge_time: DischargeTime::At0200,
                debug_full_charge: DebugFullCharge::None,
                pessimism_multiplier_modifier: 1.0,
                next_full_charge: None,
                // PR-inverter-safe-discharge-knob: default for fixtures
                // is `false` (the production default). Tests asserting
                // legacy safety/discharge-cap/max_discharge behaviour
                // override this to `true` per their globals literal.
                inverter_safe_discharge_enable: false,
            },
            power_consumption: 1500.0,
            battery_soc: 80.0,
            soh: 95.0,
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            soltaro_power: 500.0,
            // A-17: no Hoymiles export in the baseline fixture; the
            // branch is idle. Tests that want to exercise the Hoymiles
            // term override with a negative value (export).
            evcharger_ac_power: 0.0,
            capacity: 100.0,
        }
    }

    fn clock_at(y: i32, m: u32, d: u32, h: u32, min: u32) -> FixedClock {
        let nt = NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap();
        FixedClock::at(nt)
    }

    // ------------------------------------------------------------------
    // Basic functionality
    // ------------------------------------------------------------------

    #[test]
    fn capacity_model() {
        let input = SetpointInput {
            capacity: 100.0,
            soh: 95.0,
            battery_soc: 80.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());

        let expected_total = 100.0 * (95.0 / 100.0) * 48.0; // 4560
        let expected_current = expected_total * (80.0 / 100.0); // 3648
        assert!((out.debug.total_capacity - expected_total).abs() < 1e-9);
        assert!((out.debug.current_capacity - expected_current).abs() < 1e-9);
    }

    #[test]
    fn solar_export_sums_mppts_and_soltaro() {
        let input = SetpointInput {
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            soltaro_power: 800.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        let expected = ((2000.0_f64 + 1500.0).max(0.0) + 800.0_f64.max(0.0)).floor();
        assert!((out.debug.solar_export - expected).abs() < 1e-9);
    }

    #[test]
    fn solar_export_includes_hoymiles_ev_branch_export() {
        // A-17 / SPEC §5.8: max(0, -evcharger_ac_power) is the Hoymiles
        // branch contribution. EV branch exporting 1200 W (i.e.
        // evcharger_ac_power = -1200) + 2000 W from mppt0 + 0 soltaro
        // should net 3200 W of solar_export.
        let input = SetpointInput {
            mppt_power_0: 2000.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            evcharger_ac_power: -1200.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(
            (out.debug.solar_export - 3200.0).abs() < 1e-9,
            "expected 3200, got {}",
            out.debug.solar_export
        );
    }

    #[test]
    fn solar_export_ignores_ev_branch_when_importing() {
        // Zappi charging on the EV branch → evcharger_ac_power > 0.
        // max(0, -positive) = 0, so the Hoymiles term contributes zero.
        let input = SetpointInput {
            mppt_power_0: 2000.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            evcharger_ac_power: 1500.0, // Zappi drawing 1.5 kW
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(
            (out.debug.solar_export - 2000.0).abs() < 1e-9,
            "expected 2000 (mppt only, Zappi import ignored), got {}",
            out.debug.solar_export
        );
    }

    #[test]
    fn solar_export_clamps_negatives_to_zero() {
        let input = SetpointInput {
            mppt_power_0: -100.0,
            mppt_power_1: 1500.0,
            soltaro_power: -200.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // mppt_sum = 1400 (>0); soltaro clamped to 0
        let expected = (((-100.0_f64) + 1500.0).max(0.0) + (-200.0_f64).max(0.0)).floor();
        assert!((out.debug.solar_export - expected).abs() < 1e-9);
    }

    // ------------------------------------------------------------------
    // Force-disable-export
    // ------------------------------------------------------------------

    #[test]
    fn force_disable_export_pins_to_idle() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                force_disable_export: true,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    // ------------------------------------------------------------------
    // Zappi branches
    // ------------------------------------------------------------------

    #[test]
    fn zappi_active_before_8am_uses_soltaro_export() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                ..base_input().globals
            },
            soltaro_power: 600.0,
            power_consumption: 1200.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 5, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // soltaro_export = max(10 + 600 - 1200, 0) = 0
        // setpoint = 10 - 0 = 10 → after rounding/≥0 rule → 10
        assert_eq!(out.setpoint_target, 10);
    }

    // ------------------------------------------------------------------
    // allow_battery_to_car toggle — SPEC §5.9
    // ------------------------------------------------------------------

    #[test]
    fn allow_battery_to_car_bypasses_zappi_night_branch() {
        // Evening time, zappi active, allow_battery_to_car=true — the
        // Zappi-specific branch is bypassed and the evening discharge
        // controller runs. We don't assert a specific setpoint value,
        // just that it's NOT the `10 - soltaro_export` or `-solar_export`
        // form the zappi branch would produce.
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: true,
                export_soc_threshold: 70.0,
                ..base_input().globals
            },
            battery_soc: 90.0,
            power_consumption: 1500.0,
            ..base_input()
        };
        // 20:30 — evening branch.
        let c = clock_at(2026, 1, 15, 20, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // Evening controller engages: hours_remaining is set (it's -1 in
        // other branches). That's the strongest signal that we took the
        // evening path, not the zappi path.
        assert!(out.debug.hours_remaining > 0.0);
    }

    #[test]
    fn allow_battery_to_car_false_keeps_zappi_clamp() {
        // Same inputs, knob off — verify we still take the Zappi branch
        // and produce `-solar_export`.
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                ..base_input().globals
            },
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            soltaro_power: 500.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 20, 30); // evening
        let out = evaluate_setpoint(&input, &c, &hw());
        let expected_solar_export = (3500.0_f64.max(0.0) + 500.0_f64.max(0.0)).floor();
        assert_eq!(out.setpoint_target, -(expected_solar_export as i32));
        // hours_remaining stays at its -1 sentinel — evening branch did NOT run.
        assert!((out.debug.hours_remaining + 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn force_disable_export_overrides_allow_battery_to_car() {
        // Kill switch still wins.
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                force_disable_export: true,
                zappi_active: true,
                allow_battery_to_car: true,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 20, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    #[test]
    fn zappi_active_daytime_uses_solar_export() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                ..base_input().globals
            },
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            soltaro_power: 500.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        let expected_solar_export = (3500.0_f64.max(0.0) + 500.0_f64.max(0.0)).floor(); // 4000
        // raw target = -4000; no clamp (max_discharge = -5000 - 4020 - 4000 capped at -5000)
        // Actually max_discharge = max(-5000, -(4020+4000)) = max(-5000, -8020) = -5000.
        // setpoint after prepare = max(-5000, floor(-4000)) = -4000; round to 50 = -4000.
        let expected = -(expected_solar_export as i32);
        assert_eq!(out.setpoint_target, expected);
    }

    // ------------------------------------------------------------------
    // Evening discharge
    // ------------------------------------------------------------------

    #[test]
    fn evening_discharge_produces_sub_10_setpoint() {
        let input = SetpointInput {
            battery_soc: 80.0,
            power_consumption: 1500.0,
            globals: SetpointInputGlobals {
                discharge_time: DischargeTime::At0200,
                export_soc_threshold: 70.0,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 20, 30);
        let out = evaluate_setpoint(&input, &c, &hw());

        assert!(out.debug.hours_remaining > 0.0);
        assert!(out.debug.exportable_capacity >= 0.0);
        assert!(out.setpoint_target <= 10);
    }

    #[test]
    fn evening_discharge_halts_after_end_time() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                discharge_time: DischargeTime::At0200,
                ..base_input().globals
            },
            ..base_input()
        };
        // 02:30 is past the 02:00 discharge end (millis_remaining becomes ≤ 0).
        let c = clock_at(2026, 1, 15, 2, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    // ------------------------------------------------------------------
    // Daytime branches
    // ------------------------------------------------------------------

    #[test]
    fn daytime_exports_when_above_threshold_and_good_weather() {
        let input = SetpointInput {
            battery_soc: 75.0,
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            power_consumption: 1200.0,
            globals: SetpointInputGlobals {
                export_soc_threshold: 70.0,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(out.setpoint_target < 10);
        assert!(out.debug.pv_multiplier > 0.0);
    }

    #[test]
    fn daytime_does_not_export_below_threshold() {
        let input = SetpointInput {
            battery_soc: 65.0,
            globals: SetpointInputGlobals {
                export_soc_threshold: 70.0,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    #[test]
    fn daytime_does_not_export_in_bad_weather() {
        let input = SetpointInput {
            battery_soc: 75.0,
            mppt_power_0: 300.0,
            mppt_power_1: 200.0,
            soltaro_power: 100.0, // solar_export = 600 ≤ 1100 → bad weather
            globals: SetpointInputGlobals {
                export_soc_threshold: 70.0,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    #[test]
    fn daytime_preserves_battery_when_threshold_is_100() {
        let input = SetpointInput {
            battery_soc: 90.0,
            globals: SetpointInputGlobals {
                export_soc_threshold: 100.0,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    // ------------------------------------------------------------------
    // Special time windows
    // ------------------------------------------------------------------

    #[test]
    fn last_5_minutes_before_midnight_pins_to_idle() {
        let input = base_input();
        let c = clock_at(2026, 1, 15, 23, 57);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    #[test]
    fn boost_window_pins_to_idle() {
        let input = base_input();
        let c = clock_at(2026, 1, 15, 3, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    #[test]
    fn extended_night_pins_to_idle() {
        let input = base_input();
        let c = clock_at(2026, 1, 15, 6, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    // ------------------------------------------------------------------
    // Full-charge logic
    // ------------------------------------------------------------------

    #[test]
    fn debug_full_charge_force_sets_flag() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                debug_full_charge: DebugFullCharge::Force,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());

        assert!(out.debug.charge_to_full_required);
        // full_charge_export_soc_threshold=85 wins
        assert!((out.debug.export_soc_threshold - 85.0).abs() < f64::EPSILON);
        // max(full_charge_discharge_soc_target=30, discharge_soc_target=25) = 30
        assert!((out.debug.soc_end_of_day_target - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn debug_full_charge_forbid_ignores_past_next_full_charge() {
        let past = NaiveDate::from_ymd_opt(2023, 1, 1)
            .unwrap()
            .and_hms_opt(17, 0, 0)
            .unwrap();
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                debug_full_charge: DebugFullCharge::Forbid,
                next_full_charge: Some(past),
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(!out.debug.charge_to_full_required);
    }

    #[test]
    fn future_next_full_charge_does_not_trigger() {
        // Pick a date far in the future relative to the clock we supply.
        let future = NaiveDate::from_ymd_opt(2030, 1, 8)
            .unwrap() // Tuesday
            .and_hms_opt(17, 0, 0)
            .unwrap();
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                debug_full_charge: DebugFullCharge::None,
                next_full_charge: Some(future),
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(!out.debug.charge_to_full_required);
    }

    #[test]
    fn soc_100_rolls_next_full_charge_to_next_sunday_17() {
        // Preserved input: a next_full_charge in the past does not matter
        // because SoC==100 triggers the roll-forward path.
        let existing = NaiveDate::from_ymd_opt(2025, 9, 3)
            .unwrap()
            .and_hms_opt(1, 0, 0)
            .unwrap();
        let input = SetpointInput {
            battery_soc: 100.0,
            globals: SetpointInputGlobals {
                next_full_charge: Some(existing),
                ..base_input().globals
            },
            ..base_input()
        };
        // Wednesday 2026-04-22 as our "now"
        let c = clock_at(2026, 4, 22, 10, 0);
        let out = evaluate_setpoint(&input, &c, &hw());

        let next = out.debug.next_full_charge;
        assert_eq!(next.hour(), 17);
        assert_eq!(next.minute(), 0);
        // Sunday = 0 in num_days_from_sunday
        assert_eq!(next.weekday().num_days_from_sunday(), 0);
    }

    #[test]
    fn soc_not_100_preserves_existing_next_full_charge() {
        let existing = NaiveDate::from_ymd_opt(2025, 9, 3)
            .unwrap()
            .and_hms_opt(1, 0, 0)
            .unwrap(); // Wednesday, 01:00
        let input = SetpointInput {
            battery_soc: 99.0,
            globals: SetpointInputGlobals {
                next_full_charge: Some(existing),
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());

        let next = out.debug.next_full_charge;
        assert_eq!(next.hour(), 1);
        assert_eq!(next.minute(), 0);
        assert_eq!(next.weekday().num_days_from_sunday(), 3); // Wednesday
    }

    // ------------------------------------------------------------------
    // Edge cases
    // ------------------------------------------------------------------

    #[test]
    fn zero_battery_soc_current_capacity_is_zero() {
        let input = SetpointInput {
            battery_soc: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.debug.current_capacity, 0.0);
        assert!(out.setpoint_target >= -5000);
    }

    #[test]
    fn full_battery_soc_current_capacity_equals_total() {
        let input = SetpointInput {
            battery_soc: 100.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!((out.debug.current_capacity - out.debug.total_capacity).abs() < 1e-9);
    }

    #[test]
    fn zero_power_consumption_passes_through() {
        let input = SetpointInput {
            power_consumption: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.debug.consumption, 0.0);
    }

    #[test]
    fn discharge_is_clamped_to_5000w_floor() {
        // PR-inverter-safe-discharge-knob: this test exists to pin the
        // legacy `max_discharge` safety clamp. Force the knob `true` so
        // the assertion exercises the original 4020 W margin path.
        let input = SetpointInput {
            battery_soc: 100.0,
            power_consumption: 10_000.0,
            globals: SetpointInputGlobals {
                inverter_safe_discharge_enable: true,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 20, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(out.setpoint_target >= -5000);
    }

    #[test]
    fn final_setpoint_is_multiple_of_50() {
        let input = SetpointInput {
            battery_soc: 75.0,
            mppt_power_0: 1000.0,
            mppt_power_1: 777.0,
            power_consumption: 1200.0,
            globals: SetpointInputGlobals {
                export_soc_threshold: 70.0,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target % 50, 0);
    }

    #[test]
    fn non_negative_setpoint_is_promoted_to_10() {
        let input = SetpointInput {
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            power_consumption: 100.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        // Daytime, battery_soc=80, export_soc_threshold=70 (from base),
        // pv_multiplier branch — could produce any number. We just check
        // the ≥0 → 10 rule holds.
        if out.setpoint_target >= 0 {
            assert_eq!(out.setpoint_target, 10);
        }
    }

    // ------------------------------------------------------------------
    // PV-multiplier / pessimism-multiplier shape
    // ------------------------------------------------------------------

    #[test]
    fn pessimism_multiplier_within_range_during_evening() {
        let input = SetpointInput {
            battery_soc: 80.0,
            power_consumption: 1500.0,
            globals: SetpointInputGlobals {
                pessimism_multiplier_modifier: 1.2,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 20, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(out.debug.pessimism_multiplier > 1.0);
        assert!(out.debug.pessimism_multiplier <= 1.8);
    }

    #[test]
    fn pv_multiplier_positive_for_low_threshold() {
        let input = SetpointInput {
            battery_soc: 75.0,
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            globals: SetpointInputGlobals {
                export_soc_threshold: 60.0, // ≤ 67 branch
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(out.debug.pv_multiplier > 1.0);
    }

    #[test]
    fn pv_multiplier_positive_for_high_threshold() {
        let input = SetpointInput {
            battery_soc: 80.0,
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            globals: SetpointInputGlobals {
                export_soc_threshold: 70.0, // > 67 branch
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 12, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(out.debug.pv_multiplier > 1.0);
    }

    #[test]
    fn preserve_battery_fires_when_baseload_would_drain() {
        let input = SetpointInput {
            battery_soc: 30.0,
            capacity: 50.0, // small battery
            soh: 90.0,
            power_consumption: 1500.0,
            globals: SetpointInputGlobals {
                discharge_soc_target: 25.0,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 20, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert!(out.debug.preserve_battery);
    }

    // ------------------------------------------------------------------
    // prepare_setpoint unit tests (the small post-processing helper)
    // ------------------------------------------------------------------

    #[test]
    fn prepare_setpoint_rounds_to_50() {
        assert_eq!(prepare_setpoint(-5000.0, -1237.0, 10.0), -1250);
        assert_eq!(prepare_setpoint(-5000.0, -1212.0, 10.0), -1200);
    }

    #[test]
    fn prepare_setpoint_clamps_to_max_discharge() {
        assert_eq!(prepare_setpoint(-2000.0, -3500.0, 10.0), -2000);
    }

    #[test]
    fn prepare_setpoint_floors_before_rounding() {
        // floor(-100.9) = -101; round(-101/50) = round(-2.02) = -2; -2*50 = -100
        assert_eq!(prepare_setpoint(-5000.0, -100.9, 10.0), -100);
    }

    #[test]
    fn prepare_setpoint_promotes_zero_or_positive_to_10() {
        assert_eq!(prepare_setpoint(-5000.0, 0.0, 10.0), 10);
        assert_eq!(prepare_setpoint(-5000.0, 42.0, 10.0), 10);
        // An input that rounds to exactly 0 also gets promoted.
        assert_eq!(prepare_setpoint(-5000.0, -24.0, 10.0), 10); // floor=-24, /50=-0.48, round=0, *50=0 → ≥0 → 10
    }

    // ------------------------------------------------------------------
    // get_next_charge_date_to_sunday_5pm — date arithmetic
    // ------------------------------------------------------------------

    #[test]
    fn sunday_rollover_for_undefined_lands_on_this_week_sunday_or_next() {
        // 2026-04-21 Tuesday — this week's Sunday was 2026-04-19 (past),
        // so the function should push to 2026-04-26.
        let now = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let d = get_next_charge_date_to_sunday_5pm(now, 0, false);
        assert_eq!(d.weekday().num_days_from_sunday(), 0);
        assert_eq!(d.hour(), 17);
        assert!(d >= now);
    }

    #[test]
    fn sunday_rollover_for_defined_from_friday_goes_forward() {
        // 2026-04-24 Friday — with defined=true and dow=5, expect push to
        // Sunday 2026-04-26.
        let now = NaiveDate::from_ymd_opt(2026, 4, 24)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let d = get_next_charge_date_to_sunday_5pm(now, 0, true);
        assert_eq!(d.weekday().num_days_from_sunday(), 0);
        assert_eq!(d.date(), NaiveDate::from_ymd_opt(2026, 4, 26).unwrap());
        assert_eq!(d.hour(), 17);
    }

    #[test]
    fn sunday_rollover_weeks_1_from_any_day_lands_on_a_sunday() {
        // A Monday.
        let now = NaiveDate::from_ymd_opt(2026, 4, 20)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let d = get_next_charge_date_to_sunday_5pm(now, 1, true);
        assert_eq!(d.weekday().num_days_from_sunday(), 0);
        assert_eq!(d.hour(), 17);
    }
}
