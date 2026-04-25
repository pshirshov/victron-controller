// @ts-nocheck
import {SetKillSwitch as v0_1_0_SetKillSwitch} from './v0_1_0/Command'
import {SetKillSwitch as dashboard_SetKillSwitch} from './Command'

export function convert__command__set_kill_switch__from__0_1_0(from: v0_1_0_SetKillSwitch): dashboard_SetKillSwitch {
    return new dashboard_SetKillSwitch (
        from.value
    )
}