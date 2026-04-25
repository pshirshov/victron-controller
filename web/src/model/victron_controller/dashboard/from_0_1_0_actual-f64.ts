// @ts-nocheck
import {ActualF64 as v0_1_0_ActualF64} from './v0_1_0/ActualF64'
import {ActualF64 as dashboard_ActualF64} from './ActualF64'

export function convert__actual_f64__from__0_1_0(from: v0_1_0_ActualF64): dashboard_ActualF64 {
    return new dashboard_ActualF64 (
        from.value,
        JSON.parse(JSON.stringify(from.freshness)),
        from.since_epoch_ms
    )
}