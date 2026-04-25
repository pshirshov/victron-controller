// @ts-nocheck
import {WsLogLine as v0_1_0_WsLogLine} from './v0_1_0/WsLogLine'
import {WsLogLine as dashboard_WsLogLine} from './WsLogLine'

export function convert__ws_log_line__from__0_1_0(from: v0_1_0_WsLogLine): dashboard_WsLogLine {
    return new dashboard_WsLogLine (
        from.at_epoch_ms,
        from.level,
        from.source,
        from.message
    )
}