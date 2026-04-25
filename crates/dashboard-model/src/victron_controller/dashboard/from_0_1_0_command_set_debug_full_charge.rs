

pub fn convert__command__set_debug_full_charge__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::command::SetDebugFullCharge) -> crate::victron_controller::dashboard::command::SetDebugFullCharge {
    crate::victron_controller::dashboard::command::SetDebugFullCharge {
        value: serde_json::from_value(serde_json::to_value(&from.value).unwrap()).unwrap(),
    }
}