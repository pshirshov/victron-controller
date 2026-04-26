

pub fn convert__sensors__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::sensors::Sensors) -> crate::victron_controller::dashboard::sensors::Sensors {
    crate::victron_controller::dashboard::sensors::Sensors {
        battery_soc: serde_json::from_value(serde_json::to_value(&from.battery_soc).unwrap()).unwrap(),
        battery_soh: serde_json::from_value(serde_json::to_value(&from.battery_soh).unwrap()).unwrap(),
        battery_installed_capacity: serde_json::from_value(serde_json::to_value(&from.battery_installed_capacity).unwrap()).unwrap(),
        battery_dc_power: serde_json::from_value(serde_json::to_value(&from.battery_dc_power).unwrap()).unwrap(),
        mppt_power_0: serde_json::from_value(serde_json::to_value(&from.mppt_power_0).unwrap()).unwrap(),
        mppt_power_1: serde_json::from_value(serde_json::to_value(&from.mppt_power_1).unwrap()).unwrap(),
        soltaro_power: serde_json::from_value(serde_json::to_value(&from.soltaro_power).unwrap()).unwrap(),
        power_consumption: serde_json::from_value(serde_json::to_value(&from.power_consumption).unwrap()).unwrap(),
        grid_power: serde_json::from_value(serde_json::to_value(&from.grid_power).unwrap()).unwrap(),
        grid_voltage: serde_json::from_value(serde_json::to_value(&from.grid_voltage).unwrap()).unwrap(),
        grid_current: serde_json::from_value(serde_json::to_value(&from.grid_current).unwrap()).unwrap(),
        consumption_current: serde_json::from_value(serde_json::to_value(&from.consumption_current).unwrap()).unwrap(),
        offgrid_power: serde_json::from_value(serde_json::to_value(&from.offgrid_power).unwrap()).unwrap(),
        offgrid_current: serde_json::from_value(serde_json::to_value(&from.offgrid_current).unwrap()).unwrap(),
        vebus_input_current: serde_json::from_value(serde_json::to_value(&from.vebus_input_current).unwrap()).unwrap(),
        evcharger_ac_power: serde_json::from_value(serde_json::to_value(&from.evcharger_ac_power).unwrap()).unwrap(),
        evcharger_ac_current: serde_json::from_value(serde_json::to_value(&from.evcharger_ac_current).unwrap()).unwrap(),
        ess_state: serde_json::from_value(serde_json::to_value(&from.ess_state).unwrap()).unwrap(),
        outdoor_temperature: serde_json::from_value(serde_json::to_value(&from.outdoor_temperature).unwrap()).unwrap(),
        session_kwh: serde_json::from_value(serde_json::to_value(&from.session_kwh).unwrap()).unwrap(),
        ev_soc: serde_json::from_value(serde_json::to_value(&from.ev_soc).unwrap()).unwrap(),
        ev_charge_target: serde_json::from_value(serde_json::to_value(&from.ev_charge_target).unwrap()).unwrap(),
    }
}