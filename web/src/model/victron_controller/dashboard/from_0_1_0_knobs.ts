// @ts-nocheck
import {Knobs as v0_1_0_Knobs} from './v0_1_0/Knobs'
import {Knobs as dashboard_Knobs} from './Knobs'

export function convert__knobs__from__0_1_0(from: v0_1_0_Knobs): dashboard_Knobs {
    return new dashboard_Knobs (
        from.force_disable_export,
        from.export_soc_threshold,
        from.discharge_soc_target,
        from.battery_soc_target,
        from.full_charge_discharge_soc_target,
        from.full_charge_export_soc_threshold,
        JSON.parse(JSON.stringify(from.discharge_time)),
        JSON.parse(JSON.stringify(from.debug_full_charge)),
        from.pessimism_multiplier_modifier,
        from.disable_night_grid_discharge,
        from.charge_car_boost,
        from.charge_car_extended,
        from.zappi_current_target,
        from.zappi_limit,
        from.zappi_emergency_margin,
        from.grid_export_limit_w,
        from.grid_import_limit_w,
        from.allow_battery_to_car,
        from.eddi_enable_soc,
        from.eddi_disable_soc,
        from.eddi_dwell_s,
        from.weathersoc_winter_temperature_threshold,
        from.weathersoc_low_energy_threshold,
        from.weathersoc_ok_energy_threshold,
        from.weathersoc_high_energy_threshold,
        from.weathersoc_too_much_energy_threshold,
        from.writes_enabled,
        JSON.parse(JSON.stringify(from.forecast_disagreement_strategy)),
        JSON.parse(JSON.stringify(from.charge_battery_extended_mode))
    )
}