

pub fn convert__ws_server_message__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::ws_server_message::WsServerMessage) -> crate::victron_controller::dashboard::ws_server_message::WsServerMessage {
    match from {
        crate::victron_controller::dashboard::v0_1_0::ws_server_message::WsServerMessage::Hello(x) => crate::victron_controller::dashboard::ws_server_message::WsServerMessage::Hello(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_1_0::ws_server_message::WsServerMessage::Pong(x) => crate::victron_controller::dashboard::ws_server_message::WsServerMessage::Pong(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_1_0::ws_server_message::WsServerMessage::Snapshot(x) => crate::victron_controller::dashboard::ws_server_message::WsServerMessage::Snapshot(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_1_0::ws_server_message::WsServerMessage::Log(x) => crate::victron_controller::dashboard::ws_server_message::WsServerMessage::Log(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_1_0::ws_server_message::WsServerMessage::Ack(x) => crate::victron_controller::dashboard::ws_server_message::WsServerMessage::Ack(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
    }
}