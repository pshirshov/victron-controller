// @ts-nocheck
import {WsServerMessage as v0_2_0_WsServerMessage, Hello as v0_2_0_Hello, Pong as v0_2_0_Pong, Snapshot as v0_2_0_Snapshot, Log as v0_2_0_Log, Ack as v0_2_0_Ack} from './WsServerMessage'
import {WsServerMessage as v0_1_0_WsServerMessage, Hello as v0_1_0_Hello, Pong as v0_1_0_Pong, Snapshot as v0_1_0_Snapshot, Log as v0_1_0_Log, Ack as v0_1_0_Ack} from '../v0_1_0/WsServerMessage'

export function convert__ws_server_message__from__0_1_0(from: v0_1_0_WsServerMessage): v0_2_0_WsServerMessage {
    if (from instanceof v0_1_0_Hello) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_Hello
    }
    if (from instanceof v0_1_0_Pong) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_Pong
    }
    if (from instanceof v0_1_0_Snapshot) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_Snapshot
    }
    if (from instanceof v0_1_0_Log) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_Log
    }
    if (from instanceof v0_1_0_Ack) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_Ack
    }

    throw new Error("Unknown ADT branch: " + from);
}