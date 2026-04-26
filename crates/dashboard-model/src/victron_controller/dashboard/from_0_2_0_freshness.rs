

pub fn convert__freshness__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::freshness::Freshness) -> crate::victron_controller::dashboard::freshness::Freshness {
    from.to_string().parse().expect("enum parse")
}