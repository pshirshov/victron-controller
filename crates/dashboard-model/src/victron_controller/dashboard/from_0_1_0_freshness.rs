

pub fn convert__freshness__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::freshness::Freshness) -> crate::victron_controller::dashboard::freshness::Freshness {
    from.to_string().parse().expect("enum parse")
}