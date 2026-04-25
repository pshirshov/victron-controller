// @ts-nocheck
import {Ack as v0_1_0_Ack} from './v0_1_0/WsServerMessage'
import {Ack as dashboard_Ack} from './WsServerMessage'

export function convert__ws_server_message__ack__from__0_1_0(from: v0_1_0_Ack): dashboard_Ack {
    return new dashboard_Ack (
        JSON.parse(JSON.stringify(from.body))
    )
}