

pub fn convert__target_phase__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::target_phase::TargetPhase) -> crate::victron_controller::dashboard::v0_2_0::target_phase::TargetPhase {
    from.to_string().parse().expect("enum parse")
}