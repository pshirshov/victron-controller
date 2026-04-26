// @ts-nocheck
import {Snapshot as v0_2_0_Snapshot} from './WsServerMessage'
import {Snapshot as v0_1_0_Snapshot} from '../v0_1_0/WsServerMessage'

export function convert__ws_server_message__snapshot__from__0_1_0(from: v0_1_0_Snapshot): v0_2_0_Snapshot {
    return new v0_2_0_Snapshot (
        JSON.parse(JSON.stringify(from.body))
    )
}