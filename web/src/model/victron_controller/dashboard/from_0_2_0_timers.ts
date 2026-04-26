// @ts-nocheck
import {Timers as v0_2_0_Timers} from './v0_2_0/Timers'
import {Timers as dashboard_Timers} from './Timers'

export function convert__timers__from__0_2_0(from: v0_2_0_Timers): dashboard_Timers {
    return new dashboard_Timers (
        JSON.parse(JSON.stringify(from.entries))
    )
}