

pub fn convert__bookkeeping_key__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::bookkeeping_key::BookkeepingKey) -> crate::victron_controller::dashboard::bookkeeping_key::BookkeepingKey {
    from.to_string().parse().expect("enum parse")
}