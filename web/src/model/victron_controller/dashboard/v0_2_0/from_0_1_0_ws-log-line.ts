// @ts-nocheck
import {WsLogLine as v0_1_0_WsLogLine} from '../v0_1_0/WsLogLine'
import {WsLogLine as v0_2_0_WsLogLine} from './WsLogLine'

export function convert__ws_log_line__from__0_1_0(from: v0_1_0_WsLogLine): v0_2_0_WsLogLine {
    return new v0_2_0_WsLogLine (
        from.at_epoch_ms,
        from.level,
        from.source,
        from.message
    )
}