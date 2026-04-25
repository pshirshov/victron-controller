

pub fn convert__forecast_disagreement_strategy__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::forecast_disagreement_strategy::ForecastDisagreementStrategy) -> crate::victron_controller::dashboard::forecast_disagreement_strategy::ForecastDisagreementStrategy {
    from.to_string().parse().expect("enum parse")
}