

pub fn convert__cores_state__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::cores_state::CoresState) -> crate::victron_controller::dashboard::cores_state::CoresState {
    crate::victron_controller::dashboard::cores_state::CoresState {
        cores: serde_json::from_value(serde_json::to_value(&from.cores).unwrap()).unwrap(),
        topo_order: from.topo_order.clone(),
    }
}