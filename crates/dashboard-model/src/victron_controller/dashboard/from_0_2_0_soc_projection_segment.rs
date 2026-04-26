

pub fn convert__soc_projection_segment__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::soc_projection_segment::SocProjectionSegment) -> crate::victron_controller::dashboard::soc_projection_segment::SocProjectionSegment {
    crate::victron_controller::dashboard::soc_projection_segment::SocProjectionSegment {
        start_epoch_ms: from.start_epoch_ms.clone(),
        end_epoch_ms: from.end_epoch_ms.clone(),
        start_soc_pct: from.start_soc_pct.clone(),
        end_soc_pct: from.end_soc_pct.clone(),
        kind: serde_json::from_value(serde_json::to_value(&from.kind).unwrap()).unwrap(),
    }
}