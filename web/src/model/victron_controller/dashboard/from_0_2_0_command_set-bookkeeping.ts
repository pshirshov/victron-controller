// @ts-nocheck
import {SetBookkeeping as v0_2_0_SetBookkeeping} from './v0_2_0/Command'
import {SetBookkeeping as dashboard_SetBookkeeping} from './Command'

export function convert__command__set_bookkeeping__from__0_2_0(from: v0_2_0_SetBookkeeping): dashboard_SetBookkeeping {
    return new dashboard_SetBookkeeping (
        JSON.parse(JSON.stringify(from.key)),
        JSON.parse(JSON.stringify(from.value))
    )
}