

pub fn convert__command__set_mode__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::command::SetMode) -> crate::victron_controller::dashboard::command::SetMode {
    crate::victron_controller::dashboard::command::SetMode {
        knob_name: from.knob_name.clone(),
        value: serde_json::from_value(serde_json::to_value(&from.value).unwrap()).unwrap(),
    }
}