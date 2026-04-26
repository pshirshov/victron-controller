// @ts-nocheck
import {ActualI32 as v0_2_0_ActualI32} from './v0_2_0/ActualI32'
import {ActualI32 as dashboard_ActualI32} from './ActualI32'

export function convert__actual_i32__from__0_2_0(from: v0_2_0_ActualI32): dashboard_ActualI32 {
    return new dashboard_ActualI32 (
        from.value,
        JSON.parse(JSON.stringify(from.freshness)),
        from.since_epoch_ms
    )
}