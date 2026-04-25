// @ts-nocheck
import {Hello as v0_1_0_Hello} from './v0_1_0/WsServerMessage'
import {Hello as dashboard_Hello} from './WsServerMessage'

export function convert__ws_server_message__hello__from__0_1_0(from: v0_1_0_Hello): dashboard_Hello {
    return new dashboard_Hello (
        from.server_version,
        from.server_ts_ms
    )
}