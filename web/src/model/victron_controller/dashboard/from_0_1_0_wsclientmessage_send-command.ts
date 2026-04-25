// @ts-nocheck
import {SendCommand as v0_1_0_SendCommand} from './v0_1_0/WsClientMessage'
import {SendCommand as dashboard_SendCommand} from './WsClientMessage'

export function convert__ws_client_message__send_command__from__0_1_0(from: v0_1_0_SendCommand): dashboard_SendCommand {
    return new dashboard_SendCommand (
        JSON.parse(JSON.stringify(from.body))
    )
}