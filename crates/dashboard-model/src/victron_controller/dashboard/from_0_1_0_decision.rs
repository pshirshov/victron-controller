

pub fn convert__decision__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::decision::Decision) -> crate::victron_controller::dashboard::decision::Decision {
    crate::victron_controller::dashboard::decision::Decision {
        summary: from.summary.clone(),
        factors: serde_json::from_value(serde_json::to_value(&from.factors).unwrap()).unwrap(),
    }
}