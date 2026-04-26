// @ts-nocheck
import {SetFloatKnob as v0_1_0_SetFloatKnob} from '../v0_1_0/Command'
import {SetFloatKnob as v0_2_0_SetFloatKnob} from './Command'

export function convert__command__set_float_knob__from__0_1_0(from: v0_1_0_SetFloatKnob): v0_2_0_SetFloatKnob {
    return new v0_2_0_SetFloatKnob (
        from.knob_name,
        from.value
    )
}