// @ts-nocheck
import {Ack as v0_2_0_Ack} from './WsServerMessage'
import {Ack as v0_1_0_Ack} from '../v0_1_0/WsServerMessage'

export function convert__ws_server_message__ack__from__0_1_0(from: v0_1_0_Ack): v0_2_0_Ack {
    return new v0_2_0_Ack (
        JSON.parse(JSON.stringify(from.body))
    )
}