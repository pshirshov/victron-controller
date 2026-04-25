// @ts-nocheck
import {SetUintKnob as v0_1_0_SetUintKnob} from './v0_1_0/Command'
import {SetUintKnob as dashboard_SetUintKnob} from './Command'

export function convert__command__set_uint_knob__from__0_1_0(from: v0_1_0_SetUintKnob): dashboard_SetUintKnob {
    return new dashboard_SetUintKnob (
        from.knob_name,
        from.value
    )
}