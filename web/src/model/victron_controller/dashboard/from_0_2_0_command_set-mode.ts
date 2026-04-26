// @ts-nocheck
import {SetMode as v0_2_0_SetMode} from './v0_2_0/Command'
import {SetMode as dashboard_SetMode} from './Command'

export function convert__command__set_mode__from__0_2_0(from: v0_2_0_SetMode): dashboard_SetMode {
    return new dashboard_SetMode (
        from.knob_name,
        JSON.parse(JSON.stringify(from.value))
    )
}