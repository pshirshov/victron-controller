// @ts-nocheck
import {Log as v0_2_0_Log} from './WsServerMessage'
import {Log as v0_1_0_Log} from '../v0_1_0/WsServerMessage'

export function convert__ws_server_message__log__from__0_1_0(from: v0_1_0_Log): v0_2_0_Log {
    return new v0_2_0_Log (
        JSON.parse(JSON.stringify(from.body))
    )
}