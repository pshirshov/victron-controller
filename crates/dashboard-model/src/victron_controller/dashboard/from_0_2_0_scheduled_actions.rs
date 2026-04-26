

pub fn convert__scheduled_actions__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::scheduled_actions::ScheduledActions) -> crate::victron_controller::dashboard::scheduled_actions::ScheduledActions {
    crate::victron_controller::dashboard::scheduled_actions::ScheduledActions {
        entries: serde_json::from_value(serde_json::to_value(&from.entries).unwrap()).unwrap(),
    }
}