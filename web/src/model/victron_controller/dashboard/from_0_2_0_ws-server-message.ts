// @ts-nocheck
import {WsServerMessage as v0_2_0_WsServerMessage, Hello as v0_2_0_Hello, Pong as v0_2_0_Pong, Snapshot as v0_2_0_Snapshot, Log as v0_2_0_Log, Ack as v0_2_0_Ack} from './v0_2_0/WsServerMessage'
import {WsServerMessage as dashboard_WsServerMessage, Hello as dashboard_Hello, Pong as dashboard_Pong, Snapshot as dashboard_Snapshot, Log as dashboard_Log, Ack as dashboard_Ack} from './WsServerMessage'

export function convert__ws_server_message__from__0_2_0(from: v0_2_0_WsServerMessage): dashboard_WsServerMessage {
    if (from instanceof v0_2_0_Hello) {
        return JSON.parse(JSON.stringify(from)) as dashboard_Hello
    }
    if (from instanceof v0_2_0_Pong) {
        return JSON.parse(JSON.stringify(from)) as dashboard_Pong
    }
    if (from instanceof v0_2_0_Snapshot) {
        return JSON.parse(JSON.stringify(from)) as dashboard_Snapshot
    }
    if (from instanceof v0_2_0_Log) {
        return JSON.parse(JSON.stringify(from)) as dashboard_Log
    }
    if (from instanceof v0_2_0_Ack) {
        return JSON.parse(JSON.stringify(from)) as dashboard_Ack
    }

    throw new Error("Unknown ADT branch: " + from);
}