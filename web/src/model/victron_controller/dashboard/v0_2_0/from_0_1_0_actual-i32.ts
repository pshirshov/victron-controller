// @ts-nocheck
import {ActualI32 as v0_2_0_ActualI32} from './ActualI32'
import {ActualI32 as v0_1_0_ActualI32} from '../v0_1_0/ActualI32'

export function convert__actual_i32__from__0_1_0(from: v0_1_0_ActualI32): v0_2_0_ActualI32 {
    return new v0_2_0_ActualI32 (
        from.value,
        JSON.parse(JSON.stringify(from.freshness)),
        from.since_epoch_ms
    )
}