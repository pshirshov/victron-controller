

pub fn convert__command__set_forecast_disagreement_strategy__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::command::SetForecastDisagreementStrategy) -> crate::victron_controller::dashboard::command::SetForecastDisagreementStrategy {
    crate::victron_controller::dashboard::command::SetForecastDisagreementStrategy {
        value: serde_json::from_value(serde_json::to_value(&from.value).unwrap()).unwrap(),
    }
}