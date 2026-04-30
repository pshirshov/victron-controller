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
    /// DC battery power (W). Sign convention: positive = charging,
    /// negative = discharging. Required-fresh — stale battery DC power
    /// triggers `apply_setpoint_safety` (idle 10 W). PR-ZD-3.
    pub battery_dc_power: f64,
    /// AC power draw of the heat-pump grid-side load (W). Excluded from
    /// the compensated-drain feedback signal. Caller passes `0.0` when
    /// the sensor is stale (conservative: clamps tighter). PR-ZD-3.
    pub heat_pump_power: f64,
    /// AC power draw of the cooker grid-side load (W). Same stale
    /// semantics as `heat_pump_power`. PR-ZD-3.
    pub cooker_power: f64,
    /// Previously-commanded grid setpoint (W). Used as the recurrence
    /// base for the compensated-drain soft loop. Sourced from
    /// `world.grid_setpoint.target.value.unwrap_or(idle_setpoint_w)`.
    /// PR-ZD-3.
    pub setpoint_target_prev: i32,
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
    /// When `true`, the SoC ≥ 99.99 weekly rollover always lands on
    /// the Sunday at-or-after `now + 7d` (never snaps back to the
    /// current week's Sunday). Default `false` preserves legacy
    /// behaviour. Only consulted by the SoC-100 rollover call site;
    /// manually-edited `next_full_charge` values are not retroactively
    /// reinterpreted.
    pub full_charge_defer_to_next_sunday: bool,
    /// Inclusive max weekday (Sun=0, Mon=1, ..., Sat=6) for the
    /// snap-back branch. Replaces the legacy hard-coded `3`. Range
    /// validated upstream; the helper clamps defensively. Only
    /// applies when `full_charge_defer_to_next_sunday` is `false`.
    pub full_charge_snap_back_max_weekday: u32,
    /// Compensated drain threshold (W). When
    /// `compensated_drain > zappi_drain_threshold_w`, the loop
    /// tightens the setpoint. PR-ZD-3.
    pub zappi_drain_threshold_w: u32,
    /// Relax step (W) per tick when compensated drain is below threshold.
    /// The setpoint relaxes by this amount toward `-solar_export`. PR-ZD-3.
    pub zappi_drain_relax_step_w: u32,
    /// Proportional gain for the compensated-drain controller. PR-ZD-3.
    pub zappi_drain_kp: f64,
    /// Reference target for the drain controller (W). Reserved for a
    /// future PI extension; currently inert — the loop uses
    /// `zappi_drain_threshold_w` as its reference. PR-ZD-3.
    pub zappi_drain_target_w: i32,
    /// PR-ZDP-1: probe offset (W) added to the relax target when MPPT
    /// is curtailed (mode 1). Set to 0 to disable probing.
    pub zappi_drain_mppt_probe_w: u32,
    /// PR-ZDP-1: `true` when at least one MPPT reports mode 1
    /// (voltage/current limited — curtailed by the inverter).
    pub mppt_curtailed: bool,
    /// Absolute floor for the probed relax target. The probe cannot
    /// push the target below `-grid_export_limit_w`.
    pub grid_export_limit_w: u32,
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
// PR-soc-chart-export-policy: shared battery-balance helper
// -----------------------------------------------------------------------------

/// Hypothetical world overrides used by [`compute_battery_balance`]. The
/// live setpoint controller passes the current values; the SoC-chart
/// projection passes per-hour hypotheticals (projected SoC, forecast
/// solar, hour-boundary clock) so the projection consults the same
/// branch tree as the live controller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BalanceHypothetical {
    /// SoC % to use instead of `input.battery_soc`. Pass live SoC for
    /// the live tick.
    pub battery_soc: f64,
    /// MPPT solar power (W) to use instead of
    /// `input.mppt_power_0 + input.mppt_power_1`. Pass current sum for
    /// the live tick.
    pub mppt_power_total_w: f64,
    /// Wall-clock `now` (NaiveDateTime). Pass live clock for the live
    /// tick; for projection pass the hour boundary's local-clock time.
    pub now: NaiveDateTime,
}

/// Which branch of the export policy fired. 1:1 with the branch tree of
/// [`evaluate_setpoint`] (and therefore with `BatteryBalanceBranch` →
/// `SocProjectionKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryBalanceBranch {
    /// `force_disable_export` — all surplus to battery.
    ForcedNoExport,
    /// `zappi_active && !allow_battery_to_car` early-morning carve-out.
    PreserveForZappi,
    /// `battery_soc < export_soc_threshold` — only solar surplus exports.
    BelowExportThreshold,
    /// `battery_soc >= export_soc_threshold` discharge window — battery
    /// actively pulled to export.
    EveningDischarge,
    /// Battery at 100 % — no further charge possible.
    BatteryFull,
    /// Anything else / fallback (preserve battery, idle windows).
    Idle,
}

/// What the setpoint controller decided about battery flow at a given
/// instant. Positive `net_battery_w` = battery charging (current
/// flowing into battery); negative = battery discharging.
///
/// Used by the live controller (which converts to a grid setpoint) AND
/// the SoC-chart projection (which integrates it across the horizon to
/// extrapolate SoC). The live and projection paths share the branch
/// tree so the projection cannot disagree with the live controller
/// about what the live controller will do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatteryBalance {
    pub net_battery_w: f64,
    pub branch: BatteryBalanceBranch,
}

/// Pure compute of net battery power. Mirrors the branching of
/// [`evaluate_setpoint`] exactly (single source of truth — drift-guard
/// tests in `convert_soc_chart` keep them aligned). The projection
/// passes hypothetical (soc, solar_w, now) to extrapolate forward; the
/// live tick passes the current values.
///
/// Returns the *modelled* battery power (W) the controller's branch
/// would produce given the inputs, plus a tag identifying which branch
/// fired. The setpoint-target derivation (the part that produces a
/// *grid* setpoint number) stays in [`evaluate_setpoint`].
#[must_use]
#[allow(clippy::float_cmp)]
pub fn compute_battery_balance(
    input: &SetpointInput,
    hw: &HardwareParams,
    h: BalanceHypothetical,
) -> BatteryBalance {
    let g = &input.globals;
    let baseload_w = hw.baseload_consumption_w;
    let nominal_voltage_v = hw.battery_nominal_voltage_v;

    let battery_soc = h.battery_soc;
    let mppt_power = h.mppt_power_total_w;
    let now = h.now;

    let consumption = input.power_consumption;

    // Mirror the full-charge promotion of export_soc_threshold so the
    // branch tree below sees the same effective threshold as the live
    // controller's daytime branches.
    let charge_to_full_required = match g.debug_full_charge {
        DebugFullCharge::Forbid => false,
        DebugFullCharge::Force => true,
        DebugFullCharge::Auto => g.next_full_charge.is_some_and(|nfc| nfc <= now),
    };
    let export_soc_threshold = if charge_to_full_required {
        g.full_charge_export_soc_threshold
    } else {
        g.export_soc_threshold
    };
    let soc_end_of_day_target = if charge_to_full_required {
        g.full_charge_discharge_soc_target.max(g.discharge_soc_target)
    } else {
        g.discharge_soc_target
    };

    // (Removed `if battery_soc >= 99.99 → BatteryFull` short-circuit
    // — it pre-empted the evening branch when projection started at
    // 100% SoC and prevented EveningDischarge slopes from ever
    // appearing on the chart. The projection walker emits its own
    // `Clamped` kind when SoC genuinely hits the ceiling, and the live
    // controller has no such short-circuit either, so removing it
    // brings the helper back in line with `evaluate_setpoint`.)

    let hour = now.hour();
    let minute = now.minute();

    // PV that physically flows into the battery DC bus. MPPTs feed the
    // battery directly; Soltaro feeds the AC bus. Hoymiles is on the EV
    // branch and not modelled here (it never charges the house battery).
    let solar_to_battery_capable_w = mppt_power.max(0.0);
    // House drain that the battery must cover when idle/below-threshold:
    // baseline + any surplus consumption above baseline. Caller controls
    // whether `power_consumption` already includes baseline (live tick)
    // or only Zappi/loads (projection — baseline added explicitly here).
    let baseline_drain_w = baseload_w.max(consumption);

    // 1. force_disable_export.
    if g.force_disable_export {
        // Idle setpoint pinned, so all available solar that can charge
        // the battery does, minus baseline drain.
        return BatteryBalance {
            net_battery_w: solar_to_battery_capable_w - baseline_drain_w,
            branch: BatteryBalanceBranch::ForcedNoExport,
        };
    }

    // 2. zappi_active && !allow_battery_to_car.
    if g.zappi_active && !g.allow_battery_to_car {
        // PR-ZD-3: the live controller now runs a compensated-drain
        // feedback loop (recurrence on the previous setpoint). The loop's
        // intent is to hold battery drain at zero, so `net_battery_w = 0`
        // remains the correct approximation for the SoC-chart projection.
        //
        // Projection-vs-live mismatch: the projection cannot replay the
        // recurrence dynamics (it has no prev-setpoint history), so the
        // chart approximates the Zappi-active window as "battery flat".
        // Chart-parity with the loop is a follow-up, not M-ZAPPI-DRAIN
        // scope. See plan §2 "Out-of-scope".
        return BatteryBalance {
            net_battery_w: 0.0,
            branch: BatteryBalanceBranch::PreserveForZappi,
        };
    }

    // 3. 23:55-00:00 protection window — idle, battery follows surplus.
    if hour == 23 && minute >= 55 {
        return BatteryBalance {
            net_battery_w: solar_to_battery_capable_w - baseline_drain_w,
            branch: BatteryBalanceBranch::Idle,
        };
    }

    // 4. Evening discharge window — `!(2..17).contains(&hour)` excluding
    // the 23:55 sliver above.
    if !(2..17).contains(&hour) {
        // Build the same `hours_remaining` the live controller would.
        let early_discharge = g.discharge_time == DischargeTime::At2300;
        let mut discharge_end_time = now;
        if early_discharge {
            if hour < 2 {
                discharge_end_time -= TimeDelta::days(1);
            }
            discharge_end_time = match discharge_end_time.date().and_hms_opt(23, 0, 0) {
                Some(t) => t,
                None => {
                    return BatteryBalance {
                        net_battery_w: 0.0,
                        branch: BatteryBalanceBranch::Idle,
                    };
                }
            };
        } else {
            if hour > 2 {
                discharge_end_time += TimeDelta::days(1);
            }
            discharge_end_time = match discharge_end_time.date().and_hms_opt(2, 0, 0) {
                Some(t) => t,
                None => {
                    return BatteryBalance {
                        net_battery_w: 0.0,
                        branch: BatteryBalanceBranch::Idle,
                    };
                }
            };
        }
        let hour_before = discharge_end_time - TimeDelta::hours(1);
        #[allow(clippy::cast_precision_loss)]
        let millis_remaining_1hour = (hour_before - now).num_milliseconds() as f64;
        #[allow(clippy::cast_precision_loss)]
        let millis_remaining_end = (discharge_end_time - now).num_milliseconds() as f64;
        let millis_remaining = if millis_remaining_1hour <= 0.0 {
            millis_remaining_end
        } else {
            millis_remaining_1hour
        };

        if millis_remaining <= 0.0 {
            // Past discharge end — idle, battery follows residual surplus.
            return BatteryBalance {
                net_battery_w: solar_to_battery_capable_w - baseline_drain_w,
                branch: BatteryBalanceBranch::Idle,
            };
        }

        let hours_remaining = millis_remaining / (1000.0 * 60.0 * 60.0);
        let pessimism_multiplier = 1.8_f64.min(
            g.pessimism_multiplier_modifier
                * (((hours_remaining / 10.0 + 1.0) * 80.0).round() / 80.0),
        );

        let battery_capacity_ah = input.capacity;
        let total_capacity_wh =
            battery_capacity_ah * (input.soh / 100.0) * nominal_voltage_v;
        let current_capacity = total_capacity_wh * (battery_soc / 100.0);
        let end_of_day_target = total_capacity_wh * (soc_end_of_day_target / 100.0);
        // Mirror evaluate_setpoint: the +3000 Wh "discharge buffer"
        // applies only while we're still inside the 1-hour-before-end
        // window (millis_remaining_1hour > 0). Past the 1-hour mark we
        // squeeze toward end_of_day_target exactly.
        let current_target = if millis_remaining_1hour <= 0.0 {
            end_of_day_target
        } else {
            end_of_day_target + 3000.0
        };
        let to_be_consumed = 0.0_f64.max(consumption * hours_remaining * pessimism_multiplier);
        let exportable_capacity =
            0.0_f64.max(current_capacity - to_be_consumed - current_target);

        let remaining_baseload_consumption = baseload_w * hours_remaining;
        let baseload_to_be_consumed =
            0.0_f64.max(remaining_baseload_consumption * pessimism_multiplier);
        // Mirror evaluate_setpoint: compare against `current_target`
        // (which already includes the +3000 buffer when applicable),
        // not bare `end_of_day_target`.
        let preserve_battery = export_soc_threshold == 100.0
            || current_capacity - current_target < baseload_to_be_consumed;

        if preserve_battery {
            // Setpoint pinned at idle — all surplus stays in battery.
            return BatteryBalance {
                net_battery_w: solar_to_battery_capable_w - baseline_drain_w,
                branch: BatteryBalanceBranch::Idle,
            };
        }

        if battery_soc >= export_soc_threshold {
            // Active discharge window. The live controller targets
            //   setpoint = -(battery_export + solar_export - consumption)
            // (see evaluate_setpoint's evening sub-branch). Conservation
            // (grid_import + solar + battery_discharge = loads) plus the
            // assumption solar_export ≈ mppt (the soltaro/hoymiles
            // contributions are small in the projection horizon) gives
            //   battery_discharge ≈ battery_export
            // So the net flow into the battery is `-battery_export`. The
            // baseline is NOT separately subtracted: it's already folded
            // into the setpoint via the `-consumption` term.
            let battery_export = exportable_capacity / hours_remaining;
            return BatteryBalance {
                net_battery_w: -battery_export,
                branch: BatteryBalanceBranch::EveningDischarge,
            };
        }

        // Below threshold — only solar surplus may export, battery sees
        // its own surplus.
        return BatteryBalance {
            net_battery_w: solar_to_battery_capable_w - baseline_drain_w,
            branch: BatteryBalanceBranch::BelowExportThreshold,
        };
    }

    // 5. Daytime PV-multiplier window (8..17).
    if (8..17).contains(&hour) {
        if export_soc_threshold == 100.0 {
            // Threshold pinned to 100 → idle, battery absorbs surplus.
            return BatteryBalance {
                net_battery_w: solar_to_battery_capable_w - baseline_drain_w,
                branch: BatteryBalanceBranch::Idle,
            };
        }
        if battery_soc < export_soc_threshold {
            // Below threshold — setpoint is idle, all PV charges battery.
            return BatteryBalance {
                net_battery_w: solar_to_battery_capable_w - baseline_drain_w,
                branch: BatteryBalanceBranch::BelowExportThreshold,
            };
        }
        // At/above threshold — daytime PV-multiplier export window.
        // The export rate depends on the multiplier (≥ 1 → battery is
        // actively drained on top of solar export). For projection
        // purposes treat as the same "actively exporting" branch as the
        // evening-discharge case.
        // Conservative model: steady-state intent is to hold battery
        // flat while exporting solar surplus; pessimism multiplier and
        // exact export rate aren't surfaced here.
        return BatteryBalance {
            net_battery_w: 0.0,
            branch: BatteryBalanceBranch::EveningDischarge,
        };
    }

    // 6. Boost / extended-night windows (2..8) — idle.
    BatteryBalance {
        net_battery_w: solar_to_battery_capable_w - baseline_drain_w,
        branch: BatteryBalanceBranch::Idle,
    }
}

// -----------------------------------------------------------------------------
// Shared formula helpers
// -----------------------------------------------------------------------------

/// PR-ZD-3/ZD-4: shared formula for the soft loop (in `evaluate_setpoint`)
/// and the Fast-mode hard clamp (in `process::run_setpoint`).
///
/// Returns the portion of battery discharge NOT explained by the two
/// metered grid-side loads the operator excluded on purpose:
///
///   max(0, -battery_dc_power - heat_pump_w - cooker_w)
///
/// Sign convention: `battery_dc_power > 0` = charging, `< 0` = discharging.
/// Stale HP/cooker are represented as `0.0` by the caller (conservative:
/// clamps tighter on a dead bridge, never looser).
#[must_use]
pub(crate) fn compute_compensated_drain(battery_dc_power: f64, heat_pump_w: f64, cooker_w: f64) -> f64 {
    (0.0_f64).max(-battery_dc_power - heat_pump_w - cooker_w)
}

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

    let mut next_full_charge = g
        .next_full_charge
        // Seed for the local when bookkeeping is `None`. With
        // `defined=false`, the snap-back branch fires regardless of
        // the threshold value (the `(dow > t && false)` subterm is
        // dead). Pass the live knob value anyway so the call site is
        // syntactically uniform with the SoC-100 branch — if a future
        // change ever flips `defined=true` here, the threshold will
        // Just Work.
        .unwrap_or_else(|| {
            get_next_charge_date_to_sunday_5pm(
                now,
                0,
                false,
                false,
                g.full_charge_snap_back_max_weekday,
            )
        });

    let charge_to_full_required = match g.debug_full_charge {
        DebugFullCharge::Forbid => false,
        DebugFullCharge::Force => true,
        DebugFullCharge::Auto => next_full_charge <= now,
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
        // SoC-100 rollover always defines a fresh `next_full_charge`,
        // so pass `defined=true` regardless of whether bookkeeping
        // entered this tick as `None` (cleared by the operator) or
        // `Some(past_dt)`. This makes the threshold knob load-bearing
        // for both paths — pre-fix, `defined=false` short-circuited
        // the threshold subterm and silently snap-backed regardless
        // of the operator's chosen weekday cap.
        next_full_charge = get_next_charge_date_to_sunday_5pm(
            now,
            1,
            true,
            g.full_charge_defer_to_next_sunday,
            g.full_charge_snap_back_max_weekday,
        );
    }

    let hour = now.hour();
    let minute = now.minute();

    // Branches match the TS version's order exactly, with one addition:
    // the zappi_active branch is gated on `!allow_battery_to_car` so the
    // new knob (SPEC §5.9) can bypass the PV-only export clamp and let
    // the regular time-of-day controller run (which may discharge battery
    // through the grid into the EV).
    //
    // LOCKSTEP: `classify_zappi_drain_branch` in `crates/core/src/process.rs`
    // mirrors this branch ladder for observability. If a new branch is added
    // here, the classifier MUST be updated in the same commit.
    let mut decision: Decision;
    if g.force_disable_export {
        setpoint_target = idle_setpoint_w;
        decision = Decision::new("Export killed by force_disable_export → idle 10 W")
            .with_factor("force_disable_export", "true");
    } else if g.zappi_active && !g.allow_battery_to_car {
        // PR-ZD-3: unified compensated-drain feedback loop. Replaces the
        // PV-only clamp (which had a separate early-morning Soltaro carve-out
        // for (2..8) h). Soltaro AC export registers in the battery power
        // balance, so the unified loop handles early-morning surplus without
        // a separate time-of-day branch.
        //
        // compensated_drain = max(0, -battery_dc_power - hp_w - cooker_w)
        // — discharges that are NOT explained by the two metered grid-side
        // loads the operator excluded on purpose. Stale HP/cooker = 0 W
        // (conservative — clamps tighter, never looser on a dead bridge).
        // PR-ZD-4: formula centralised in `compute_compensated_drain`.
        let hp_w = input.heat_pump_power;
        let cooker_w = input.cooker_power;
        let compensated_drain_w =
            compute_compensated_drain(input.battery_dc_power, hp_w, cooker_w);

        let prev = f64::from(input.setpoint_target_prev);
        let threshold = f64::from(g.zappi_drain_threshold_w);

        let tightening = compensated_drain_w > threshold;
        let new_setpoint = if tightening {
            // Tighten — raise (less negative) by kp × excess drain.
            prev + g.zappi_drain_kp * (compensated_drain_w - threshold)
        } else {
            // Relax — step toward the target at relax_step_w per tick
            // from any direction.
            //
            // PR-ZDP-1: when MPPT is curtailed (mode 1), probe deeper
            // than observed solar_export to create demand. The MPPT
            // ramps up to meet the demand; once it's at MPP (mode 2),
            // op-mode flips, probe disengages, target settles at the
            // new (higher) -solar_export. Bounded below by
            // -grid_export_limit_w to stay within operator's ceiling.
            //
            // prev < target means exporting more than target. Step UP
            // (less negative), clamped at target from above.
            //
            // prev > target means exporting less than target. Step DOWN
            // (more negative), clamped at target from below.
            let probe_offset_w = if g.mppt_curtailed {
                f64::from(g.zappi_drain_mppt_probe_w)
            } else {
                0.0
            };
            let absolute_floor = -f64::from(g.grid_export_limit_w);
            let target = (-(solar_export + probe_offset_w)).max(absolute_floor);
            let step = f64::from(g.zappi_drain_relax_step_w);
            if prev < target {
                (prev + step).min(target)
            } else {
                (prev - step).max(target)
            }
        };

        setpoint_target = new_setpoint;

        let common_decision = Decision::new(if tightening {
            "Zappi active — tightening setpoint to halt battery drain into EV"
        } else {
            "Zappi active — relaxing setpoint toward solar-only export"
        })
        .with_factor("zappi_active", "true")
        .with_factor("allow_battery_to_car", "false")
        .with_factor("battery_dc_power_W", format!("{:.0}", input.battery_dc_power))
        .with_factor("heat_pump_W", format!("{hp_w:.0}"))
        .with_factor("cooker_W", format!("{cooker_w:.0}"))
        .with_factor("compensated_drain_W", format!("{compensated_drain_w:.0}"))
        .with_factor("threshold_W", format!("{threshold:.0}"))
        .with_factor("kp", format!("{:.2}", g.zappi_drain_kp))
        .with_factor("solar_export_W", format!("{solar_export:.0}"))
        .with_factor("setpoint_prev_W", format!("{prev:.0}"))
        .with_factor("setpoint_new_W (pre-clamp)", format!("{new_setpoint:.0}"));

        decision = if tightening {
            common_decision
        } else {
            // PR-ZDP-1: relax-branch-only probe factors.
            let probe_offset_w = if g.mppt_curtailed {
                f64::from(g.zappi_drain_mppt_probe_w)
            } else {
                0.0
            };
            common_decision
                .with_factor("mppt_curtailed", format!("{}", g.mppt_curtailed))
                .with_factor("probe_offset_W", format!("{probe_offset_w:.0}"))
        };
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
///
/// `defer_to_next_sunday` (knob `full-charge.defer-to-next-sunday`):
/// when `true`, override the snap-back branch — for any non-Sunday
/// `dow`, push forward to the Sunday at-or-after `now + weeks*7d`
/// rather than dropping back to the prior Sunday.
///
/// `snap_back_max_weekday` (knob `full-charge.snap-back-max-weekday`,
/// range 1..=5, default 3): inclusive cap on the snap-back branch.
/// `dow <= snap_back_max_weekday` snaps to this week's Sunday;
/// `dow > snap_back_max_weekday` pushes forward. The helper clamps
/// defensively to [1, 5] so an out-of-range retained value never
/// produces nonsense (e.g. 0 would force every weekday to push;
/// 6 would suppress the push branch entirely).
fn get_next_charge_date_to_sunday_5pm(
    now: NaiveDateTime,
    weeks: i64,
    next_full_charge_defined: bool,
    defer_to_next_sunday: bool,
    snap_back_max_weekday: u32,
) -> NaiveDateTime {
    let threshold = i64::from(snap_back_max_weekday.clamp(1, 5));
    let mut d = now + TimeDelta::days(weeks * 7);
    d = d
        .date()
        .and_hms_opt(17, 0, 0)
        .expect("hms constants are always valid");
    let dow = i64::from(d.weekday().num_days_from_sunday()); // 0 = Sunday
    let push_forward =
        (dow > threshold && next_full_charge_defined) || (dow > 0 && defer_to_next_sunday);
    if push_forward {
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
                debug_full_charge: DebugFullCharge::Auto,
                pessimism_multiplier_modifier: 1.0,
                next_full_charge: None,
                // PR-inverter-safe-discharge-knob: default for fixtures
                // is `false` (the production default). Tests asserting
                // legacy safety/discharge-cap/max_discharge behaviour
                // override this to `true` per their globals literal.
                inverter_safe_discharge_enable: false,
                full_charge_defer_to_next_sunday: false,
                full_charge_snap_back_max_weekday: 3,
                // PR-ZD-3: safe defaults matching knob safe_defaults().
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                zappi_drain_target_w: 0,
                // PR-ZDP-1: probe off by default in fixtures so existing
                // tests are unaffected (curtailed=false → probe_offset=0
                // regardless of knob value; curtailed=true with knob=0
                // → also 0). Tests that exercise the probe override these.
                zappi_drain_mppt_probe_w: 0,
                mppt_curtailed: false,
                grid_export_limit_w: 5000,
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
            // PR-ZD-3: battery DC power. Sign: positive = charging,
            // negative = discharging. 0.0 in the base fixture (battery flat).
            battery_dc_power: 0.0,
            // PR-ZD-3: grid-side loads (W). 0.0 = no metered load.
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            // PR-ZD-3: previously-commanded setpoint (W). Default to
            // idle (10 W) so the relax branch stays near zero in tests
            // that don't exercise the Zappi soft loop.
            setpoint_target_prev: 10,
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
    // PR-ZD-3: Zappi compensated-drain soft loop
    // ------------------------------------------------------------------

    // Test 15: battery=-1500, HP=1000, cooker=500 →
    // compensated_drain = max(0, 1500-1000-500) = 0; relax branch fires.
    #[test]
    fn compensated_drain_zero_when_loads_explain_battery_flow() {
        // Battery discharging 1500 W; HP+cooker explain it entirely.
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                // threshold=1000; drain=0 < 1000 → relax branch
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -1500.0,
            heat_pump_power: 1000.0,
            cooker_power: 500.0,
            setpoint_target_prev: -3000,
            // solar_export = mppt0+mppt1+soltaro = 4000
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            soltaro_power: 500.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // Relax: prev=-3000 > target=-4000 → step DOWN:
        //   (prev - step).max(target) = (-3000-100).max(-4000) = -3100
        // prepare_setpoint: max(-5000, floor(-3100)) = -3100; /50=-62; *50=-3100
        assert_eq!(out.setpoint_target, -3100);
        assert!(out.decision.summary.contains("relaxing"));
    }

    // Test 16: battery=+2000 (charging) → compensated_drain = max(0,-2000)=0; relax.
    #[test]
    fn compensated_drain_clamped_zero_when_battery_charging() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: 2000.0, // charging
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -3000,
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            soltaro_power: 500.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // compensated_drain = max(0, -2000 - 0 - 0) = 0 < 1000 → relax
        assert!(out.decision.summary.contains("relaxing"));
        // D13: verify the clamp to zero specifically — if clamp is removed the
        // factor would be negative (reflecting the charging value).
        let drain_factor = out.decision.factors.iter()
            .find(|f| f.name == "compensated_drain_W")
            .expect("compensated_drain_W factor must be present");
        assert_eq!(drain_factor.value, "0", "compensated_drain_W must be clamped to 0 when battery is charging");
    }

    // Test 17: battery=-2500, HP=0, cooker=0, threshold=1000, kp=1.0, prev=-3000
    // → drain=2500, excess=1500, new = -3000+1500 = -1500.
    #[test]
    fn tightens_setpoint_when_drain_exceeds_threshold() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -2500.0, // discharging 2500 W
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -3000,
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // new_setpoint = -3000 + 1.0 * (2500 - 1000) = -1500
        // prepare_setpoint: max(-5000, floor(-1500)) = -1500; /50 = -30; *50 = -1500
        assert_eq!(out.setpoint_target, -1500);
        assert!(out.decision.summary.contains("tightening"));
    }

    // D03 new test: kp ≠ 1.0 — verifies the multiplication scales correctly.
    #[test]
    fn tighten_scales_with_kp() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 0.3, // ≠ 1.0 — exercises the multiplication
                ..base_input().globals
            },
            battery_dc_power: -3000.0, // drain = 3000
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -5000,
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // new = -5000 + 0.3 * (3000 - 1000) = -5000 + 600 = -4400
        // prepare_setpoint: floor(-4400) = -4400; /50 = -88; *50 = -4400
        assert_eq!(out.setpoint_target, -4400);
        assert!(out.decision.summary.contains("tightening"));
    }

    // Test 18: battery=-500, threshold=1000, prev=-3000, solar=2000, relax_step=100
    // → target = -2000. prev(-3000) < target(-2000) → step UP:
    //   (prev + step).min(target) = (-3000+100).min(-2000) = -2900.
    // The new bidirectional formula walks one step per tick, not jumping
    // to the target in one go (the old buggy formula used .max which
    // clamped to -2000 immediately from below).
    #[test]
    fn relaxes_setpoint_toward_minus_solar_export_when_drain_below_threshold() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -500.0,
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -3000,
            // solar_export = 2000, so target = -2000
            mppt_power_0: 2000.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // prev=-3000 < target=-2000 → step UP: (-3000+100).min(-2000) = -2900
        assert_eq!(out.setpoint_target, -2900);
        assert!(out.decision.summary.contains("relaxing"));
    }

    // D01 new test: prev=-100 (above target=-2000), drain < threshold → relax DOWN.
    // Verifies the bidirectional relax works from a setpoint that is above the target
    // (e.g. after boot or after a tighten cycle produced a less-negative value).
    #[test]
    fn relaxes_setpoint_from_above_target_toward_minus_solar_export() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            // drain = max(0, -0 - 0 - 0) = 0 < 1000 → relax branch
            battery_dc_power: 0.0,
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -100,
            // solar_export = 2000, so target = -2000
            mppt_power_0: 2000.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // prev=-100 > target=-2000 → step DOWN: (-100-100).max(-2000) = -200
        assert_eq!(out.setpoint_target, -200);
        assert!(out.decision.summary.contains("relaxing"));
    }

    // Test 19: stale HP treated as 0.0 by caller (verify arithmetic; no safety fallback).
    #[test]
    fn stale_heat_pump_treated_as_zero() {
        // HP stale → caller passes 0.0. Battery discharging 2000 W;
        // no other loads. drain = max(0, 2000-0-0) = 2000 > threshold → tighten.
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -2000.0,
            heat_pump_power: 0.0, // stale HP → 0
            cooker_power: 0.0,
            setpoint_target_prev: -4000,
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // new = -4000 + 1.0*(2000-1000) = -3000
        assert_eq!(out.setpoint_target, -3000);
        assert!(out.decision.summary.contains("tightening"));
    }

    // Test 20: stale cooker treated as 0.0 (symmetric to test 19).
    #[test]
    fn stale_cooker_treated_as_zero() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -2000.0,
            heat_pump_power: 0.0,
            cooker_power: 0.0, // stale cooker → 0
            setpoint_target_prev: -4000,
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // new = -4000 + 1.0*(2000-1000) = -3000
        assert_eq!(out.setpoint_target, -3000);
        assert!(out.decision.summary.contains("tightening"));
    }

    // D09 new test: 03:00, Zappi active, battery draining hard → tighten fires.
    // Confirms the unified loop handles early-morning battery drain that the
    // deleted (2..8) Soltaro-only branch could not address.
    #[test]
    fn early_morning_zappi_tightens_when_battery_draining() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -3000.0, // 3 kW drain
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            soltaro_power: 0.0, // no Soltaro export (deleted branch's trigger)
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            setpoint_target_prev: -2000,
            ..base_input()
        };
        // 03:00 — formerly inside the (2..8) time-of-day carve-out
        let c = clock_at(2026, 1, 15, 3, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        // drain=3000 > threshold=1000 → tighten.
        // new = -2000 + 1.0*(3000-1000) = 0; prepare_setpoint promotes to 10.
        assert_eq!(out.setpoint_target, 10);
        assert!(out.decision.summary.contains("tightening"));
    }

    // Test 21: clock at 03:00 (inside old (2..8) carve-out), Zappi active,
    // soltaro=2000, battery=-100, HP=0, cooker=0 → drain=100 < 1000 → relax.
    // Confirms the deleted early-morning (2..8) branch is no longer needed.
    #[test]
    fn early_morning_zappi_handled_by_unified_loop() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -100.0,
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            soltaro_power: 2000.0,
            // solar_export = 2000 (soltaro only; mppts produce 0 at 03:00)
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            setpoint_target_prev: -4000,
            ..base_input()
        };
        // 03:00 — previously the (2..8) carve-out fired here
        let c = clock_at(2026, 1, 15, 3, 0);
        let out = evaluate_setpoint(&input, &c, &hw());
        // drain = max(0, 100) = 100 < 1000 → relax toward target=-solar_export=-2000
        // prev=-4000 < target=-2000 → step UP: (-4000+100).min(-2000) = -3900
        assert_eq!(out.setpoint_target, -3900);
        assert!(out.decision.summary.contains("relaxing"));
    }

    // Test 22: assert all 11 decision factors are populated.
    #[test]
    fn zappi_active_decision_factors_present() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -2000.0,
            heat_pump_power: 500.0,
            cooker_power: 300.0,
            setpoint_target_prev: -3000,
            mppt_power_0: 1500.0,
            mppt_power_1: 500.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        let factors = &out.decision.factors;
        // Verify all 11 required factors are present.
        let factor_keys: Vec<&str> = factors.iter().map(|f| f.name.as_str()).collect();
        assert!(factor_keys.contains(&"zappi_active"), "missing zappi_active");
        assert!(factor_keys.contains(&"allow_battery_to_car"), "missing allow_battery_to_car");
        assert!(factor_keys.contains(&"battery_dc_power_W"), "missing battery_dc_power_W");
        assert!(factor_keys.contains(&"heat_pump_W"), "missing heat_pump_W");
        assert!(factor_keys.contains(&"cooker_W"), "missing cooker_W");
        assert!(factor_keys.contains(&"compensated_drain_W"), "missing compensated_drain_W");
        assert!(factor_keys.contains(&"threshold_W"), "missing threshold_W");
        assert!(factor_keys.contains(&"kp"), "missing kp");
        assert!(factor_keys.contains(&"solar_export_W"), "missing solar_export_W");
        assert!(factor_keys.contains(&"setpoint_prev_W"), "missing setpoint_prev_W");
        assert!(factor_keys.contains(&"setpoint_new_W (pre-clamp)"), "missing setpoint_new_W (pre-clamp)");
    }

    // D08 new test: verify load-bearing factor VALUES are correct (not just names).
    #[test]
    fn zappi_active_decision_factor_values_correct() {
        // battery=-2500, HP=300, cooker=200, threshold=1000, kp=1.0
        // compensated_drain = max(0, 2500-300-200) = 2000 > 1000 → tighten
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -2500.0,
            heat_pump_power: 300.0,
            cooker_power: 200.0,
            setpoint_target_prev: -3000,
            // solar_export = mppt0+mppt1 = 2000
            mppt_power_0: 2000.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        let factors = &out.decision.factors;
        let get = |name: &str| {
            factors
                .iter()
                .find(|f| f.name == name)
                .map_or("(missing)", |f| f.value.as_str())
        };
        // compensated_drain = max(0, 2500-300-200) = 2000
        assert_eq!(get("compensated_drain_W"), "2000");
        assert_eq!(get("threshold_W"), "1000");
        assert_eq!(get("kp"), "1.00");
        assert_eq!(get("solar_export_W"), "2000");
        // setpoint_new = -3000 + 1.0*(2000-1000) = -2000
        assert_eq!(get("setpoint_new_W (pre-clamp)"), "-2000");
    }

    // Test 23: allow_battery_to_car=true → Zappi branch skipped; evening branch runs.
    // ------------------------------------------------------------------
    // allow_battery_to_car toggle — SPEC §5.9
    // ------------------------------------------------------------------

    #[test]
    fn zappi_branch_bypassed_when_allow_battery_to_car_true() {
        // Evening time, zappi active, allow_battery_to_car=true — the
        // Zappi-specific branch is bypassed and the evening discharge
        // controller runs. We don't assert a specific setpoint value,
        // just that it's NOT the zappi branch (which would not set
        // hours_remaining).
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

    // Test 24: force_disable_export=true short-circuits zappi branch → idle 10 W.
    #[test]
    fn force_disable_export_takes_priority_over_zappi_branch() {
        // Kill switch still wins.
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                force_disable_export: true,
                zappi_active: true,
                allow_battery_to_car: false,
                ..base_input().globals
            },
            battery_dc_power: -2000.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        assert_eq!(out.setpoint_target, 10);
    }

    // Test 26: bookkeeping fields not disturbed by unified Zappi branch.
    // hours_remaining stays at sentinel (-1) since the Zappi branch fires.
    #[test]
    fn bookkeeping_unchanged_for_unified_zappi_branch() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -500.0,
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -3000,
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            soltaro_power: 500.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // The Zappi branch doesn't touch hours_remaining, exportable_capacity,
        // to_be_consumed, or pv_multiplier — they remain at their sentinel -1.
        assert!((out.debug.hours_remaining + 1.0).abs() < f64::EPSILON,
            "hours_remaining should be sentinel -1, got {}", out.debug.hours_remaining);
        assert!((out.debug.exportable_capacity + 1.0).abs() < f64::EPSILON,
            "exportable_capacity should be sentinel -1, got {}", out.debug.exportable_capacity);
        assert!((out.debug.to_be_consumed + 1.0).abs() < f64::EPSILON,
            "to_be_consumed should be sentinel -1, got {}", out.debug.to_be_consumed);
        assert!((out.debug.pv_multiplier + 1.0).abs() < f64::EPSILON,
            "pv_multiplier should be sentinel -1, got {}", out.debug.pv_multiplier);
    }

    // D07 new test: bookkeeping unchanged when the tighten branch fires.
    #[test]
    fn bookkeeping_unchanged_in_tighten_branch() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                ..base_input().globals
            },
            battery_dc_power: -3000.0, // drain=3000 > threshold → tighten
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -5000,
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            soltaro_power: 500.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // Tighten fires; bookkeeping sentinels must remain at -1.
        assert!((out.debug.hours_remaining + 1.0).abs() < f64::EPSILON,
            "hours_remaining should be sentinel -1, got {}", out.debug.hours_remaining);
        assert!((out.debug.exportable_capacity + 1.0).abs() < f64::EPSILON,
            "exportable_capacity should be sentinel -1, got {}", out.debug.exportable_capacity);
        assert!((out.debug.to_be_consumed + 1.0).abs() < f64::EPSILON,
            "to_be_consumed should be sentinel -1, got {}", out.debug.to_be_consumed);
        assert!((out.debug.pv_multiplier + 1.0).abs() < f64::EPSILON,
            "pv_multiplier should be sentinel -1, got {}", out.debug.pv_multiplier);
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
                debug_full_charge: DebugFullCharge::Auto,
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

    /// Regression for the seed-unwrap gap: when `next_full_charge` is
    /// `None` at SoC ≥ 99.99, the SoC-100 rollover used to ignore
    /// `full_charge_snap_back_max_weekday` because `defined=false`
    /// short-circuited the threshold subterm. The fix passes
    /// `defined=true` at the SoC-100 site so the knob governs both
    /// the cleared-bookkeeping and past-datetime paths identically.
    #[test]
    fn soc_100_with_cleared_bookkeeping_honors_snap_back_threshold() {
        // 2026-04-21 Tuesday 10:00 → now+7d = 2026-04-28 Tuesday
        // (dow=2). With cap=3 (legacy default), dow ≤ 3 → snap back
        // to 2026-04-26. With cap=1, dow > 1 → push to 2026-05-03.
        // Both globals set `next_full_charge: None`.
        let with_legacy_cap = SetpointInput {
            battery_soc: 100.0,
            globals: SetpointInputGlobals {
                next_full_charge: None,
                full_charge_snap_back_max_weekday: 3,
                ..base_input().globals
            },
            ..base_input()
        };
        let with_low_cap = SetpointInput {
            battery_soc: 100.0,
            globals: SetpointInputGlobals {
                next_full_charge: None,
                full_charge_snap_back_max_weekday: 1,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 4, 21, 10, 0);
        let legacy = evaluate_setpoint(&with_legacy_cap, &c, &hw())
            .debug
            .next_full_charge;
        let low = evaluate_setpoint(&with_low_cap, &c, &hw())
            .debug
            .next_full_charge;
        assert_eq!(legacy.date(), NaiveDate::from_ymd_opt(2026, 4, 26).unwrap());
        assert_eq!(low.date(), NaiveDate::from_ymd_opt(2026, 5, 3).unwrap());
    }

    /// Companion: setting bookkeeping to a past datetime produces the
    /// SAME rollover destination as clearing it does — the threshold
    /// is consulted in both cases. Pre-fix these would diverge.
    #[test]
    fn soc_100_rollover_destination_independent_of_seed_definedness() {
        let past = NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let cleared = SetpointInput {
            battery_soc: 100.0,
            globals: SetpointInputGlobals {
                next_full_charge: None,
                full_charge_snap_back_max_weekday: 1,
                ..base_input().globals
            },
            ..base_input()
        };
        let with_past = SetpointInput {
            battery_soc: 100.0,
            globals: SetpointInputGlobals {
                next_full_charge: Some(past),
                full_charge_snap_back_max_weekday: 1,
                ..base_input().globals
            },
            ..base_input()
        };
        let c = clock_at(2026, 4, 21, 10, 0);
        let cleared_next = evaluate_setpoint(&cleared, &c, &hw()).debug.next_full_charge;
        let past_next = evaluate_setpoint(&with_past, &c, &hw()).debug.next_full_charge;
        assert_eq!(cleared_next, past_next);
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
        let d = get_next_charge_date_to_sunday_5pm(now, 0, false, false, 3);
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
        let d = get_next_charge_date_to_sunday_5pm(now, 0, true, false, 3);
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
        let d = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 3);
        assert_eq!(d.weekday().num_days_from_sunday(), 0);
        assert_eq!(d.hour(), 17);
    }

    /// `full_charge_defer_to_next_sunday=true` overrides the snap-back
    /// branch for Mon/Tue/Wed: with `weeks=1`, `now+7d` is Mon/Tue/Wed
    /// — legacy snaps back to that week's Sunday (≈4-6 days out); the
    /// knob pushes to the Sunday after instead (≈8-10 days out).
    #[test]
    fn defer_to_next_sunday_pushes_forward_from_monday_seed() {
        // 2026-04-20 Monday 10:00 → now+7d = 2026-04-27 Monday → legacy
        // snaps back to 2026-04-26 Sunday; knob ON pushes to 2026-05-03.
        let now = NaiveDate::from_ymd_opt(2026, 4, 20)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let legacy = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 3);
        let deferred = get_next_charge_date_to_sunday_5pm(now, 1, true, true, 3);
        assert_eq!(legacy.date(), NaiveDate::from_ymd_opt(2026, 4, 26).unwrap());
        assert_eq!(deferred.date(), NaiveDate::from_ymd_opt(2026, 5, 3).unwrap());
        assert_eq!(legacy.hour(), 17);
        assert_eq!(deferred.hour(), 17);
    }

    /// On a Sunday seed (`now+7d` lands on Sunday), the knob is a no-op
    /// — both paths return that Sunday at 17:00.
    #[test]
    fn defer_to_next_sunday_no_op_when_seed_already_sunday() {
        // 2026-04-19 Sunday 10:00 → now+7d = 2026-04-26 Sunday → both
        // legacy and deferred return 2026-04-26 17:00.
        let now = NaiveDate::from_ymd_opt(2026, 4, 19)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let legacy = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 3);
        let deferred = get_next_charge_date_to_sunday_5pm(now, 1, true, true, 3);
        assert_eq!(legacy, deferred);
        assert_eq!(legacy.date(), NaiveDate::from_ymd_opt(2026, 4, 26).unwrap());
    }

    /// On Thu/Fri/Sat seeds the legacy branch already pushes forward
    /// when `defined=true`, so the knob is also a no-op there.
    #[test]
    fn defer_to_next_sunday_matches_legacy_push_branch() {
        // 2026-04-24 Friday 10:00 → now+7d = 2026-05-01 Friday (dow=5).
        // Legacy with defined=true pushes to 2026-05-03; knob ON should
        // produce the same.
        let now = NaiveDate::from_ymd_opt(2026, 4, 24)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let legacy = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 3);
        let deferred = get_next_charge_date_to_sunday_5pm(now, 1, true, true, 3);
        assert_eq!(legacy, deferred);
        assert_eq!(legacy.date(), NaiveDate::from_ymd_opt(2026, 5, 3).unwrap());
    }

    /// With `defined=false`, legacy *always* snaps back regardless of
    /// dow. Knob ON instead pushes to the upcoming Sunday for any
    /// non-Sunday seed.
    #[test]
    fn defer_to_next_sunday_overrides_undefined_snapback() {
        // 2026-04-24 Friday 10:00 (dow=5 of seed). Legacy with
        // defined=false → snap back to this week's Sunday 2026-04-19
        // (in the past) → +7d safety push → 2026-04-26.
        // Knob ON with defined=false → push forward → 2026-04-26 (same
        // result here because the safety net rescued the past date).
        // Use weeks=1 to avoid the safety net interfering: seed
        // 2026-04-24 Friday with weeks=1 = 2026-05-01 Friday (dow=5),
        // legacy snap-back = 2026-04-26, knob ON push = 2026-05-03.
        let now = NaiveDate::from_ymd_opt(2026, 4, 24)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let legacy = get_next_charge_date_to_sunday_5pm(now, 1, false, false, 3);
        let deferred = get_next_charge_date_to_sunday_5pm(now, 1, false, true, 3);
        assert_eq!(legacy.date(), NaiveDate::from_ymd_opt(2026, 4, 26).unwrap());
        assert_eq!(deferred.date(), NaiveDate::from_ymd_opt(2026, 5, 3).unwrap());
    }

    /// Lowering `snap_back_max_weekday` to 1 (Mon) makes Tue/Wed
    /// also push forward — the threshold replaces the legacy hard-
    /// coded `> 3`. Seed Tuesday 2026-04-21, weeks=1 → now+7d =
    /// Tuesday 2026-04-28 (dow=2). With cap=3 (legacy), dow ≤ 3
    /// → snap-back to 2026-04-26. With cap=1, dow > 1 → push to
    /// 2026-05-03.
    #[test]
    fn snap_back_threshold_lowered_pushes_tuesday_forward() {
        let now = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let with_legacy_cap = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 3);
        let with_low_cap = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 1);
        assert_eq!(with_legacy_cap.date(), NaiveDate::from_ymd_opt(2026, 4, 26).unwrap());
        assert_eq!(with_low_cap.date(), NaiveDate::from_ymd_opt(2026, 5, 3).unwrap());
    }

    /// Raising `snap_back_max_weekday` to 5 (Fri) makes Thursday
    /// snap back instead of pushing forward. Seed 2026-04-23
    /// Thursday → now+7d = 2026-04-30 Thursday (dow=4). With
    /// cap=3 (legacy), dow > 3 → push to 2026-05-03. With cap=5,
    /// dow ≤ 5 → snap back to this week's Sunday 2026-04-26.
    #[test]
    fn snap_back_threshold_raised_snaps_thursday_back() {
        let now = NaiveDate::from_ymd_opt(2026, 4, 23)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let with_legacy_cap = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 3);
        let with_high_cap = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 5);
        assert_eq!(with_legacy_cap.date(), NaiveDate::from_ymd_opt(2026, 5, 3).unwrap());
        assert_eq!(with_high_cap.date(), NaiveDate::from_ymd_opt(2026, 4, 26).unwrap());
    }

    /// `defer_to_next_sunday=true` overrides the threshold entirely
    /// — even with cap=5 (which would normally snap back), defer
    /// pushes for any non-Sunday `dow`.
    #[test]
    fn defer_overrides_threshold() {
        let now = NaiveDate::from_ymd_opt(2026, 4, 21)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let d = get_next_charge_date_to_sunday_5pm(now, 1, true, true, 5);
        assert_eq!(d.date(), NaiveDate::from_ymd_opt(2026, 5, 3).unwrap());
    }

    /// Out-of-range threshold (0 or > 5) is clamped to [1, 5].
    /// Threshold 0 would otherwise push every weekday; clamping to
    /// 1 preserves Mon snap-back. Threshold 99 clamps to 5.
    #[test]
    fn snap_back_threshold_clamps_out_of_range() {
        let now = NaiveDate::from_ymd_opt(2026, 4, 20)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap();
        let clamped_low = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 0);
        let clamped_low_ref = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 1);
        assert_eq!(clamped_low, clamped_low_ref);
        let clamped_high = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 99);
        let clamped_high_ref = get_next_charge_date_to_sunday_5pm(now, 1, true, false, 5);
        assert_eq!(clamped_high, clamped_high_ref);
    }

    // ------------------------------------------------------------------
    // PR-soc-chart-export-policy: compute_battery_balance unit tests
    // ------------------------------------------------------------------

    fn now_at(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d)
            .unwrap()
            .and_hms_opt(h, min, 0)
            .unwrap()
    }

    #[test]
    fn balance_force_disable_export_returns_charging() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                force_disable_export: true,
                ..base_input().globals
            },
            mppt_power_0: 2000.0,
            mppt_power_1: 1500.0,
            power_consumption: 1500.0, // matches baseload
            ..base_input()
        };
        let h = BalanceHypothetical {
            battery_soc: 60.0,
            mppt_power_total_w: 3500.0,
            now: now_at(2026, 1, 15, 12, 0),
        };
        let b = compute_battery_balance(&input, &hw(), h);
        assert_eq!(b.branch, BatteryBalanceBranch::ForcedNoExport);
        // 3500 W mppt - max(1500 baseload, 1500 consumption) = 2000
        assert!((b.net_battery_w - 2000.0).abs() < 1e-9, "got {}", b.net_battery_w);
    }

    #[test]
    fn balance_below_export_threshold_returns_solar_only() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                export_soc_threshold: 70.0,
                ..base_input().globals
            },
            mppt_power_0: 5000.0,
            mppt_power_1: 0.0,
            power_consumption: 1500.0,
            ..base_input()
        };
        let h = BalanceHypothetical {
            battery_soc: 60.0, // below 70 threshold
            mppt_power_total_w: 5000.0,
            now: now_at(2026, 1, 15, 12, 0),
        };
        let b = compute_battery_balance(&input, &hw(), h);
        assert_eq!(b.branch, BatteryBalanceBranch::BelowExportThreshold);
        // 5000 - 1500 = 3500
        assert!((b.net_battery_w - 3500.0).abs() < 1e-9);
    }

    #[test]
    fn balance_evening_discharge_returns_negative() {
        // Big battery so the preserve_battery clamp doesn't fire — this
        // test pins the discharge-window arithmetic specifically.
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                export_soc_threshold: 70.0,
                discharge_soc_target: 25.0,
                discharge_time: DischargeTime::At0200,
                ..base_input().globals
            },
            battery_soc: 90.0,
            soh: 100.0,
            capacity: 800.0, // 800 Ah * 48 V = 38.4 kWh
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            power_consumption: 1500.0,
            ..base_input()
        };
        let h = BalanceHypothetical {
            battery_soc: 90.0,
            mppt_power_total_w: 0.0,
            now: now_at(2026, 1, 15, 22, 0), // late evening, ~4 h remaining
        };
        let b = compute_battery_balance(&input, &hw(), h);
        assert_eq!(b.branch, BatteryBalanceBranch::EveningDischarge);
        assert!(b.net_battery_w < 0.0, "expected discharge, got {}", b.net_battery_w);
    }

    /// At SoC = 100% in the daytime window, the helper now lands in
    /// the daytime branch's "actively exporting" arm (slope 0, tagged
    /// EveningDischarge) instead of the removed BatteryFull
    /// short-circuit. The projection walker emits its own `Clamped`
    /// kind when SoC genuinely sits at the ceiling — see
    /// `convert_soc_chart::compute_projection`.
    #[test]
    fn balance_at_full_soc_daytime_returns_zero_via_export_arm() {
        let input = SetpointInput {
            mppt_power_0: 1000.0,
            mppt_power_1: 0.0,
            power_consumption: 800.0,
            ..base_input()
        };
        let h = BalanceHypothetical {
            battery_soc: 100.0,
            mppt_power_total_w: 1000.0,
            now: now_at(2026, 1, 15, 12, 0),
        };
        let b = compute_battery_balance(&input, &hw(), h);
        // Daytime + above export threshold → "actively exporting" arm
        // returns net = 0 with EveningDischarge tag.
        assert_eq!(b.branch, BatteryBalanceBranch::EveningDischarge);
        assert_eq!(b.net_battery_w, 0.0);
    }

    /// The fix that made the chart's evening drain visible: at SoC =
    /// 100% in the EVENING window, the helper must land in the active-
    /// discharge arm of the evening branch and return a NEGATIVE net
    /// battery flow. Pre-fix this case hit the BatteryFull short-
    /// circuit and returned 0, masking the controller's actual
    /// evening behaviour on the chart.
    ///
    /// Fixture pins to the 1-hour-before-discharge-end window
    /// (At0200 + now=01:00 → millis_remaining_1hour is 0, so the
    /// `current_target` drops the +3000 Wh buffer and the headroom
    /// exceeds the pessimism-baseload check).
    #[test]
    fn balance_at_full_soc_evening_returns_negative_via_evening_branch() {
        let input = SetpointInput {
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            power_consumption: 600.0,
            ..base_input()
        };
        let h = BalanceHypothetical {
            battery_soc: 100.0,
            mppt_power_total_w: 0.0,
            now: now_at(2026, 1, 16, 1, 0),
        };
        let b = compute_battery_balance(&input, &hw(), h);
        assert_eq!(b.branch, BatteryBalanceBranch::EveningDischarge);
        assert!(
            b.net_battery_w < 0.0,
            "evening drain at SoC=100 must show negative net flow, got {}",
            b.net_battery_w,
        );
    }

    #[test]
    fn balance_zappi_carveout_preserves_battery() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                ..base_input().globals
            },
            mppt_power_0: 3000.0,
            mppt_power_1: 0.0,
            power_consumption: 5000.0, // includes Zappi
            ..base_input()
        };
        let h = BalanceHypothetical {
            battery_soc: 80.0,
            mppt_power_total_w: 3000.0,
            now: now_at(2026, 1, 15, 14, 0),
        };
        let b = compute_battery_balance(&input, &hw(), h);
        assert_eq!(b.branch, BatteryBalanceBranch::PreserveForZappi);
        assert_eq!(b.net_battery_w, 0.0);
    }

    // ------------------------------------------------------------------
    // PR-soc-chart-evening-discharge: evening-branch helper tests
    // ------------------------------------------------------------------

    /// Evening branch must return a negative net_battery_w (i.e.
    /// discharging) at the magnitude `exportable_capacity / hours_remaining`.
    /// Pin the exact value so a future drift in the helper math gets
    /// caught here, not silently downstream in the chart.
    #[test]
    fn balance_evening_discharge_emits_negative_net_battery_w() {
        // Big battery so preserve_battery clamp doesn't fire.
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                export_soc_threshold: 70.0,
                discharge_soc_target: 25.0,
                discharge_time: DischargeTime::At0200,
                pessimism_multiplier_modifier: 1.0,
                ..base_input().globals
            },
            battery_soc: 90.0,
            soh: 100.0,
            capacity: 800.0, // 800 Ah * 48 V = 38.4 kWh
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            power_consumption: 1500.0,
            ..base_input()
        };
        let h = BalanceHypothetical {
            battery_soc: 90.0,
            mppt_power_total_w: 0.0,
            now: now_at(2026, 1, 15, 22, 0),
        };
        let b = compute_battery_balance(&input, &hw(), h);
        assert_eq!(b.branch, BatteryBalanceBranch::EveningDischarge);

        // Reproduce the helper's arithmetic to pin the slope exactly.
        // discharge_end_time = 02:00 next day; hour_before = 01:00
        // next day; now = 22:00 → millis_remaining_1hour = 3 h (positive).
        let total_capacity_wh = 800.0 * 1.0 * 48.0; // 38400
        let current_capacity = total_capacity_wh * 0.90; // 34560
        let end_of_day_target = total_capacity_wh * 0.25; // 9600
        let current_target = end_of_day_target + 3000.0; // 12600
        let hours_remaining = 3.0_f64;
        // pessimism_multiplier = min(1.8, 1.0 * round((3/10 + 1)*80)/80)
        //                      = min(1.8, 1.3) = 1.3
        let pessimism = 1.3_f64;
        let to_be_consumed = 1500.0 * hours_remaining * pessimism;
        let exportable = (current_capacity - to_be_consumed - current_target).max(0.0);
        let battery_export = exportable / hours_remaining;
        let expected = -battery_export;
        assert!(
            (b.net_battery_w - expected).abs() < 1.0,
            "expected {expected}, got {}",
            b.net_battery_w
        );
        assert!(b.net_battery_w < 0.0);
    }

    /// `preserve_battery=true` must short-circuit to non-discharging
    /// (helper folds it into Idle). With a small battery whose
    /// remaining headroom is dominated by baseload, preserve_battery
    /// fires and the controller pins setpoint to idle — net battery
    /// flow is bounded by `solar - load`, never deeper.
    #[test]
    fn balance_evening_discharge_respects_preserve_battery() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                export_soc_threshold: 70.0,
                discharge_soc_target: 25.0,
                discharge_time: DischargeTime::At0200,
                pessimism_multiplier_modifier: 1.0,
                ..base_input().globals
            },
            battery_soc: 80.0,
            soh: 100.0,
            // Small battery: 100 Ah × 48 V = 4800 Wh. With SoC=80 →
            // current_capacity ≈ 3840 Wh. End-of-day target at 25 % +
            // 3000 buffer ≈ 4200 Wh (already above current_capacity),
            // so preserve_battery fires.
            capacity: 100.0,
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            power_consumption: 1500.0,
            ..base_input()
        };
        let h = BalanceHypothetical {
            battery_soc: 80.0,
            mppt_power_total_w: 0.0,
            now: now_at(2026, 1, 15, 22, 0),
        };
        let b = compute_battery_balance(&input, &hw(), h);
        // preserve_battery → Idle (per helper). Not EveningDischarge.
        assert_eq!(b.branch, BatteryBalanceBranch::Idle);
        // The Idle branch returns solar − load. With solar=0, load=1500,
        // baseload=1500 (default hw), result is -1500. NOT a deeper
        // discharge driven by `battery_export`.
        assert!(
            b.net_battery_w >= -1501.0 && b.net_battery_w <= -1499.0,
            "preserve_battery should bound net to ~-baseline, got {}",
            b.net_battery_w
        );
    }

    /// At SoC = `discharge_soc_target` the exportable_capacity clamps
    /// to 0; net_battery_w must not exceed (in magnitude) the
    /// baseline-to-load drain the controller would otherwise pin to
    /// idle. With this configuration preserve_battery fires (the only
    /// remaining headroom IS exactly the baseload buffer), so the
    /// branch is Idle and net = solar − load.
    #[test]
    fn balance_evening_discharge_clamps_at_target_soc() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                export_soc_threshold: 50.0,
                discharge_soc_target: 30.0,
                discharge_time: DischargeTime::At0200,
                pessimism_multiplier_modifier: 1.0,
                ..base_input().globals
            },
            battery_soc: 30.0, // exactly at target
            soh: 100.0,
            capacity: 200.0, // 200 Ah * 48 V = 9.6 kWh
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            power_consumption: 1500.0,
            ..base_input()
        };
        let h = BalanceHypothetical {
            battery_soc: 30.0,
            mppt_power_total_w: 0.0,
            now: now_at(2026, 1, 15, 22, 0),
        };
        let b = compute_battery_balance(&input, &hw(), h);
        // current_capacity − current_target = 0 − 3000 = -3000 < baseload
        // pessimism → preserve_battery fires.
        assert_eq!(b.branch, BatteryBalanceBranch::Idle);
        // Drain bounded by the baseline-vs-solar idle formula. With
        // solar=0, baseline=1500 (default hw) → -1500 W. Critically:
        // NOT deeper than -baseline (no battery_export added on top).
        assert!(
            b.net_battery_w >= -1501.0,
            "drain must not exceed baseline at target SoC, got {}",
            b.net_battery_w
        );
    }

    /// Sharp drift-guard: live `evaluate_setpoint` and helper
    /// `compute_battery_balance` must agree on the modelled battery
    /// flow during the evening branch. We derive the expected
    /// `net_battery_w` from the live controller's debug fields
    /// (`exportable_capacity` / `hours_remaining`) and assert the
    /// helper produces the same number to within 1 W.
    ///
    /// If a future setpoint refactor breaks this equivalence the chart
    /// projection will silently drift from the live controller's
    /// behaviour — this test is the canary.
    #[test]
    fn drift_guard_evening_branch() {
        // High SoC, big battery, modest baseline → ensures the active
        // discharge sub-branch fires (preserve_battery=false). The PR
        // suggested 14 kWh at SoC=70 with 1.2 kW baseline, but with the
        // controller's pessimism multiplier (~1.4 over 4 h) the
        // baseload_to_be_consumed (~6.7 kWh) dominates the headroom and
        // preserve_battery wins. Bump SoC and trim baseline so the
        // active branch is exercised here; the unit-tests above already
        // cover the preserve_battery path.
        let mut hw = HardwareParams::defaults();
        hw.battery_nominal_voltage_v = 70.0;
        hw.baseload_consumption_w = 600.0;

        let input = SetpointInput {
            globals: SetpointInputGlobals {
                export_soc_threshold: 50.0,
                discharge_soc_target: 30.0,
                discharge_time: DischargeTime::At2300,
                pessimism_multiplier_modifier: 1.0,
                ..base_input().globals
            },
            battery_soc: 90.0,
            soh: 100.0,
            capacity: 200.0,
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            evcharger_ac_power: 0.0,
            power_consumption: 600.0, // = baseline
            ..base_input()
        };

        // 18:00 — evening branch with At2300, hours_remaining ≈ 4 h.
        let c = clock_at(2026, 1, 15, 18, 0);
        let live = evaluate_setpoint(&input, &c, &hw);

        // Live must have entered the evening branch — hours_remaining
        // > 0, exportable_capacity computed.
        assert!(
            live.debug.hours_remaining > 0.0,
            "live controller must have entered evening branch; hours_remaining={}",
            live.debug.hours_remaining
        );

        let now = c.naive();
        let bal = compute_battery_balance(
            &input,
            &hw,
            BalanceHypothetical {
                battery_soc: 90.0,
                mppt_power_total_w: 0.0,
                now,
            },
        );

        // Expected: -exportable_capacity / hours_remaining (the
        // discharge rate the live controller plans for). The helper
        // returns this directly when EveningDischarge fires.
        let expected_net_w =
            -live.debug.exportable_capacity / live.debug.hours_remaining;

        // Confirm the live controller actually planned a real export
        // (not a preserve_battery clamp). If preserve_battery fires
        // the helper returns Idle — that's a different branch and
        // the equivalence we want to pin doesn't apply.
        assert!(
            !live.debug.preserve_battery,
            "this fixture must exercise the active-discharge sub-branch; \
             preserve_battery={}",
            live.debug.preserve_battery
        );
        assert_eq!(
            bal.branch,
            BatteryBalanceBranch::EveningDischarge,
            "expected EveningDischarge, got {:?}",
            bal.branch
        );
        assert!(
            (bal.net_battery_w - expected_net_w).abs() < 1.0,
            "drift: helper net_battery_w={} vs live-derived={}",
            bal.net_battery_w,
            expected_net_w
        );
    }

    // ------------------------------------------------------------------
    // PR-ZDP-1: MPPT curtailment probe tests
    // ------------------------------------------------------------------

    // ZDP-T1: mppt_curtailed=true, probe_w=500, drain<threshold,
    // solar_export=2000, prev=-2000.
    // probe_offset=500, target=-(2000+500).max(-5000)=-2500.
    // prev=-2000 > target=-2500 → step DOWN: (-2000-100).max(-2500) = -2100.
    #[test]
    fn probe_fires_when_either_mppt_curtailed() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                zappi_drain_mppt_probe_w: 500,
                mppt_curtailed: true,
                grid_export_limit_w: 5000,
                ..base_input().globals
            },
            battery_dc_power: -500.0, // drain < threshold
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -2000,
            mppt_power_0: 2000.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // probe_offset=500, target=-2500
        // prev(-2000) > target(-2500) → step DOWN: (-2000-100).max(-2500) = -2100
        assert_eq!(out.setpoint_target, -2100);
        assert!(out.decision.summary.contains("relaxing"));
        // probe_offset factor must be present and non-zero
        let factor = out.decision.factors.iter()
            .find(|f| f.name == "probe_offset_W")
            .expect("probe_offset_W factor must be present in relax branch");
        assert_eq!(factor.value, "500", "probe_offset_W must equal probe_w when curtailed");
    }

    // ZDP-T2: mppt_curtailed=false → no probe, target=-solar_export=-2000.
    // prev=-2000 == target → step DOWN clamped at target: (-2000-100).max(-2000) = -2000.
    #[test]
    fn probe_does_not_fire_when_both_mppts_tracking() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                zappi_drain_mppt_probe_w: 500,
                mppt_curtailed: false, // not curtailed → no probe
                grid_export_limit_w: 5000,
                ..base_input().globals
            },
            battery_dc_power: -500.0,
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -2000,
            mppt_power_0: 2000.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // No probe: target=-2000, prev=-2000 → at target, stays at -2000.
        assert_eq!(out.setpoint_target, -2000);
        // probe_offset factor must be zero
        let factor = out.decision.factors.iter()
            .find(|f| f.name == "probe_offset_W")
            .expect("probe_offset_W factor must be present");
        assert_eq!(factor.value, "0", "probe_offset_W must be 0 when not curtailed");
    }

    // ZDP-T3: probe clamped by grid_export_limit.
    // solar_export=2000, probe=5000, grid_export_limit=4000
    // target = -(2000+5000).max(-4000) = -4000.
    // prev=-2000 > target=-4000 → step DOWN: (-2000-100).max(-4000) = -2100.
    #[test]
    fn probe_clamped_by_grid_export_limit() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                zappi_drain_mppt_probe_w: 5000,
                mppt_curtailed: true,
                grid_export_limit_w: 4000, // cap at 4000 W export
                ..base_input().globals
            },
            battery_dc_power: -500.0,
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -2000,
            mppt_power_0: 2000.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // target = max(-(2000+5000), -4000) = -4000
        // prev=-2000 > -4000 → step DOWN: (-2000-100).max(-4000) = -2100
        assert_eq!(out.setpoint_target, -2100);
        // Verify it is NOT stepping toward -7000 (unclamped would be -7000)
        // by checking the probe_offset factor equals 5000 but output is -2100.
        let factor = out.decision.factors.iter()
            .find(|f| f.name == "probe_offset_W")
            .expect("probe_offset_W factor must be present");
        assert_eq!(factor.value, "5000");
    }

    // ZDP-T4: tighten branch overrides probe.
    // mppt_curtailed=true AND drain>threshold → tighten branch fires.
    // drain=2500, threshold=1000, kp=1.0, prev=-2000
    // new = -2000 + 1.0*(2500-1000) = -500.
    #[test]
    fn tighten_branch_overrides_probe() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                zappi_drain_mppt_probe_w: 500,
                mppt_curtailed: true, // curtailed but drain > threshold → tighten wins
                grid_export_limit_w: 5000,
                ..base_input().globals
            },
            battery_dc_power: -2500.0, // drain=2500 > threshold=1000
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -2000,
            mppt_power_0: 0.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // Tighten: new = -2000 + 1.0*(2500-1000) = -500
        assert_eq!(out.setpoint_target, -500);
        assert!(out.decision.summary.contains("tightening"),
            "expected tightening but got: {}", out.decision.summary);
        // probe_offset_W must NOT appear in the tighten decision
        let probe_factor = out.decision.factors.iter()
            .find(|f| f.name == "probe_offset_W");
        assert!(probe_factor.is_none(), "probe_offset_W must not appear in tighten branch");
    }

    // ZDP-T5: probe knob=0 disables probing even when curtailed.
    // zappi_drain_mppt_probe_w=0, mppt_curtailed=true
    // probe_offset=0, target=-solar_export=-2000.
    // prev=-2000 == target → stays at -2000.
    #[test]
    fn probe_off_when_knob_zero() {
        let input = SetpointInput {
            globals: SetpointInputGlobals {
                zappi_active: true,
                allow_battery_to_car: false,
                zappi_drain_threshold_w: 1000,
                zappi_drain_relax_step_w: 100,
                zappi_drain_kp: 1.0,
                zappi_drain_mppt_probe_w: 0, // disabled
                mppt_curtailed: true,
                grid_export_limit_w: 5000,
                ..base_input().globals
            },
            battery_dc_power: -500.0,
            heat_pump_power: 0.0,
            cooker_power: 0.0,
            setpoint_target_prev: -2000,
            mppt_power_0: 2000.0,
            mppt_power_1: 0.0,
            soltaro_power: 0.0,
            ..base_input()
        };
        let c = clock_at(2026, 1, 15, 14, 30);
        let out = evaluate_setpoint(&input, &c, &hw());
        // No probe (knob=0): target=-2000, prev=-2000 → stays at -2000.
        assert_eq!(out.setpoint_target, -2000);
        let factor = out.decision.factors.iter()
            .find(|f| f.name == "probe_offset_W")
            .expect("probe_offset_W factor must be present");
        assert_eq!(factor.value, "0", "probe_offset_W must be 0 when knob is zero");
    }
}
