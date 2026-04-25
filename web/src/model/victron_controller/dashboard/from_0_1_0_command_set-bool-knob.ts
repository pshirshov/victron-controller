// @ts-nocheck
import {SetBoolKnob as v0_1_0_SetBoolKnob} from './v0_1_0/Command'
import {SetBoolKnob as dashboard_SetBoolKnob} from './Command'

export function convert__command__set_bool_knob__from__0_1_0(from: v0_1_0_SetBoolKnob): dashboard_SetBoolKnob {
    return new dashboard_SetBoolKnob (
        from.knob_name,
        from.value
    )
}