// @ts-nocheck
import {SetKillSwitch as v0_1_0_SetKillSwitch} from '../v0_1_0/Command'
import {SetKillSwitch as v0_2_0_SetKillSwitch} from './Command'

export function convert__command__set_kill_switch__from__0_1_0(from: v0_1_0_SetKillSwitch): v0_2_0_SetKillSwitch {
    return new v0_2_0_SetKillSwitch (
        from.value
    )
}