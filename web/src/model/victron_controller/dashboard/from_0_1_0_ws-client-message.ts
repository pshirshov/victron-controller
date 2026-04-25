// @ts-nocheck
import {WsClientMessage as v0_1_0_WsClientMessage, Ping as v0_1_0_Ping, SendCommand as v0_1_0_SendCommand} from './v0_1_0/WsClientMessage'
import {WsClientMessage as dashboard_WsClientMessage, Ping as dashboard_Ping, SendCommand as dashboard_SendCommand} from './WsClientMessage'

export function convert__ws_client_message__from__0_1_0(from: v0_1_0_WsClientMessage): dashboard_WsClientMessage {
    if (from instanceof v0_1_0_Ping) {
        return JSON.parse(JSON.stringify(from)) as dashboard_Ping
    }
    if (from instanceof v0_1_0_SendCommand) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SendCommand
    }

    throw new Error("Unknown ADT branch: " + from);
}