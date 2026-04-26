

pub fn convert__ws_client_message__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::ws_client_message::WsClientMessage) -> crate::victron_controller::dashboard::ws_client_message::WsClientMessage {
    match from {
        crate::victron_controller::dashboard::v0_2_0::ws_client_message::WsClientMessage::Ping(x) => crate::victron_controller::dashboard::ws_client_message::WsClientMessage::Ping(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::ws_client_message::WsClientMessage::SendCommand(x) => crate::victron_controller::dashboard::ws_client_message::WsClientMessage::SendCommand(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
    }
}