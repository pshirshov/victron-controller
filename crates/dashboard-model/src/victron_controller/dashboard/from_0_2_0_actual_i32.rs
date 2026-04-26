

pub fn convert__actual_i32__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::actual_i32::ActualI32) -> crate::victron_controller::dashboard::actual_i32::ActualI32 {
    crate::victron_controller::dashboard::actual_i32::ActualI32 {
        value: from.value.clone(),
        freshness: serde_json::from_value(serde_json::to_value(&from.freshness).unwrap()).unwrap(),
        since_epoch_ms: from.since_epoch_ms.clone(),
    }
}