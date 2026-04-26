

pub fn convert__extended_charge_mode__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::extended_charge_mode::ExtendedChargeMode) -> crate::victron_controller::dashboard::extended_charge_mode::ExtendedChargeMode {
    from.to_string().parse().expect("enum parse")
}