// @ts-nocheck
import {CommandAck as v0_1_0_CommandAck} from '../v0_1_0/CommandAck'
import {CommandAck as v0_2_0_CommandAck} from './CommandAck'

export function convert__command_ack__from__0_1_0(from: v0_1_0_CommandAck): v0_2_0_CommandAck {
    return new v0_2_0_CommandAck (
        from.accepted,
        from.error_message
    )
}