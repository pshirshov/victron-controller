

pub fn convert__owner__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::owner::Owner) -> crate::victron_controller::dashboard::v0_2_0::owner::Owner {
    from.to_string().parse().expect("enum parse")
}