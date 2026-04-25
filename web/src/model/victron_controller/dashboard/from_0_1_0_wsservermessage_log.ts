// @ts-nocheck
import {Log as v0_1_0_Log} from './v0_1_0/WsServerMessage'
import {Log as dashboard_Log} from './WsServerMessage'

export function convert__ws_server_message__log__from__0_1_0(from: v0_1_0_Log): dashboard_Log {
    return new dashboard_Log (
        JSON.parse(JSON.stringify(from.body))
    )
}