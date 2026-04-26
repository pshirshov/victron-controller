

pub fn convert__soc_projection__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::soc_projection::SocProjection) -> crate::victron_controller::dashboard::soc_projection::SocProjection {
    crate::victron_controller::dashboard::soc_projection::SocProjection {
        segments: serde_json::from_value(serde_json::to_value(&from.segments).unwrap()).unwrap(),
        net_power_w: from.net_power_w.clone(),
        capacity_wh: from.capacity_wh.clone(),
        charge_rate_w: from.charge_rate_w.clone(),
    }
}