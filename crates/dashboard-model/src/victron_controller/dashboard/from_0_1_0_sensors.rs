// Manual conversion: 0.1.0 Sensors -> 0.2.0 Sensors. Additive change —
// the new `session_kwh` field has no source in 0.1.0, so initialise it
// to a freshness=Unknown, value=None ActualF64 with since_epoch_ms=0.
// Live data overwrites this on the first myenergi poll. PR-session-kwh-sensor.

pub fn convert__sensors__from__0_1_0(
    from: &crate::victron_controller::dashboard::v0_1_0::sensors::Sensors,
) -> crate::victron_controller::dashboard::sensors::Sensors {
    use crate::victron_controller::dashboard::actual_f64::ActualF64;
    use crate::victron_controller::dashboard::freshness::Freshness;
    let convert =
        crate::victron_controller::dashboard::from_0_1_0_actual_f64::convert__actual_f64__from__0_1_0;
    crate::victron_controller::dashboard::sensors::Sensors {
        battery_soc: convert(&from.battery_soc),
        battery_soh: convert(&from.battery_soh),
        battery_installed_capacity: convert(&from.battery_installed_capacity),
        battery_dc_power: convert(&from.battery_dc_power),
        mppt_power_0: convert(&from.mppt_power_0),
        mppt_power_1: convert(&from.mppt_power_1),
        soltaro_power: convert(&from.soltaro_power),
        power_consumption: convert(&from.power_consumption),
        grid_power: convert(&from.grid_power),
        grid_voltage: convert(&from.grid_voltage),
        grid_current: convert(&from.grid_current),
        consumption_current: convert(&from.consumption_current),
        offgrid_power: convert(&from.offgrid_power),
        offgrid_current: convert(&from.offgrid_current),
        vebus_input_current: convert(&from.vebus_input_current),
        evcharger_ac_power: convert(&from.evcharger_ac_power),
        evcharger_ac_current: convert(&from.evcharger_ac_current),
        ess_state: convert(&from.ess_state),
        outdoor_temperature: convert(&from.outdoor_temperature),
        session_kwh: ActualF64 {
            value: None,
            freshness: Freshness::Unknown,
            since_epoch_ms: 0,
        },
    }
}
