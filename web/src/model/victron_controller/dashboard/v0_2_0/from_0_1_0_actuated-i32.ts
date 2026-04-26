// @ts-nocheck
import {ActuatedI32 as v0_1_0_ActuatedI32} from '../v0_1_0/ActuatedI32'
import {ActuatedI32 as v0_2_0_ActuatedI32} from './ActuatedI32'

export function convert__actuated_i32__from__0_1_0(from: v0_1_0_ActuatedI32): v0_2_0_ActuatedI32 {
    return new v0_2_0_ActuatedI32 (
        from.target_value,
        JSON.parse(JSON.stringify(from.target_owner)),
        JSON.parse(JSON.stringify(from.target_phase)),
        from.target_since_epoch_ms,
        JSON.parse(JSON.stringify(from.actual))
    )
}