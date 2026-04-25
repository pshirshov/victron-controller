

pub fn convert__wsservermessage__pong__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::ws_server_message::Pong) -> crate::victron_controller::dashboard::ws_server_message::Pong {
    crate::victron_controller::dashboard::ws_server_message::Pong {
        body: serde_json::from_value(serde_json::to_value(&from.body).unwrap()).unwrap(),
    }
}