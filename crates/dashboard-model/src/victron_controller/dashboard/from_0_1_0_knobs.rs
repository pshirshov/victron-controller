// PR-gamma-hold-redesign: 0.1.0 → 0.2.0 manual converter for `Knobs`.
// 0.2.0 adds four `*_mode: Mode` selectors. Default to `Mode::Weather`
// so the back-compat path preserves prior implicit behaviour: the four
// weather_soc-driven knobs continue to be driven by the planner unless
// the user explicitly flips the matching mode to `Forced`.

pub fn convert__knobs__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::knobs::Knobs) -> crate::victron_controller::dashboard::knobs::Knobs {
    use crate::victron_controller::dashboard::mode::Mode;
    crate::victron_controller::dashboard::knobs::Knobs {
        force_disable_export: from.force_disable_export,
        export_soc_threshold: from.export_soc_threshold,
        discharge_soc_target: from.discharge_soc_target,
        battery_soc_target: from.battery_soc_target,
        full_charge_discharge_soc_target: from.full_charge_discharge_soc_target,
        full_charge_export_soc_threshold: from.full_charge_export_soc_threshold,
        discharge_time: serde_json::from_value(serde_json::to_value(&from.discharge_time).unwrap()).unwrap(),
        debug_full_charge: serde_json::from_value(serde_json::to_value(&from.debug_full_charge).unwrap()).unwrap(),
        pessimism_multiplier_modifier: from.pessimism_multiplier_modifier,
        disable_night_grid_discharge: from.disable_night_grid_discharge,
        charge_car_boost: from.charge_car_boost,
        charge_car_extended: from.charge_car_extended,
        zappi_current_target: from.zappi_current_target,
        zappi_limit: from.zappi_limit,
        zappi_emergency_margin: from.zappi_emergency_margin,
        grid_export_limit_w: from.grid_export_limit_w,
        grid_import_limit_w: from.grid_import_limit_w,
        allow_battery_to_car: from.allow_battery_to_car,
        eddi_enable_soc: from.eddi_enable_soc,
        eddi_disable_soc: from.eddi_disable_soc,
        eddi_dwell_s: from.eddi_dwell_s,
        weathersoc_winter_temperature_threshold: from.weathersoc_winter_temperature_threshold,
        weathersoc_low_energy_threshold: from.weathersoc_low_energy_threshold,
        weathersoc_ok_energy_threshold: from.weathersoc_ok_energy_threshold,
        weathersoc_high_energy_threshold: from.weathersoc_high_energy_threshold,
        weathersoc_too_much_energy_threshold: from.weathersoc_too_much_energy_threshold,
        writes_enabled: from.writes_enabled,
        forecast_disagreement_strategy: serde_json::from_value(serde_json::to_value(&from.forecast_disagreement_strategy).unwrap()).unwrap(),
        charge_battery_extended_mode: serde_json::from_value(serde_json::to_value(&from.charge_battery_extended_mode).unwrap()).unwrap(),
        // PR-gamma-hold-redesign — initialise the four mode selectors to
        // `Weather` so the back-compat path preserves prior implicit
        // behaviour (planner drives these knobs).
        export_soc_threshold_mode: Mode::Weather,
        discharge_soc_target_mode: Mode::Weather,
        battery_soc_target_mode: Mode::Weather,
        disable_night_grid_discharge_mode: Mode::Weather,
    }
}
