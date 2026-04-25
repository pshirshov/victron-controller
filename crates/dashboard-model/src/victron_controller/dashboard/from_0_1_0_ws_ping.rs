

pub fn convert__ws_ping__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::ws_ping::WsPing) -> crate::victron_controller::dashboard::ws_ping::WsPing {
    crate::victron_controller::dashboard::ws_ping::WsPing {
        nonce: from.nonce.clone(),
        client_ts_ms: from.client_ts_ms.clone(),
    }
}