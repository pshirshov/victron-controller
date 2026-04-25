// @ts-nocheck
import {WsPong as v0_1_0_WsPong} from './v0_1_0/WsPong'
import {WsPong as dashboard_WsPong} from './WsPong'

export function convert__ws_pong__from__0_1_0(from: v0_1_0_WsPong): dashboard_WsPong {
    return new dashboard_WsPong (
        from.nonce,
        from.client_ts_ms,
        from.server_ts_ms
    )
}