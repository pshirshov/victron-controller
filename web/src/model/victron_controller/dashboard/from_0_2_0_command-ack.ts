// @ts-nocheck
import {CommandAck as v0_2_0_CommandAck} from './v0_2_0/CommandAck'
import {CommandAck as dashboard_CommandAck} from './CommandAck'

export function convert__command_ack__from__0_2_0(from: v0_2_0_CommandAck): dashboard_CommandAck {
    return new dashboard_CommandAck (
        from.accepted,
        from.error_message
    )
}