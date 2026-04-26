

pub fn convert__discharge_time__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::discharge_time::DischargeTime) -> crate::victron_controller::dashboard::v0_2_0::discharge_time::DischargeTime {
    from.to_string().parse().expect("enum parse")
}