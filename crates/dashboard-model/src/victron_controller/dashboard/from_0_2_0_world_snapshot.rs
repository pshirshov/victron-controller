

pub fn convert__world_snapshot__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::world_snapshot::WorldSnapshot) -> crate::victron_controller::dashboard::world_snapshot::WorldSnapshot {
    crate::victron_controller::dashboard::world_snapshot::WorldSnapshot {
        captured_at_epoch_ms: from.captured_at_epoch_ms.clone(),
        captured_at_naive_iso: from.captured_at_naive_iso.clone(),
        sensors: serde_json::from_value(serde_json::to_value(&from.sensors).unwrap()).unwrap(),
        sensors_meta: serde_json::from_value(serde_json::to_value(&from.sensors_meta).unwrap()).unwrap(),
        actuated: serde_json::from_value(serde_json::to_value(&from.actuated).unwrap()).unwrap(),
        knobs: serde_json::from_value(serde_json::to_value(&from.knobs).unwrap()).unwrap(),
        bookkeeping: serde_json::from_value(serde_json::to_value(&from.bookkeeping).unwrap()).unwrap(),
        forecasts: serde_json::from_value(serde_json::to_value(&from.forecasts).unwrap()).unwrap(),
        decisions: serde_json::from_value(serde_json::to_value(&from.decisions).unwrap()).unwrap(),
        cores_state: serde_json::from_value(serde_json::to_value(&from.cores_state).unwrap()).unwrap(),
        timers: serde_json::from_value(serde_json::to_value(&from.timers).unwrap()).unwrap(),
        timezone: from.timezone.clone(),
        soc_chart: serde_json::from_value(serde_json::to_value(&from.soc_chart).unwrap()).unwrap(),
        scheduled_actions: serde_json::from_value(serde_json::to_value(&from.scheduled_actions).unwrap()).unwrap(),
        pinned_registers: Vec::new(),
    }
}