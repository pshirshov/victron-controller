// @ts-nocheck
import {SetChargeBatteryExtendedMode as v0_1_0_SetChargeBatteryExtendedMode} from '../v0_1_0/Command'
import {SetChargeBatteryExtendedMode as v0_2_0_SetChargeBatteryExtendedMode} from './Command'

export function convert__command__set_charge_battery_extended_mode__from__0_1_0(from: v0_1_0_SetChargeBatteryExtendedMode): v0_2_0_SetChargeBatteryExtendedMode {
    return new v0_2_0_SetChargeBatteryExtendedMode (
        JSON.parse(JSON.stringify(from.value))
    )
}