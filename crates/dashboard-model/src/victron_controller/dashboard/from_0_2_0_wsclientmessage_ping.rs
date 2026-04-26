

pub fn convert__wsclientmessage__ping__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::ws_client_message::Ping) -> crate::victron_controller::dashboard::ws_client_message::Ping {
    crate::victron_controller::dashboard::ws_client_message::Ping {
        body: serde_json::from_value(serde_json::to_value(&from.body).unwrap()).unwrap(),
    }
}