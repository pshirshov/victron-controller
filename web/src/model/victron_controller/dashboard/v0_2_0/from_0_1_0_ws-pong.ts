// @ts-nocheck
import {WsPong as v0_2_0_WsPong} from './WsPong'
import {WsPong as v0_1_0_WsPong} from '../v0_1_0/WsPong'

export function convert__ws_pong__from__0_1_0(from: v0_1_0_WsPong): v0_2_0_WsPong {
    return new v0_2_0_WsPong (
        from.nonce,
        from.client_ts_ms,
        from.server_ts_ms
    )
}