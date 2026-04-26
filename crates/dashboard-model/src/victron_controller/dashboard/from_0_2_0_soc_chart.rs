

pub fn convert__soc_chart__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::soc_chart::SocChart) -> crate::victron_controller::dashboard::soc_chart::SocChart {
    crate::victron_controller::dashboard::soc_chart::SocChart {
        history: serde_json::from_value(serde_json::to_value(&from.history).unwrap()).unwrap(),
        projection: serde_json::from_value(serde_json::to_value(&from.projection).unwrap()).unwrap(),
        now_epoch_ms: from.now_epoch_ms.clone(),
        now_soc_pct: from.now_soc_pct.clone(),
        discharge_target_pct: from.discharge_target_pct.clone(),
        charge_target_pct: from.charge_target_pct.clone(),
    }
}