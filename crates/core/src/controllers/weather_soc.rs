//! Weather-SoC planner. 1:1 port of the legacy NR 'weather_soc_target'
//! function (see `legacy/debug/20260421-120100-functions.txt`
//! lines 381-476).
//!
//! Scheduled to run once a night at 01:55. Given today's temperature
//! forecast and PV-energy estimate, produces a bundle of knob decisions:
//! `export_soc_threshold`, `discharge_soc_target`, `battery_soc_target`,
//! `disable_night_grid_discharge`, and `charge_battery_extended`.
//!
//! Cascading ladder (order matters — last-write-wins across the
//! independent `if`s):
//!
//! - Very sunny day → raise export aggression.
//! - Very sunny + summer → max export.
//! - Cold day → preserve evening battery.
//! - Below "high" kWh → disable export; if also cold, extend charge +
//!   preserve morning battery.
//! - Below "ok" kWh → extend charge + preserve morning.
//! - Below "low" kWh → charge to full + extended.
//! - Weekly full-charge required → force charge-to-full-extended unless
//!   today is already predicted sunny.

use crate::Clock;
use crate::types::Decision;

/// Inputs — two aggregates from the forecast stack plus thresholds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherSocInput {
    pub globals: WeatherSocInputGlobals,
    pub today_temperature_c: f64,
    /// Today's PV-energy estimate in kWh (same scale as the `weathersoc_*`
    /// kWh thresholds). Produced by the forecast fusion layer.
    pub today_energy_kwh: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeatherSocInputGlobals {
    pub charge_to_full_required: bool,
    pub winter_temperature_threshold_c: f64,
    pub low_energy_threshold_kwh: f64,
    pub ok_energy_threshold_kwh: f64,
    pub high_energy_threshold_kwh: f64,
    pub too_much_energy_threshold_kwh: f64,
}

/// Output: proposed values for five knobs + the human-readable
/// reasoning chain for how they were arrived at.
#[derive(Debug, Clone, PartialEq)]
pub struct WeatherSocDecision {
    pub export_soc_threshold: f64,
    pub discharge_soc_target: f64,
    pub battery_soc_target: f64,
    pub disable_night_grid_discharge: bool,
    pub charge_battery_extended: bool,
    pub decision: Decision,
}

/// Evaluate the weather-SoC planner's knob proposals for the given
/// forecast inputs. `Clock` is taken for uniformity (even though this
/// controller doesn't actually need the current time — invocation
/// scheduling is the shell's job).
#[must_use]
pub fn evaluate_weather_soc(
    input: &WeatherSocInput,
    _clock: &dyn Clock,
) -> WeatherSocDecision {
    let g = &input.globals;
    let today_temp = input.today_temperature_c;
    let today_energy = input.today_energy_kwh;

    // Initial defaults — match the TS version exactly. Strings "67"/"20"/...
    // become numeric values here.
    let mut export_soc_threshold: f64 = 67.0;
    let mut discharge_soc_target: f64 = 20.0;
    let mut battery_soc_target: f64 = 100.0;
    let mut charge_battery_extended: bool = false;
    let mut disable_night_grid_discharge: bool = false;

    // --- Named actions (mirror the TS inner functions) ---
    let export_more = |threshold: &mut f64| *threshold = 50.0;
    let export_max = |threshold: &mut f64| *threshold = 35.0;

    let preserve_evening_battery = |threshold: &mut f64, dsoc: &mut f64| {
        if (*threshold - 100.0).abs() >= f64::EPSILON {
            *threshold = 80.0;
        }
        *dsoc = 30.0;
    };

    let disable_export = |threshold: &mut f64, dsoc: &mut f64| {
        *threshold = 100.0;
        // preserve_evening_battery:
        if (*threshold - 100.0).abs() >= f64::EPSILON {
            *threshold = 80.0;
        }
        *dsoc = 30.0;
    };

    let extend_charge = |ctarget: &mut f64, ext: &mut bool| {
        *ctarget = 90.0;
        *ext = true;
    };

    let charge_to_full_extended = |ctarget: &mut f64, ext: &mut bool| {
        *ctarget = 100.0;
        *ext = true;
    };

    let preserve_morning_battery = |dng: &mut bool| *dng = true;

    let way_too_much = g.too_much_energy_threshold_kwh * 1.5;

    // --- Cascading decision ladder — also narrate which rungs fired ---
    let mut rungs: Vec<&'static str> = Vec::new();
    if today_energy > g.too_much_energy_threshold_kwh {
        export_more(&mut export_soc_threshold);
        rungs.push("today_energy > too_much → export_more");
    }
    if today_temp > g.winter_temperature_threshold_c && today_energy > way_too_much {
        export_max(&mut export_soc_threshold);
        rungs.push("warm + way-too-much → export_max");
    }
    if today_temp <= g.winter_temperature_threshold_c {
        preserve_evening_battery(&mut export_soc_threshold, &mut discharge_soc_target);
        rungs.push("cold → preserve_evening_battery");
    }
    if today_energy <= g.high_energy_threshold_kwh {
        disable_export(&mut export_soc_threshold, &mut discharge_soc_target);
        rungs.push("today_energy ≤ high → disable_export");
        if today_temp <= g.winter_temperature_threshold_c {
            extend_charge(&mut battery_soc_target, &mut charge_battery_extended);
            preserve_morning_battery(&mut disable_night_grid_discharge);
            rungs.push("cold + ≤ high → extend_charge + preserve_morning_battery");
        }
    }
    if today_energy <= g.ok_energy_threshold_kwh {
        extend_charge(&mut battery_soc_target, &mut charge_battery_extended);
        preserve_morning_battery(&mut disable_night_grid_discharge);
        rungs.push("today_energy ≤ ok → extend_charge + preserve_morning_battery");
    }
    if today_energy <= g.low_energy_threshold_kwh {
        charge_to_full_extended(&mut battery_soc_target, &mut charge_battery_extended);
        rungs.push("today_energy ≤ low → charge_to_full_extended");
    }
    if g.charge_to_full_required && today_energy < g.high_energy_threshold_kwh {
        charge_to_full_extended(&mut battery_soc_target, &mut charge_battery_extended);
        rungs.push("charge_to_full_required + not-sunny → charge_to_full_extended");
    }

    let summary = if rungs.is_empty() {
        "Mild/moderate day — defaults apply".to_string()
    } else {
        format!("Rungs fired: {}", rungs.join("; "))
    };
    let decision = Decision::new(summary)
        .with_factor("today_temp_C", format!("{today_temp:.1}"))
        .with_factor("today_energy_kWh", format!("{today_energy:.1}"))
        .with_factor("winter_threshold_C", format!("{:.1}", g.winter_temperature_threshold_c))
        .with_factor("low_threshold_kWh", format!("{:.0}", g.low_energy_threshold_kwh))
        .with_factor("ok_threshold_kWh", format!("{:.0}", g.ok_energy_threshold_kwh))
        .with_factor("high_threshold_kWh", format!("{:.0}", g.high_energy_threshold_kwh))
        .with_factor("too_much_threshold_kWh", format!("{:.0}", g.too_much_energy_threshold_kwh))
        .with_factor("charge_to_full_required", format!("{}", g.charge_to_full_required))
        .with_factor("→ export_soc_threshold", format!("{export_soc_threshold:.0}%"))
        .with_factor("→ discharge_soc_target", format!("{discharge_soc_target:.0}%"))
        .with_factor("→ battery_soc_target", format!("{battery_soc_target:.0}%"))
        .with_factor("→ charge_battery_extended", format!("{charge_battery_extended}"))
        .with_factor("→ disable_night_grid_discharge", format!("{disable_night_grid_discharge}"));

    WeatherSocDecision {
        export_soc_threshold,
        discharge_soc_target,
        battery_soc_target,
        disable_night_grid_discharge,
        charge_battery_extended,
        decision,
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::FixedClock;
    use chrono::NaiveDate;

    fn any_clock() -> FixedClock {
        FixedClock::at(
            NaiveDate::from_ymd_opt(2026, 4, 21)
                .unwrap()
                .and_hms_opt(1, 55, 0)
                .unwrap(),
        )
    }

    fn base_globals() -> WeatherSocInputGlobals {
        WeatherSocInputGlobals {
            charge_to_full_required: false,
            winter_temperature_threshold_c: 12.0,
            low_energy_threshold_kwh: 12.0,
            ok_energy_threshold_kwh: 20.0,
            high_energy_threshold_kwh: 80.0,
            too_much_energy_threshold_kwh: 80.0,
        }
    }

    // ------------------------------------------------------------------
    // Defaults path: mild-ish day with moderate energy
    // ------------------------------------------------------------------

    #[test]
    fn mild_day_moderate_energy_uses_defaults() {
        // temp > winter, 20 < energy ≤ 80 — only the "≤ high" branch fires,
        // which calls disable_export.
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 20.0,
            today_energy_kwh: 50.0, // ≤ 80 (high)
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        assert!((d.export_soc_threshold - 100.0).abs() < f64::EPSILON);
        assert!((d.discharge_soc_target - 30.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // Sunny day branches
    // ------------------------------------------------------------------

    #[test]
    fn very_sunny_day_exports_more() {
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 20.0,
            today_energy_kwh: 100.0, // > 80 = too_much_energy_threshold
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        assert!((d.export_soc_threshold - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn very_sunny_and_warm_day_exports_max() {
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 25.0,
            today_energy_kwh: 150.0, // > 1.5 * 80 = 120
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        assert!((d.export_soc_threshold - 35.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // Cold day branches
    // ------------------------------------------------------------------

    #[test]
    fn cold_day_moderate_energy_preserves_evening() {
        // temp ≤ winter, 20 < energy ≤ 80
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 5.0,
            today_energy_kwh: 50.0,
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        // preserve_evening + disable_export → export=100, discharge=30
        assert!((d.export_soc_threshold - 100.0).abs() < f64::EPSILON);
        assert!((d.discharge_soc_target - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cold_and_low_energy_extends_charge_and_preserves_morning() {
        // temp ≤ winter, energy ≤ high
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 5.0,
            today_energy_kwh: 30.0, // ≤ 80 high; > 20 ok
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        assert!((d.battery_soc_target - 90.0).abs() < f64::EPSILON);
        assert!(d.charge_battery_extended);
        assert!(d.disable_night_grid_discharge);
    }

    // ------------------------------------------------------------------
    // Energy-threshold ladders
    // ------------------------------------------------------------------

    #[test]
    fn ok_or_below_always_extends_charge_regardless_of_temp() {
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 25.0, // warm
            today_energy_kwh: 15.0,    // ≤ 20 ok
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        assert!((d.battery_soc_target - 90.0).abs() < f64::EPSILON);
        assert!(d.charge_battery_extended);
        assert!(d.disable_night_grid_discharge);
    }

    #[test]
    fn very_low_energy_forces_charge_to_full() {
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 25.0,
            today_energy_kwh: 5.0, // ≤ 12 low
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        assert!((d.battery_soc_target - 100.0).abs() < f64::EPSILON);
        assert!(d.charge_battery_extended);
    }

    // ------------------------------------------------------------------
    // Full-charge override
    // ------------------------------------------------------------------

    #[test]
    fn charge_to_full_required_with_low_energy_forces_full_charge() {
        let mut g = base_globals();
        g.charge_to_full_required = true;
        let input = WeatherSocInput {
            globals: g,
            today_temperature_c: 25.0,
            today_energy_kwh: 50.0, // < high(80), so the final guard fires
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        assert!((d.battery_soc_target - 100.0).abs() < f64::EPSILON);
        assert!(d.charge_battery_extended);
    }

    #[test]
    fn charge_to_full_required_with_high_energy_skips_forcing() {
        let mut g = base_globals();
        g.charge_to_full_required = true;
        let input = WeatherSocInput {
            globals: g,
            today_temperature_c: 25.0,
            today_energy_kwh: 100.0, // > 80 high → skip the full-charge guard
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        // The very-sunny branch fired (export_more); no extension forced.
        assert!((d.export_soc_threshold - 50.0).abs() < f64::EPSILON);
        assert!(!d.charge_battery_extended);
    }

    // ------------------------------------------------------------------
    // Boundary conditions
    // ------------------------------------------------------------------

    #[test]
    fn exact_temp_threshold_counts_as_cold() {
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 12.0, // == threshold
            today_energy_kwh: 50.0,
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        // `today_temp <= winter_temperature_threshold` is true → preserve
        assert!((d.discharge_soc_target - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn exact_energy_threshold_counts_as_below() {
        // `today_energy <= high_energy_threshold` true at equality.
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 25.0,
            today_energy_kwh: 80.0, // == high
        };
        let d = evaluate_weather_soc(&input, &any_clock());
        // disable_export fires.
        assert!((d.export_soc_threshold - 100.0).abs() < f64::EPSILON);
    }
}
