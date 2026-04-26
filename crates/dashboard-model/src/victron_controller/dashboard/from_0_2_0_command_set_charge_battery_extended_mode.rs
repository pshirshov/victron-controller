

pub fn convert__command__set_charge_battery_extended_mode__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::command::SetChargeBatteryExtendedMode) -> crate::victron_controller::dashboard::command::SetChargeBatteryExtendedMode {
    crate::victron_controller::dashboard::command::SetChargeBatteryExtendedMode {
        value: serde_json::from_value(serde_json::to_value(&from.value).unwrap()).unwrap(),
    }
}