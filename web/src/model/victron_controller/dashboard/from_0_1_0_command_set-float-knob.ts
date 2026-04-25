// @ts-nocheck
import {SetFloatKnob as v0_1_0_SetFloatKnob} from './v0_1_0/Command'
import {SetFloatKnob as dashboard_SetFloatKnob} from './Command'

export function convert__command__set_float_knob__from__0_1_0(from: v0_1_0_SetFloatKnob): dashboard_SetFloatKnob {
    return new dashboard_SetFloatKnob (
        from.knob_name,
        from.value
    )
}