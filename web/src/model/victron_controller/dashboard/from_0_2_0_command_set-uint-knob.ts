// @ts-nocheck
import {SetUintKnob as v0_2_0_SetUintKnob} from './v0_2_0/Command'
import {SetUintKnob as dashboard_SetUintKnob} from './Command'

export function convert__command__set_uint_knob__from__0_2_0(from: v0_2_0_SetUintKnob): dashboard_SetUintKnob {
    return new dashboard_SetUintKnob (
        from.knob_name,
        from.value
    )
}