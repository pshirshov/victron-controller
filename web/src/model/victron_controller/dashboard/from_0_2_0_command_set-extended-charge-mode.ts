// @ts-nocheck
import {SetExtendedChargeMode as v0_2_0_SetExtendedChargeMode} from './v0_2_0/Command'
import {SetExtendedChargeMode as dashboard_SetExtendedChargeMode} from './Command'

export function convert__command__set_extended_charge_mode__from__0_2_0(from: v0_2_0_SetExtendedChargeMode): dashboard_SetExtendedChargeMode {
    return new dashboard_SetExtendedChargeMode (
        JSON.parse(JSON.stringify(from.value))
    )
}