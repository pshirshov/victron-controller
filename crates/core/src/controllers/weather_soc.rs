//! Weather-SoC planner. PR-WSOC-TABLE-1: replaced the legacy cascading
//! rung-by-rung ladder (`crates/core/src/controllers/weather_soc.rs` at
//! 944e3bd and earlier; itself a 1:1 port of the NR
//! `weather_soc_target` function in `legacy/debug/20260421-120100-functions.txt`
//! lines 381-476) with a 2D lookup-table model.
//!
//! Scheduled to run once a night at 01:55. Given today's temperature
//! forecast and PV-energy estimate, produces a bundle of knob decisions:
//! `export_soc_threshold`, `discharge_soc_target`, `battery_soc_target`,
//! `disable_night_grid_discharge`, and `charge_battery_extended`.
//!
//! # Model
//!
//! - 6 energy buckets along the energy axis:
//!   - **VerySunny**: `today_energy > weathersoc_very_sunny_threshold`
//!   - **Sunny**: `(too_much, very_sunny]`
//!   - **Mid**: `(high, too_much]`
//!   - **Low**: `(ok, high]`
//!   - **Dim**: `(low, ok]`
//!   - **VeryDim**: `<= low`
//! - 2 temperature columns: `warm` (`today_temp > winter_threshold`)
//!   and `cold` (`<=` — boundary at threshold counts as cold).
//! - 12 cells, each with `(export_soc_threshold, battery_soc_target,
//!   discharge_soc_target, extended)`. The fifth output —
//!   `disable_night_grid_discharge` — is **derived**: `dng = cell.extended`,
//!   computed *before* the stacked override applies. The override only
//!   mutates `bat` + `ext`, leaving `dng` intact (this preserves the
//!   prior cascade's rung-7 semantics where the full-charge-required
//!   override didn't touch the night-discharge bit).
//!
//! # Default cell table
//!
//! | Bucket    | Warm: exp / bat / dis / ext | Cold: exp / bat / dis / ext |
//! |-----------|-----------------------------|-----------------------------|
//! | VerySunny | 35 / 100 / 20 / no          | 80 / 100 / 30 / no          |
//! | Sunny     | 50 / 100 / 20 / no          | 80 / 100 / 30 / no          |
//! | Mid       | 67 / 100 / 20 / no          | 80 / 100 / 30 / no          |
//! | Low       | 100 / 100 / 30 / no         | 100 / 100 / 30 / yes        |
//! | Dim       | 100 / 100 / 30 / yes        | 100 / 100 / 30 / yes        |
//! | VeryDim   | 100 / 100 / 30 / yes        | 100 / 100 / 30 / yes        |
//!
//! These defaults are operator-tunable (PR-WSOC-EDIT-1: per-cell knobs).
//! PR-WSOC-TABLE-1 originally seeded the table to reproduce the legacy
//! cascade bit-for-bit; PR-WSOC-EDIT-1 normalises `bat=100` across the
//! three previously `bat=90` cells (Low.cold / Dim.warm / Dim.cold), so
//! the `extended` bit no longer implies a 90% cap. Cell-pinning tests
//! and three of the cascade-equivalence tests carry the updated
//! expectations; the rest are unchanged.
//!
//! # Stacked override (former rung 7 of the cascade)
//!
//! ```text
//! if g.charge_to_full_required && today_energy < g.high_energy_threshold_kwh {
//!     battery_soc_target = 100;
//!     extended = true;
//! }
//! ```
//!
//! The strict-`<` is preserved verbatim from the legacy source; the
//! override fires for Low buckets with energy strictly below `high`,
//! Dim, and VeryDim. It is intentionally NOT reformulated as
//! "bucket ∈ {Mid, Low}" — the kWh inequality is the contract.

use crate::Clock;
use crate::knobs::{WeatherSocCell, WeatherSocTable};
use crate::types::Decision;
// PR-WSOC-EDIT-1: cell-addressing vocabulary (EnergyBucket lived here
// pre-PR; moved to crates/core/src/weather_soc_addr.rs and re-exported
// from this module for backwards-compat with existing call sites).
pub use crate::weather_soc_addr::EnergyBucket;
use crate::weather_soc_addr::TempCol;

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

/// PR-WSOC-TABLE-1: classify `today_energy` into one of the six
/// buckets given the four kWh threshold knobs and the bucket-boundary
/// `weathersoc_very_sunny_threshold`. Boundary semantics match the
/// cascade verbatim: `<=` at the top of each band, `>` at the bottom.
#[must_use]
pub fn classify_energy(
    g: &WeatherSocInputGlobals,
    today_energy_kwh: f64,
    very_sunny_threshold: f64,
) -> EnergyBucket {
    if today_energy_kwh > very_sunny_threshold {
        EnergyBucket::VerySunny
    } else if today_energy_kwh > g.too_much_energy_threshold_kwh {
        EnergyBucket::Sunny
    } else if today_energy_kwh > g.high_energy_threshold_kwh {
        EnergyBucket::Mid
    } else if today_energy_kwh > g.ok_energy_threshold_kwh {
        EnergyBucket::Low
    } else if today_energy_kwh > g.low_energy_threshold_kwh {
        EnergyBucket::Dim
    } else {
        EnergyBucket::VeryDim
    }
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

/// PR-WSOC-TABLE-1: pick the 12-cell table cell for a given
/// `(bucket, cold)`.
fn pick_cell(table: &WeatherSocTable, bucket: EnergyBucket, cold: bool) -> WeatherSocCell {
    match (bucket, cold) {
        (EnergyBucket::VerySunny, false) => table.very_sunny_warm,
        (EnergyBucket::VerySunny, true) => table.very_sunny_cold,
        (EnergyBucket::Sunny, false) => table.sunny_warm,
        (EnergyBucket::Sunny, true) => table.sunny_cold,
        (EnergyBucket::Mid, false) => table.mid_warm,
        (EnergyBucket::Mid, true) => table.mid_cold,
        (EnergyBucket::Low, false) => table.low_warm,
        (EnergyBucket::Low, true) => table.low_cold,
        (EnergyBucket::Dim, false) => table.dim_warm,
        (EnergyBucket::Dim, true) => table.dim_cold,
        (EnergyBucket::VeryDim, false) => table.very_dim_warm,
        (EnergyBucket::VeryDim, true) => table.very_dim_cold,
    }
}

/// PR-WSOC-EDIT-1: mutable-borrow counterpart to `pick_cell`. Used by
/// `apply_knob`'s `KnobId::WeathersocTableCell` arm to route a single
/// `(bucket, temp, field)` write to the right cell field.
#[must_use]
pub fn cell_mut(
    table: &mut WeatherSocTable,
    bucket: EnergyBucket,
    temp: TempCol,
) -> &mut WeatherSocCell {
    match (bucket, temp) {
        (EnergyBucket::VerySunny, TempCol::Warm) => &mut table.very_sunny_warm,
        (EnergyBucket::VerySunny, TempCol::Cold) => &mut table.very_sunny_cold,
        (EnergyBucket::Sunny, TempCol::Warm) => &mut table.sunny_warm,
        (EnergyBucket::Sunny, TempCol::Cold) => &mut table.sunny_cold,
        (EnergyBucket::Mid, TempCol::Warm) => &mut table.mid_warm,
        (EnergyBucket::Mid, TempCol::Cold) => &mut table.mid_cold,
        (EnergyBucket::Low, TempCol::Warm) => &mut table.low_warm,
        (EnergyBucket::Low, TempCol::Cold) => &mut table.low_cold,
        (EnergyBucket::Dim, TempCol::Warm) => &mut table.dim_warm,
        (EnergyBucket::Dim, TempCol::Cold) => &mut table.dim_cold,
        (EnergyBucket::VeryDim, TempCol::Warm) => &mut table.very_dim_warm,
        (EnergyBucket::VeryDim, TempCol::Cold) => &mut table.very_dim_cold,
    }
}

/// PR-WSOC-TABLE-1: evaluate the weather-SoC planner via 6×2 table
/// lookup + single stacked override. `Clock` is taken for uniformity
/// (this controller doesn't actually use the wall clock — invocation
/// scheduling is the shell's job).
///
/// The override fires on the **legacy strict kWh inequality**:
/// `g.charge_to_full_required && today_energy < g.high_energy_threshold_kwh`
/// — preserved verbatim so cascade equivalence holds.
#[must_use]
pub fn evaluate_weather_soc(
    input: &WeatherSocInput,
    table: &WeatherSocTable,
    very_sunny_threshold: f64,
    _clock: &dyn Clock,
) -> WeatherSocDecision {
    let g = &input.globals;
    let today_temp = input.today_temperature_c;
    let today_energy = input.today_energy_kwh;

    let bucket = classify_energy(g, today_energy, very_sunny_threshold);
    // Boundary at threshold counts as cold (matches the legacy
    // `exact_temp_threshold_counts_as_cold` test).
    let cold = today_temp <= g.winter_temperature_threshold_c;
    let cell = pick_cell(table, bucket, cold);

    // Initial pre-override outputs come straight from the cell. The
    // contract: dng is derived *before* the override applies, so the
    // override-only-mutates-bat+ext semantic of the legacy rung 7
    // survives intact.
    let export_soc_threshold = cell.export_soc_threshold;
    let discharge_soc_target = cell.discharge_soc_target;
    let mut battery_soc_target = cell.battery_soc_target;
    let mut charge_battery_extended = cell.extended;
    // Read from cell.extended (NOT the mutable `charge_battery_extended`) so
    // the rung-7 override's mutation does NOT propagate into dng. Cascade
    // preserved this on Low.warm + charge_to_full_required: ext=true (override
    // fired), dng=false (preserve_morning_battery did not). Test
    // `override_low_warm_cf_true_only_mutates_bat_ext` pins this.
    let disable_night_grid_discharge = cell.extended;

    // Stacked override (former rung 7). Strict `<` mirrors the legacy
    // source and is the contract — do NOT translate to "bucket ∈ {Mid,
    // Low}" or any other reformulation.
    let override_fired = g.charge_to_full_required && today_energy < g.high_energy_threshold_kwh;
    if override_fired {
        battery_soc_target = 100.0;
        charge_battery_extended = true;
    }

    let summary = format!(
        "Bucket {} / {} cell{}",
        bucket.label(),
        if cold { "cold" } else { "warm" },
        if override_fired {
            "; charge-to-full override fired (bat→100, ext→true)"
        } else {
            ""
        },
    );
    let decision = Decision::new(summary)
        .with_factor("today_temp_C", format!("{today_temp:.1}"))
        .with_factor("today_energy_kWh", format!("{today_energy:.1}"))
        .with_factor("bucket", bucket.label().to_string())
        .with_factor("cold", format!("{cold}"))
        .with_factor("override_fired", format!("{override_fired}"))
        .with_factor("winter_threshold_C", format!("{:.1}", g.winter_temperature_threshold_c))
        .with_factor("low_threshold_kWh", format!("{:.0}", g.low_energy_threshold_kwh))
        .with_factor("ok_threshold_kWh", format!("{:.0}", g.ok_energy_threshold_kwh))
        .with_factor("high_threshold_kWh", format!("{:.0}", g.high_energy_threshold_kwh))
        .with_factor("too_much_threshold_kWh", format!("{:.0}", g.too_much_energy_threshold_kwh))
        .with_factor("very_sunny_threshold_kWh", format!("{very_sunny_threshold:.1}"))
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
    // The 11 tests at the top of this module are cascade-equivalence
    // goldens retained from the pre-2026-05-04 ladder implementation;
    // their docstrings reference closure names from the deleted cascade
    // (`disable_export`, `extend_charge`, `preserve_evening_battery`,
    // `charge_to_full_extended`, …) for archaeology. The lookup-table
    // model produces identical outputs by construction. Cell-pinning and
    // boundary tests for the new model live below them.
    use super::*;
    use crate::clock::FixedClock;
    use crate::knobs::Knobs;
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

    /// Cascade-equivalence test helper. The 11 retained tests below feed
    /// inputs whose bucket boundaries derive from `base_globals` (the
    /// legacy default set: low=12, ok=20, high=80, too_much=80) and
    /// expect the cells of `safe_defaults().weather_soc_table` (the new
    /// default set) to produce the cascade's outputs.
    ///
    /// **Load-bearing invariant**: `very_sunny_threshold` is derived
    /// here as `base_globals.too_much * 1.5` (= 120). DO NOT swap this
    /// for `k.weathersoc_very_sunny_threshold` (= 67.5) — the safe-defaults
    /// boundary disagrees with `base_globals` thresholds, and a uniform
    /// substitution would re-classify several test inputs into different
    /// buckets, breaking the cascade-equivalence asserts. The cells and
    /// the boundary travel together; the helper preserves the legacy
    /// `1.5×too_much` coupling that the cascade had baked in.
    fn evaluate(input: &WeatherSocInput) -> WeatherSocDecision {
        let k = Knobs::safe_defaults();
        let very_sunny = input.globals.too_much_energy_threshold_kwh * 1.5;
        evaluate_weather_soc(
            input,
            &k.weather_soc_table,
            very_sunny,
            &any_clock(),
        )
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
        let d = evaluate(&input);
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
        let d = evaluate(&input);
        assert!((d.export_soc_threshold - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn very_sunny_and_warm_day_exports_max() {
        let input = WeatherSocInput {
            globals: base_globals(),
            today_temperature_c: 25.0,
            today_energy_kwh: 150.0, // > 1.5 * 80 = 120
        };
        let d = evaluate(&input);
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
        let d = evaluate(&input);
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
        let d = evaluate(&input);
        // Operator preference 2026-05-04 (PR-WSOC-EDIT-1): Low.cold /
        // Dim cells charge to 100, not 90; the extended bit no longer
        // implies a 90 % cap.
        assert!((d.battery_soc_target - 100.0).abs() < f64::EPSILON);
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
        let d = evaluate(&input);
        // Operator preference 2026-05-04 (PR-WSOC-EDIT-1): Low.cold /
        // Dim cells charge to 100, not 90; the extended bit no longer
        // implies a 90 % cap.
        assert!((d.battery_soc_target - 100.0).abs() < f64::EPSILON);
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
        let d = evaluate(&input);
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
        let d = evaluate(&input);
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
        let d = evaluate(&input);
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
        let d = evaluate(&input);
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
        let d = evaluate(&input);
        // disable_export fires.
        assert!((d.export_soc_threshold - 100.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // PR-WSOC-TABLE-1: 12-cell pinning + override + boundary tests.
    //
    // Each of the 12 cell tests picks `(today_energy, today_temp)`
    // strictly inside the bucket and asserts the
    // `(exp, bat, dis, ext, dng)` tuple of the new evaluator. The
    // expected values mirror the M-WSOC-TABLE plan §5 test matrix
    // and the `safe_defaults` table verbatim.
    //
    // `safe_defaults` thresholds at the time of writing:
    //   low=8, ok=15, high=30, too_much=45, very_sunny=67.5, winter=12.0.
    // The base_globals() helper above uses *legacy* thresholds
    // (low=12, ok=20, high=80, too_much=80) for the 11 cascade-equivalent
    // tests; the cell-pinning tests below use safe-defaults thresholds
    // so the energy values land in the buckets the matrix assumes.
    // ------------------------------------------------------------------

    /// Globals that match `Knobs::safe_defaults` thresholds. Used by
    /// the 12 cell tests and the override / boundary tests below.
    fn defaults_globals() -> WeatherSocInputGlobals {
        let k = Knobs::safe_defaults();
        WeatherSocInputGlobals {
            charge_to_full_required: false,
            winter_temperature_threshold_c: k.weathersoc_winter_temperature_threshold,
            low_energy_threshold_kwh: k.weathersoc_low_energy_threshold,
            ok_energy_threshold_kwh: k.weathersoc_ok_energy_threshold,
            high_energy_threshold_kwh: k.weathersoc_high_energy_threshold,
            too_much_energy_threshold_kwh: k.weathersoc_too_much_energy_threshold,
        }
    }

    /// Compact tuple-matcher for cell tests.
    #[track_caller]
    fn assert_outputs(
        d: &WeatherSocDecision,
        exp: f64,
        bat: f64,
        dis: f64,
        ext: bool,
        dng: bool,
    ) {
        assert!((d.export_soc_threshold - exp).abs() < f64::EPSILON, "exp");
        assert!((d.battery_soc_target - bat).abs() < f64::EPSILON, "bat");
        assert!((d.discharge_soc_target - dis).abs() < f64::EPSILON, "dis");
        assert_eq!(d.charge_battery_extended, ext, "ext");
        assert_eq!(d.disable_night_grid_discharge, dng, "dng");
    }

    fn input_at(energy: f64, temp: f64, cf: bool) -> WeatherSocInput {
        let mut g = defaults_globals();
        g.charge_to_full_required = cf;
        WeatherSocInput {
            globals: g,
            today_temperature_c: temp,
            today_energy_kwh: energy,
        }
    }

    #[test]
    fn cell_very_sunny_warm() {
        // 100 > 67.5 → VerySunny; 25 > 12 → warm.
        let d = evaluate(&input_at(100.0, 25.0, false));
        assert_outputs(&d, 35.0, 100.0, 20.0, false, false);
    }

    #[test]
    fn cell_very_sunny_cold() {
        // 100 > 67.5 → VerySunny; 5 ≤ 12 → cold.
        let d = evaluate(&input_at(100.0, 5.0, false));
        assert_outputs(&d, 80.0, 100.0, 30.0, false, false);
    }

    #[test]
    fn cell_sunny_warm() {
        // 50 ∈ (45, 67.5] → Sunny; 25 → warm.
        let d = evaluate(&input_at(50.0, 25.0, false));
        assert_outputs(&d, 50.0, 100.0, 20.0, false, false);
    }

    #[test]
    fn cell_sunny_cold() {
        // 50 ∈ (45, 67.5] → Sunny; 5 → cold.
        let d = evaluate(&input_at(50.0, 5.0, false));
        assert_outputs(&d, 80.0, 100.0, 30.0, false, false);
    }

    #[test]
    fn cell_mid_warm() {
        // 40 ∈ (30, 45] → Mid; 25 → warm.
        let d = evaluate(&input_at(40.0, 25.0, false));
        assert_outputs(&d, 67.0, 100.0, 20.0, false, false);
    }

    #[test]
    fn cell_mid_cold() {
        // 40 ∈ (30, 45] → Mid; 5 → cold.
        let d = evaluate(&input_at(40.0, 5.0, false));
        assert_outputs(&d, 80.0, 100.0, 30.0, false, false);
    }

    #[test]
    fn cell_low_warm() {
        // 25 ∈ (15, 30] → Low; 25 → warm.
        let d = evaluate(&input_at(25.0, 25.0, false));
        assert_outputs(&d, 100.0, 100.0, 30.0, false, false);
    }

    #[test]
    fn cell_low_cold() {
        // 25 ∈ (15, 30] → Low; 5 → cold.
        // PR-WSOC-EDIT-1: bat=100 (was 90).
        let d = evaluate(&input_at(25.0, 5.0, false));
        assert_outputs(&d, 100.0, 100.0, 30.0, true, true);
    }

    #[test]
    fn cell_dim_warm() {
        // 12 ∈ (8, 15] → Dim; 25 → warm.
        // PR-WSOC-EDIT-1: bat=100 (was 90).
        let d = evaluate(&input_at(12.0, 25.0, false));
        assert_outputs(&d, 100.0, 100.0, 30.0, true, true);
    }

    #[test]
    fn cell_dim_cold() {
        // 12 ∈ (8, 15] → Dim; 5 → cold.
        // PR-WSOC-EDIT-1: bat=100 (was 90).
        let d = evaluate(&input_at(12.0, 5.0, false));
        assert_outputs(&d, 100.0, 100.0, 30.0, true, true);
    }

    #[test]
    fn cell_very_dim_warm() {
        // 5 ≤ 8 → VeryDim; 25 → warm.
        let d = evaluate(&input_at(5.0, 25.0, false));
        assert_outputs(&d, 100.0, 100.0, 30.0, true, true);
    }

    #[test]
    fn cell_very_dim_cold() {
        // 5 ≤ 8 → VeryDim; 5 → cold.
        let d = evaluate(&input_at(5.0, 5.0, false));
        assert_outputs(&d, 100.0, 100.0, 30.0, true, true);
    }

    // --- Override + boundary tests ----------------------------------------

    /// Rung-7 override on Low warm + cf=true. The override mutates
    /// `bat` + `ext` only; `dng` remains derived from `cell.extended`
    /// (false for low-warm), so dng stays false.
    #[test]
    fn override_low_warm_cf_true_only_mutates_bat_ext() {
        let d = evaluate(&input_at(25.0, 25.0, true));
        // exp from the cell unchanged; dis from the cell unchanged;
        // bat → 100 (was 100 anyway); ext → true (was false); dng → false.
        assert_outputs(&d, 100.0, 100.0, 30.0, true, false);
    }

    /// Rung-7 override on Dim warm + cf=true. The cell already has
    /// `extended=true` so `dng=true` was derived pre-override; the
    /// override raises bat from 90 to 100 and leaves dng intact.
    #[test]
    fn override_dim_warm_cf_true_extended_already_true() {
        let d = evaluate(&input_at(12.0, 25.0, true));
        assert_outputs(&d, 100.0, 100.0, 30.0, true, true);
    }

    /// Rung-7 override is gated on `today_energy < high` (strict). On
    /// VerySunny (100 kWh > 30 kWh high), the guard is false and the
    /// override doesn't fire — the cell wins entirely.
    #[test]
    fn override_skipped_on_very_sunny_cf_true() {
        let d = evaluate(&input_at(100.0, 25.0, true));
        assert_outputs(&d, 35.0, 100.0, 20.0, false, false);
    }

    /// Boundary: `today_temp == winter_threshold` counts as cold.
    /// Sunny + temp=12 → Sunny cold cell.
    #[test]
    fn boundary_temp_equals_threshold_counts_as_cold() {
        let d = evaluate(&input_at(50.0, 12.0, false));
        // Sunny cold: exp=80, bat=100, dis=30, ext=false, dng=false.
        assert_outputs(&d, 80.0, 100.0, 30.0, false, false);
    }

    /// Boundary: `today_energy == high` lands in Low (closed at top of
    /// each band). Low warm cell.
    #[test]
    fn boundary_energy_equals_high_lands_in_low() {
        let d = evaluate(&input_at(30.0, 25.0, false));
        assert_outputs(&d, 100.0, 100.0, 30.0, false, false);
    }

    /// Boundary: `today_energy == too_much` lands in Mid. Mid warm cell.
    #[test]
    fn boundary_energy_equals_too_much_lands_in_mid() {
        let d = evaluate(&input_at(45.0, 25.0, false));
        assert_outputs(&d, 67.0, 100.0, 20.0, false, false);
    }

    /// Boundary: `today_energy == very_sunny_threshold` lands in
    /// Sunny (closed at top). Sunny warm cell.
    #[test]
    fn boundary_energy_equals_very_sunny_threshold_lands_in_sunny() {
        let d = evaluate(&input_at(67.5, 25.0, false));
        assert_outputs(&d, 50.0, 100.0, 20.0, false, false);
    }
}
