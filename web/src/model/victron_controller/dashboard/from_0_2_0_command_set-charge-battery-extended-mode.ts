// @ts-nocheck
import {SetChargeBatteryExtendedMode as v0_2_0_SetChargeBatteryExtendedMode} from './v0_2_0/Command'
import {SetChargeBatteryExtendedMode as dashboard_SetChargeBatteryExtendedMode} from './Command'

export function convert__command__set_charge_battery_extended_mode__from__0_2_0(from: v0_2_0_SetChargeBatteryExtendedMode): dashboard_SetChargeBatteryExtendedMode {
    return new dashboard_SetChargeBatteryExtendedMode (
        JSON.parse(JSON.stringify(from.value))
    )
}