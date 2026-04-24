//! User-tunable knobs. See SPEC §7 for the safe-defaults baseline.
//!
//! On cold start the service actuates with [`Knobs::safe_defaults`]; retained
//! MQTT values then overwrite fields as they arrive (with the explicit
//! exception of `allow_battery_to_car`, which *always* boots `false`).

/// End-of-evening discharge target time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DischargeTime {
    /// Discharge until 02:00 (Night-Start continues through 02:00).
    #[default]
    At0200,
    /// Discharge until 23:00 (truncate early; used on tariffs with a 23:00
    /// peak-to-off-peak transition).
    At2300,
}

/// Manual override for the weekly Sunday-17:00 full-charge rollover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugFullCharge {
    /// Do not schedule a full charge, regardless of `next_full_charge`.
    Forbid,
    /// Force a full charge on the next evaluation cycle.
    Force,
    /// Follow the `next_full_charge` schedule (default).
    #[default]
    None,
}

/// Fusion strategy when forecast providers disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ForecastDisagreementStrategy {
    Max,
    Mean,
    Min,
    #[default]
    SolcastIfAvailableElseMean,
}

/// Override for the `charge_battery_extended` bit that the schedules
/// controller consults. Legacy derivation is
/// `!disable_night_grid_discharge || charge_to_full_required`, but the
/// user sometimes wants to pin it on or off regardless.
///
///   * `Auto` — use the legacy-derived value (default).
///   * `Forced` — always `true`, even when nothing else would set it.
///   * `Disabled` — always `false`, even when derivation says yes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChargeBatteryExtendedMode {
    #[default]
    Auto,
    Forced,
    Disabled,
}

/// User-controlled knobs. One struct, one source of truth.
///
/// Defaults come from [`Knobs::safe_defaults`]; see SPEC §7.
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Knobs {
    // --- Export / discharge policy ---
    pub force_disable_export: bool,
    /// Export when battery SoC (%) is at or above this threshold.
    pub export_soc_threshold: f64,
    /// Evening controller targets this SoC (%) at end-of-day.
    pub discharge_soc_target: f64,
    /// Night-time scheduled charge target (%).
    pub battery_soc_target: f64,
    /// Evening target (%) during weekly full-charge.
    pub full_charge_discharge_soc_target: f64,
    /// Export threshold (%) during weekly full-charge.
    pub full_charge_export_soc_threshold: f64,
    pub discharge_time: DischargeTime,
    pub debug_full_charge: DebugFullCharge,
    pub pessimism_multiplier_modifier: f64,
    pub disable_night_grid_discharge: bool,

    // --- Zappi ---
    pub charge_car_boost: bool,
    pub charge_car_extended: bool,
    pub zappi_current_target: f64,
    pub zappi_limit: f64,
    pub zappi_emergency_margin: f64,

    // --- New knobs (SPEC §2.10a) ---
    /// Hard cap on negative setpoint magnitude (grid-side export limit, W).
    pub grid_export_limit_w: u32,
    /// Hard cap on positive setpoint magnitude (grid-side import limit, W).
    pub grid_import_limit_w: u32,
    /// Optionally allow discharging DC battery into the EV during Zappi-active
    /// windows. Always boots `false` regardless of retained value.
    pub allow_battery_to_car: bool,
    /// Eddi target becomes Normal when battery SoC ≥ this (%).
    pub eddi_enable_soc: f64,
    /// Eddi target becomes Stopped when battery SoC ≤ this (%).
    pub eddi_disable_soc: f64,
    /// Minimum dwell time (s) at the current Eddi state before re-evaluation.
    pub eddi_dwell_s: u32,

    // --- Weather-SoC planner thresholds ---
    pub weathersoc_winter_temperature_threshold: f64,
    pub weathersoc_low_energy_threshold: f64,
    pub weathersoc_ok_energy_threshold: f64,
    pub weathersoc_high_energy_threshold: f64,
    pub weathersoc_too_much_energy_threshold: f64,

    // --- Ops ---
    pub writes_enabled: bool,
    pub forecast_disagreement_strategy: ForecastDisagreementStrategy,
    /// Manual override for the legacy `charge_battery_extended`
    /// derivation. Default `Auto` → use the derived value.
    pub charge_battery_extended_mode: ChargeBatteryExtendedMode,
}

impl Knobs {
    /// Cold-start safe defaults. Chosen per SPEC §7 to match the user's
    /// "keep battery around 80, schedule-only grid charging, cap grid export
    /// at 4900 W, don't discharge battery" policy.
    #[must_use]
    pub fn safe_defaults() -> Self {
        Self {
            force_disable_export: false,
            export_soc_threshold: 80.0,
            discharge_soc_target: 80.0,
            battery_soc_target: 80.0,
            full_charge_discharge_soc_target: 57.0,
            full_charge_export_soc_threshold: 100.0,
            discharge_time: DischargeTime::At0200,
            debug_full_charge: DebugFullCharge::None,
            pessimism_multiplier_modifier: 1.0,
            disable_night_grid_discharge: false,
            charge_car_boost: false,
            charge_car_extended: false,
            zappi_current_target: 9.5,
            zappi_limit: 100.0,
            zappi_emergency_margin: 5.0,
            grid_export_limit_w: 4900,
            grid_import_limit_w: 10,
            allow_battery_to_car: false,
            eddi_enable_soc: 96.0,
            eddi_disable_soc: 94.0,
            eddi_dwell_s: 60,
            weathersoc_winter_temperature_threshold: 12.0,
            weathersoc_low_energy_threshold: 12.0,
            weathersoc_ok_energy_threshold: 20.0,
            weathersoc_high_energy_threshold: 80.0,
            weathersoc_too_much_energy_threshold: 80.0,
            // Safe cold-start: no actuation effects until either a
            // retained MQTT `<root>/writes_enabled/state = true` seeds us
            // or a user flips the kill switch explicitly from the
            // dashboard. Bias-to-safety per SPEC §7; observer-mode is
            // the default, opt-in to act.
            writes_enabled: false,
            forecast_disagreement_strategy: ForecastDisagreementStrategy::SolcastIfAvailableElseMean,
            charge_battery_extended_mode: ChargeBatteryExtendedMode::Auto,
        }
    }
}

impl Default for Knobs {
    fn default() -> Self {
        Self::safe_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_defaults_match_spec_7() {
        let k = Knobs::safe_defaults();
        // Spot-check the non-trivial values from SPEC §7.
        assert!(!k.force_disable_export);
        assert!((k.export_soc_threshold - 80.0).abs() < f64::EPSILON);
        assert!((k.discharge_soc_target - 80.0).abs() < f64::EPSILON);
        assert!((k.battery_soc_target - 80.0).abs() < f64::EPSILON);
        assert!((k.full_charge_discharge_soc_target - 57.0).abs() < f64::EPSILON);
        assert!((k.full_charge_export_soc_threshold - 100.0).abs() < f64::EPSILON);
        assert_eq!(k.discharge_time, DischargeTime::At0200);
        assert_eq!(k.debug_full_charge, DebugFullCharge::None);
        assert_eq!(k.grid_export_limit_w, 4900);
        assert!(!k.allow_battery_to_car);
        assert!((k.eddi_enable_soc - 96.0).abs() < f64::EPSILON);
        assert!((k.eddi_disable_soc - 94.0).abs() < f64::EPSILON);
        assert_eq!(k.eddi_dwell_s, 60);
        // Cold-start safety: observer-mode by default; user/MQTT must
        // explicitly enable writes.
        assert!(!k.writes_enabled);
    }

    #[test]
    fn default_equals_safe_defaults() {
        assert_eq!(Knobs::default(), Knobs::safe_defaults());
    }
}
