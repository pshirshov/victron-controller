// @ts-nocheck
import {Ping as v0_1_0_Ping} from '../v0_1_0/WsClientMessage'
import {Ping as v0_2_0_Ping} from './WsClientMessage'

export function convert__ws_client_message__ping__from__0_1_0(from: v0_1_0_Ping): v0_2_0_Ping {
    return new v0_2_0_Ping (
        JSON.parse(JSON.stringify(from.body))
    )
}