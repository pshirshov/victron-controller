

pub fn convert__timers__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::timers::Timers) -> crate::victron_controller::dashboard::timers::Timers {
    crate::victron_controller::dashboard::timers::Timers {
        entries: serde_json::from_value(serde_json::to_value(&from.entries).unwrap()).unwrap(),
    }
}