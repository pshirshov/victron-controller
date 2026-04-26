// @ts-nocheck
import {Hello as v0_2_0_Hello} from './WsServerMessage'
import {Hello as v0_1_0_Hello} from '../v0_1_0/WsServerMessage'

export function convert__ws_server_message__hello__from__0_1_0(from: v0_1_0_Hello): v0_2_0_Hello {
    return new v0_2_0_Hello (
        from.server_version,
        from.server_ts_ms
    )
}