// @ts-nocheck
import {WsPing as v0_1_0_WsPing} from './v0_1_0/WsPing'
import {WsPing as dashboard_WsPing} from './WsPing'

export function convert__ws_ping__from__0_1_0(from: v0_1_0_WsPing): dashboard_WsPing {
    return new dashboard_WsPing (
        from.nonce,
        from.client_ts_ms
    )
}