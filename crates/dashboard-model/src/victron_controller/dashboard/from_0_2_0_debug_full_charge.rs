

pub fn convert__debug_full_charge__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::debug_full_charge::DebugFullCharge) -> crate::victron_controller::dashboard::debug_full_charge::DebugFullCharge {
    from.to_string().parse().expect("enum parse")
}