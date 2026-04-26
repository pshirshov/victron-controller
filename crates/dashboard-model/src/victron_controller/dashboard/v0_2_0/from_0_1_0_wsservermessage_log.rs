

pub fn convert__wsservermessage__log__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::ws_server_message::Log) -> crate::victron_controller::dashboard::v0_2_0::ws_server_message::Log {
    crate::victron_controller::dashboard::v0_2_0::ws_server_message::Log {
        body: serde_json::from_value(serde_json::to_value(&from.body).unwrap()).unwrap(),
    }
}