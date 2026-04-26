

pub fn convert__actuated_schedule__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::actuated_schedule::ActuatedSchedule) -> crate::victron_controller::dashboard::actuated_schedule::ActuatedSchedule {
    crate::victron_controller::dashboard::actuated_schedule::ActuatedSchedule {
        target: serde_json::from_value(serde_json::to_value(&from.target).unwrap()).unwrap(),
        target_owner: serde_json::from_value(serde_json::to_value(&from.target_owner).unwrap()).unwrap(),
        target_phase: serde_json::from_value(serde_json::to_value(&from.target_phase).unwrap()).unwrap(),
        target_since_epoch_ms: from.target_since_epoch_ms.clone(),
        actual: serde_json::from_value(serde_json::to_value(&from.actual).unwrap()).unwrap(),
        actual_freshness: serde_json::from_value(serde_json::to_value(&from.actual_freshness).unwrap()).unwrap(),
        actual_since_epoch_ms: from.actual_since_epoch_ms.clone(),
    }
}