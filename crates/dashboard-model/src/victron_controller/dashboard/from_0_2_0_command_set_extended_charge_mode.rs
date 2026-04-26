

pub fn convert__command__set_extended_charge_mode__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::command::SetExtendedChargeMode) -> crate::victron_controller::dashboard::command::SetExtendedChargeMode {
    crate::victron_controller::dashboard::command::SetExtendedChargeMode {
        value: serde_json::from_value(serde_json::to_value(&from.value).unwrap()).unwrap(),
    }
}