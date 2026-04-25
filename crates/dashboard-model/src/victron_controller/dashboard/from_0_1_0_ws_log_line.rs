

pub fn convert__ws_log_line__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::ws_log_line::WsLogLine) -> crate::victron_controller::dashboard::ws_log_line::WsLogLine {
    crate::victron_controller::dashboard::ws_log_line::WsLogLine {
        at_epoch_ms: from.at_epoch_ms.clone(),
        level: from.level.clone(),
        source: from.source.clone(),
        message: from.message.clone(),
    }
}