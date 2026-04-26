

pub fn convert__sensor_meta__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::sensor_meta::SensorMeta) -> crate::victron_controller::dashboard::v0_2_0::sensor_meta::SensorMeta {
    crate::victron_controller::dashboard::v0_2_0::sensor_meta::SensorMeta {
        origin: from.origin.clone(),
        identifier: from.identifier.clone(),
        cadence_ms: from.cadence_ms.clone(),
        staleness_ms: from.staleness_ms.clone(),
    }
}