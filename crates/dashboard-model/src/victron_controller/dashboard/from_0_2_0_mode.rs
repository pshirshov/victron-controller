

pub fn convert__mode__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::mode::Mode) -> crate::victron_controller::dashboard::mode::Mode {
    from.to_string().parse().expect("enum parse")
}