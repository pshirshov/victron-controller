

pub fn convert__command__set_discharge_time__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::command::SetDischargeTime) -> crate::victron_controller::dashboard::command::SetDischargeTime {
    crate::victron_controller::dashboard::command::SetDischargeTime {
        value: serde_json::from_value(serde_json::to_value(&from.value).unwrap()).unwrap(),
    }
}