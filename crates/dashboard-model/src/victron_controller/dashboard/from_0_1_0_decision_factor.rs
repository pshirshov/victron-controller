

pub fn convert__decision_factor__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::decision_factor::DecisionFactor) -> crate::victron_controller::dashboard::decision_factor::DecisionFactor {
    crate::victron_controller::dashboard::decision_factor::DecisionFactor {
        name: from.name.clone(),
        value: from.value.clone(),
    }
}