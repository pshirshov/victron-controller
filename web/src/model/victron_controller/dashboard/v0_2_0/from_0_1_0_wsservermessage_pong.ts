// @ts-nocheck
import {Pong as v0_2_0_Pong} from './WsServerMessage'
import {Pong as v0_1_0_Pong} from '../v0_1_0/WsServerMessage'

export function convert__ws_server_message__pong__from__0_1_0(from: v0_1_0_Pong): v0_2_0_Pong {
    return new v0_2_0_Pong (
        JSON.parse(JSON.stringify(from.body))
    )
}