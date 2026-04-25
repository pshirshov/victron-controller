

pub fn convert__charge_battery_extended_mode__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::charge_battery_extended_mode::ChargeBatteryExtendedMode) -> crate::victron_controller::dashboard::charge_battery_extended_mode::ChargeBatteryExtendedMode {
    from.to_string().parse().expect("enum parse")
}