

pub fn convert__actuated_i32__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::actuated_i32::ActuatedI32) -> crate::victron_controller::dashboard::v0_2_0::actuated_i32::ActuatedI32 {
    crate::victron_controller::dashboard::v0_2_0::actuated_i32::ActuatedI32 {
        target_value: from.target_value.clone(),
        target_owner: serde_json::from_value(serde_json::to_value(&from.target_owner).unwrap()).unwrap(),
        target_phase: serde_json::from_value(serde_json::to_value(&from.target_phase).unwrap()).unwrap(),
        target_since_epoch_ms: from.target_since_epoch_ms.clone(),
        actual: serde_json::from_value(serde_json::to_value(&from.actual).unwrap()).unwrap(),
    }
}