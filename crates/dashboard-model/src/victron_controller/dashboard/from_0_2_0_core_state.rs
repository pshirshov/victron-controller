

pub fn convert__core_state__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::core_state::CoreState) -> crate::victron_controller::dashboard::core_state::CoreState {
    crate::victron_controller::dashboard::core_state::CoreState {
        id: from.id.clone(),
        depends_on: from.depends_on.clone(),
        last_run_outcome: from.last_run_outcome.clone(),
        last_payload: from.last_payload.clone(),
        last_inputs: serde_json::from_value(serde_json::to_value(&from.last_inputs).unwrap()).unwrap(),
        last_outputs: serde_json::from_value(serde_json::to_value(&from.last_outputs).unwrap()).unwrap(),
    }
}