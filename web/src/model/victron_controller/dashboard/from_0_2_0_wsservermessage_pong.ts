// @ts-nocheck
import {Pong as v0_2_0_Pong} from './v0_2_0/WsServerMessage'
import {Pong as dashboard_Pong} from './WsServerMessage'

export function convert__ws_server_message__pong__from__0_2_0(from: v0_2_0_Pong): dashboard_Pong {
    return new dashboard_Pong (
        JSON.parse(JSON.stringify(from.body))
    )
}