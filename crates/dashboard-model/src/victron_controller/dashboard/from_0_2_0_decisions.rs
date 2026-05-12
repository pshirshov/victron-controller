

pub fn convert__decisions__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::decisions::Decisions) -> crate::victron_controller::dashboard::decisions::Decisions {
    crate::victron_controller::dashboard::decisions::Decisions {
        grid_setpoint: serde_json::from_value(serde_json::to_value(&from.grid_setpoint).unwrap()).unwrap(),
        input_current_limit: serde_json::from_value(serde_json::to_value(&from.input_current_limit).unwrap()).unwrap(),
        schedule_0: serde_json::from_value(serde_json::to_value(&from.schedule_0).unwrap()).unwrap(),
        schedule_1: serde_json::from_value(serde_json::to_value(&from.schedule_1).unwrap()).unwrap(),
        zappi_mode: serde_json::from_value(serde_json::to_value(&from.zappi_mode).unwrap()).unwrap(),
        eddi_mode: serde_json::from_value(serde_json::to_value(&from.eddi_mode).unwrap()).unwrap(),
        weather_soc: serde_json::from_value(serde_json::to_value(&from.weather_soc).unwrap()).unwrap(),
        heat_pump: None,
    }
}