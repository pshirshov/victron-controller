

pub fn convert__soc_history_sample__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::soc_history_sample::SocHistorySample) -> crate::victron_controller::dashboard::soc_history_sample::SocHistorySample {
    crate::victron_controller::dashboard::soc_history_sample::SocHistorySample {
        epoch_ms: from.epoch_ms.clone(),
        soc_pct: from.soc_pct.clone(),
    }
}