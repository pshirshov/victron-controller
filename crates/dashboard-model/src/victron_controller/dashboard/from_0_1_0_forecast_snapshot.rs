

pub fn convert__forecast_snapshot__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::forecast_snapshot::ForecastSnapshot) -> crate::victron_controller::dashboard::forecast_snapshot::ForecastSnapshot {
    crate::victron_controller::dashboard::forecast_snapshot::ForecastSnapshot {
        today_kwh: from.today_kwh.clone(),
        tomorrow_kwh: from.tomorrow_kwh.clone(),
        fetched_at_epoch_ms: from.fetched_at_epoch_ms.clone(),
    }
}