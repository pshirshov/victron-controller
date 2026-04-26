// @ts-nocheck
import {ActuatedF64 as v0_1_0_ActuatedF64} from '../v0_1_0/ActuatedF64'
import {ActuatedF64 as v0_2_0_ActuatedF64} from './ActuatedF64'

export function convert__actuated_f64__from__0_1_0(from: v0_1_0_ActuatedF64): v0_2_0_ActuatedF64 {
    return new v0_2_0_ActuatedF64 (
        from.target_value,
        JSON.parse(JSON.stringify(from.target_owner)),
        JSON.parse(JSON.stringify(from.target_phase)),
        from.target_since_epoch_ms,
        JSON.parse(JSON.stringify(from.actual))
    )
}