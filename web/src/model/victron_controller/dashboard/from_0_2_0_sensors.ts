// @ts-nocheck
import {Sensors as v0_2_0_Sensors} from './v0_2_0/Sensors'
import {Sensors as dashboard_Sensors} from './Sensors'

export function convert__sensors__from__0_2_0(from: v0_2_0_Sensors): dashboard_Sensors {
    return new dashboard_Sensors (
        JSON.parse(JSON.stringify(from.battery_soc)),
        JSON.parse(JSON.stringify(from.battery_soh)),
        JSON.parse(JSON.stringify(from.battery_installed_capacity)),
        JSON.parse(JSON.stringify(from.battery_dc_power)),
        JSON.parse(JSON.stringify(from.mppt_power_0)),
        JSON.parse(JSON.stringify(from.mppt_power_1)),
        JSON.parse(JSON.stringify(from.soltaro_power)),
        JSON.parse(JSON.stringify(from.power_consumption)),
        JSON.parse(JSON.stringify(from.grid_power)),
        JSON.parse(JSON.stringify(from.grid_voltage)),
        JSON.parse(JSON.stringify(from.grid_current)),
        JSON.parse(JSON.stringify(from.consumption_current)),
        JSON.parse(JSON.stringify(from.offgrid_power)),
        JSON.parse(JSON.stringify(from.offgrid_current)),
        JSON.parse(JSON.stringify(from.vebus_input_current)),
        JSON.parse(JSON.stringify(from.evcharger_ac_power)),
        JSON.parse(JSON.stringify(from.evcharger_ac_current)),
        JSON.parse(JSON.stringify(from.ess_state)),
        JSON.parse(JSON.stringify(from.outdoor_temperature)),
        JSON.parse(JSON.stringify(from.session_kwh)),
        JSON.parse(JSON.stringify(from.ev_soc)),
        JSON.parse(JSON.stringify(from.ev_charge_target))
    )
}