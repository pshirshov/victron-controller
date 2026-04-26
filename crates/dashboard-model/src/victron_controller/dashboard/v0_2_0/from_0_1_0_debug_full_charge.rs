

pub fn convert__debug_full_charge__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::debug_full_charge::DebugFullCharge) -> crate::victron_controller::dashboard::v0_2_0::debug_full_charge::DebugFullCharge {
    from.to_string().parse().expect("enum parse")
}