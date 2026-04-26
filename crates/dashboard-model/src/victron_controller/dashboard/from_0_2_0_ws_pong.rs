

pub fn convert__ws_pong__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::ws_pong::WsPong) -> crate::victron_controller::dashboard::ws_pong::WsPong {
    crate::victron_controller::dashboard::ws_pong::WsPong {
        nonce: from.nonce.clone(),
        client_ts_ms: from.client_ts_ms.clone(),
        server_ts_ms: from.server_ts_ms.clone(),
    }
}