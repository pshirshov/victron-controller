// PR-session-kwh-D01 fix: bridge `sensors` through the hand-written
// converter so the new `session_kwh` field is initialised correctly.
// The default JSON-roundtrip path would panic with `missing field
// 'session_kwh'` because 0.2.0 Sensors does NOT carry `#[serde(default)]`.

pub fn convert__world_snapshot__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::world_snapshot::WorldSnapshot) -> crate::victron_controller::dashboard::world_snapshot::WorldSnapshot {
    crate::victron_controller::dashboard::world_snapshot::WorldSnapshot {
        captured_at_epoch_ms: from.captured_at_epoch_ms.clone(),
        captured_at_naive_iso: from.captured_at_naive_iso.clone(),
        sensors: crate::victron_controller::dashboard::from_0_1_0_sensors::convert__sensors__from__0_1_0(&from.sensors),
        sensors_meta: serde_json::from_value(serde_json::to_value(&from.sensors_meta).unwrap()).unwrap(),
        actuated: serde_json::from_value(serde_json::to_value(&from.actuated).unwrap()).unwrap(),
        knobs: serde_json::from_value(serde_json::to_value(&from.knobs).unwrap()).unwrap(),
        bookkeeping: serde_json::from_value(serde_json::to_value(&from.bookkeeping).unwrap()).unwrap(),
        forecasts: serde_json::from_value(serde_json::to_value(&from.forecasts).unwrap()).unwrap(),
        decisions: serde_json::from_value(serde_json::to_value(&from.decisions).unwrap()).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use crate::victron_controller::dashboard::actual_f64::ActualF64 as V020ActualF64;
    use crate::victron_controller::dashboard::freshness::Freshness;
    use crate::victron_controller::dashboard::from_0_1_0_sensors::convert__sensors__from__0_1_0;
    use crate::victron_controller::dashboard::v0_1_0::actual_f64::ActualF64 as V010ActualF64;
    use crate::victron_controller::dashboard::v0_1_0::freshness::Freshness as V010Freshness;
    use crate::victron_controller::dashboard::v0_1_0::sensors::Sensors as V010Sensors;

    /// Convert a fully-populated 0.1.0 Sensors and confirm `session_kwh`
    /// is initialised to Unknown. This is the codepath exercised by
    /// `convert__world_snapshot__from__0_1_0` (the stub above bridges
    /// `sensors` through this converter); a regression on this function
    /// would re-open PR-session-kwh-D01.
    #[test]
    fn sensors_0_1_0_converter_initialises_session_kwh_unknown() {
        let any_actual = V010ActualF64 {
            value: Some(42.0),
            freshness: V010Freshness::Fresh,
            since_epoch_ms: 1_000,
        };
        let sensors = V010Sensors {
            battery_soc: any_actual.clone(),
            battery_soh: any_actual.clone(),
            battery_installed_capacity: any_actual.clone(),
            battery_dc_power: any_actual.clone(),
            mppt_power_0: any_actual.clone(),
            mppt_power_1: any_actual.clone(),
            soltaro_power: any_actual.clone(),
            power_consumption: any_actual.clone(),
            grid_power: any_actual.clone(),
            grid_voltage: any_actual.clone(),
            grid_current: any_actual.clone(),
            consumption_current: any_actual.clone(),
            offgrid_power: any_actual.clone(),
            offgrid_current: any_actual.clone(),
            vebus_input_current: any_actual.clone(),
            evcharger_ac_power: any_actual.clone(),
            evcharger_ac_current: any_actual.clone(),
            ess_state: any_actual.clone(),
            outdoor_temperature: any_actual,
        };
        let converted = convert__sensors__from__0_1_0(&sensors);
        assert!(matches!(
            converted.session_kwh,
            V020ActualF64 {
                value: None,
                freshness: Freshness::Unknown,
                ..
            }
        ));
        assert_eq!(converted.battery_soc.value, Some(42.0));
    }
}