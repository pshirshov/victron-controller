// @ts-nocheck
import {Snapshot as v0_2_0_Snapshot} from './v0_2_0/WsServerMessage'
import {Snapshot as dashboard_Snapshot} from './WsServerMessage'

export function convert__ws_server_message__snapshot__from__0_2_0(from: v0_2_0_Snapshot): dashboard_Snapshot {
    return new dashboard_Snapshot (
        JSON.parse(JSON.stringify(from.body))
    )
}