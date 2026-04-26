// @ts-nocheck
import {WsPing as v0_2_0_WsPing} from './WsPing'
import {WsPing as v0_1_0_WsPing} from '../v0_1_0/WsPing'

export function convert__ws_ping__from__0_1_0(from: v0_1_0_WsPing): v0_2_0_WsPing {
    return new v0_2_0_WsPing (
        from.nonce,
        from.client_ts_ms
    )
}