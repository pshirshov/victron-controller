// @ts-nocheck
import {Sensors as v0_1_0_Sensors} from './v0_1_0/Sensors'
import {Sensors as dashboard_Sensors} from './Sensors'
import {ActualF64 as dashboard_ActualF64} from './ActualF64'
import {Freshness as dashboard_Freshness} from './Freshness'
import {convert__actual_f64__from__0_1_0} from './from_0_1_0_actual-f64'

// Manual conversion: 0.1.0 Sensors -> 0.2.0 Sensors. Additive change —
// the new `session_kwh` field has no source in 0.1.0, so initialise it
// to a freshness=Unknown, value=null ActualF64. Live data overwrites
// this on the first myenergi poll. PR-session-kwh-sensor.
export function convert__sensors__from__0_1_0(from: v0_1_0_Sensors): dashboard_Sensors {
    const c = convert__actual_f64__from__0_1_0
    return new dashboard_Sensors(
        c(from.battery_soc),
        c(from.battery_soh),
        c(from.battery_installed_capacity),
        c(from.battery_dc_power),
        c(from.mppt_power_0),
        c(from.mppt_power_1),
        c(from.soltaro_power),
        c(from.power_consumption),
        c(from.grid_power),
        c(from.grid_voltage),
        c(from.grid_current),
        c(from.consumption_current),
        c(from.offgrid_power),
        c(from.offgrid_current),
        c(from.vebus_input_current),
        c(from.evcharger_ac_power),
        c(from.evcharger_ac_current),
        c(from.ess_state),
        c(from.outdoor_temperature),
        new dashboard_ActualF64(null, dashboard_Freshness.Unknown, 0),
    )
}
