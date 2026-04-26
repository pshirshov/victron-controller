// @ts-nocheck
import {SetDebugFullCharge as v0_2_0_SetDebugFullCharge} from './v0_2_0/Command'
import {SetDebugFullCharge as dashboard_SetDebugFullCharge} from './Command'

export function convert__command__set_debug_full_charge__from__0_2_0(from: v0_2_0_SetDebugFullCharge): dashboard_SetDebugFullCharge {
    return new dashboard_SetDebugFullCharge (
        JSON.parse(JSON.stringify(from.value))
    )
}