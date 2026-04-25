

pub fn convert__wsservermessage__hello__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::ws_server_message::Hello) -> crate::victron_controller::dashboard::ws_server_message::Hello {
    crate::victron_controller::dashboard::ws_server_message::Hello {
        server_version: from.server_version.clone(),
        server_ts_ms: from.server_ts_ms.clone(),
    }
}