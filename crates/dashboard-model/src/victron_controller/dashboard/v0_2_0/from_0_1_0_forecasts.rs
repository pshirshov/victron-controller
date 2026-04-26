

pub fn convert__forecasts__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::forecasts::Forecasts) -> crate::victron_controller::dashboard::v0_2_0::forecasts::Forecasts {
    crate::victron_controller::dashboard::v0_2_0::forecasts::Forecasts {
        solcast: serde_json::from_value(serde_json::to_value(&from.solcast).unwrap()).unwrap(),
        forecast_solar: serde_json::from_value(serde_json::to_value(&from.forecast_solar).unwrap()).unwrap(),
        open_meteo: serde_json::from_value(serde_json::to_value(&from.open_meteo).unwrap()).unwrap(),
    }
}