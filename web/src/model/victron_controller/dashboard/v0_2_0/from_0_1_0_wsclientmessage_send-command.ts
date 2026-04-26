// @ts-nocheck
import {SendCommand as v0_1_0_SendCommand} from '../v0_1_0/WsClientMessage'
import {SendCommand as v0_2_0_SendCommand} from './WsClientMessage'

export function convert__ws_client_message__send_command__from__0_1_0(from: v0_1_0_SendCommand): v0_2_0_SendCommand {
    return new v0_2_0_SendCommand (
        JSON.parse(JSON.stringify(from.body))
    )
}