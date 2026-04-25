// PR-session-kwh-D01 fix: bridge `sensors` through the hand-written
// converter so the new `session_kwh` field is initialised correctly.
// The default JSON-roundtrip path would panic with `missing field
// 'session_kwh'` because 0.2.0 Sensors does NOT carry `#[serde(default)]`.
//
// PR-tass-dag-view: also initialise the new `cores_state` to an empty
// `CoresState { cores: [], topo_order: [] }`. 0.1.0 carries no DAG view
// at all, so leave it empty until the first 0.2.0 tick repopulates it.

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
        cores_state: crate::victron_controller::dashboard::cores_state::CoresState {
            cores: Vec::new(),
            topo_order: Vec::new(),
        },
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
    use crate::victron_controller::dashboard::v0_1_0::world_snapshot::WorldSnapshot as V010WorldSnapshot;

    fn any_actual() -> V010ActualF64 {
        V010ActualF64 {
            value: Some(42.0),
            freshness: V010Freshness::Fresh,
            since_epoch_ms: 1_000,
        }
    }

    fn any_v010_sensors() -> V010Sensors {
        let a = any_actual();
        V010Sensors {
            battery_soc: a.clone(),
            battery_soh: a.clone(),
            battery_installed_capacity: a.clone(),
            battery_dc_power: a.clone(),
            mppt_power_0: a.clone(),
            mppt_power_1: a.clone(),
            soltaro_power: a.clone(),
            power_consumption: a.clone(),
            grid_power: a.clone(),
            grid_voltage: a.clone(),
            grid_current: a.clone(),
            consumption_current: a.clone(),
            offgrid_power: a.clone(),
            offgrid_current: a.clone(),
            vebus_input_current: a.clone(),
            evcharger_ac_power: a.clone(),
            evcharger_ac_current: a.clone(),
            ess_state: a.clone(),
            outdoor_temperature: a,
        }
    }

    /// Convert a fully-populated 0.1.0 Sensors and confirm `session_kwh`
    /// is initialised to Unknown. This is the codepath exercised by
    /// `convert__world_snapshot__from__0_1_0` (the stub above bridges
    /// `sensors` through this converter); a regression on this function
    /// would re-open PR-session-kwh-D01.
    #[test]
    fn sensors_0_1_0_converter_initialises_session_kwh_unknown() {
        let sensors = any_v010_sensors();
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

    /// PR-tass-dag-view: ensure the 0.1.0 -> 0.2.0 WorldSnapshot
    /// converter initialises `cores_state` to an empty CoresState
    /// (`cores: []`, `topo_order: []`). The first 0.2.0 tick repopulates
    /// it; until then the dashboard's TASS-cores section renders empty
    /// rather than crashing on a missing field.
    #[test]
    fn world_snapshot_0_1_0_converter_initialises_cores_state_empty() {
        // Build a minimal 0.1.0 snapshot via JSON to dodge the long
        // by-hand default constructor — every nested struct round-trips
        // through serde_json in the conversion stub anyway.
        let v010_json = serde_json::json!({
            "captured_at_epoch_ms": 7,
            "captured_at_naive_iso": "2026-04-25T00:00:00",
            "sensors": serde_json::to_value(any_v010_sensors()).unwrap(),
            "sensors_meta": {},
            "actuated": {
                "grid_setpoint": {
                    "target_value": null,
                    "target_owner": "Unset",
                    "target_phase": "Unset",
                    "target_since_epoch_ms": 0,
                    "actual": { "value": null, "freshness": "Unknown", "since_epoch_ms": 0 },
                },
                "input_current_limit": {
                    "target_value": null,
                    "target_owner": "Unset",
                    "target_phase": "Unset",
                    "target_since_epoch_ms": 0,
                    "actual": { "value": null, "freshness": "Unknown", "since_epoch_ms": 0 },
                },
                "zappi_mode": {
                    "target_value": null,
                    "target_owner": "Unset",
                    "target_phase": "Unset",
                    "target_since_epoch_ms": 0,
                    "actual_value": null,
                    "actual_freshness": "Unknown",
                    "actual_since_epoch_ms": 0,
                },
                "eddi_mode": {
                    "target_value": null,
                    "target_owner": "Unset",
                    "target_phase": "Unset",
                    "target_since_epoch_ms": 0,
                    "actual_value": null,
                    "actual_freshness": "Unknown",
                    "actual_since_epoch_ms": 0,
                },
                "schedule_0": {
                    "target": null,
                    "target_owner": "Unset",
                    "target_phase": "Unset",
                    "target_since_epoch_ms": 0,
                    "actual": null,
                    "actual_freshness": "Unknown",
                    "actual_since_epoch_ms": 0,
                },
                "schedule_1": {
                    "target": null,
                    "target_owner": "Unset",
                    "target_phase": "Unset",
                    "target_since_epoch_ms": 0,
                    "actual": null,
                    "actual_freshness": "Unknown",
                    "actual_since_epoch_ms": 0,
                },
            },
            "knobs": {
                "force_disable_export": false,
                "export_soc_threshold": 0.0,
                "discharge_soc_target": 0.0,
                "battery_soc_target": 0.0,
                "full_charge_discharge_soc_target": 0.0,
                "full_charge_export_soc_threshold": 0.0,
                "discharge_time": "At0200",
                "debug_full_charge": "None_",
                "pessimism_multiplier_modifier": 0.0,
                "disable_night_grid_discharge": false,
                "charge_car_boost": false,
                "charge_car_extended": false,
                "zappi_current_target": 0.0,
                "zappi_limit": 0.0,
                "zappi_emergency_margin": 0.0,
                "grid_export_limit_w": 0,
                "grid_import_limit_w": 0,
                "allow_battery_to_car": false,
                "eddi_enable_soc": 0.0,
                "eddi_disable_soc": 0.0,
                "eddi_dwell_s": 0,
                "weathersoc_winter_temperature_threshold": 0.0,
                "weathersoc_low_energy_threshold": 0.0,
                "weathersoc_ok_energy_threshold": 0.0,
                "weathersoc_high_energy_threshold": 0.0,
                "weathersoc_too_much_energy_threshold": 0.0,
                "writes_enabled": false,
                "forecast_disagreement_strategy": "Mean",
                "charge_battery_extended_mode": "Auto",
            },
            "bookkeeping": {
                "next_full_charge_iso": null,
                "above_soc_date_iso": null,
                "prev_ess_state": null,
                "zappi_active": false,
                "charge_to_full_required": false,
                "soc_end_of_day_target": 0.0,
                "effective_export_soc_threshold": 0.0,
                "battery_selected_soc_target": 0.0,
                "charge_battery_extended_today": false,
                "charge_battery_extended_today_date_iso": null,
            },
            "forecasts": { "solcast": null, "forecast_solar": null, "open_meteo": null },
            "decisions": {
                "grid_setpoint": null,
                "input_current_limit": null,
                "schedule_0": null,
                "schedule_1": null,
                "zappi_mode": null,
                "eddi_mode": null,
                "weather_soc": null,
            },
        });
        let v010: V010WorldSnapshot = serde_json::from_value(v010_json).unwrap();
        let converted = super::convert__world_snapshot__from__0_1_0(&v010);
        assert!(converted.cores_state.cores.is_empty());
        assert!(converted.cores_state.topo_order.is_empty());
        // Sanity: session_kwh still gets initialised on the bridged sensors path.
        assert!(matches!(converted.sensors.session_kwh.freshness, Freshness::Unknown));
    }
}
