

pub fn convert__scheduled_action__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::scheduled_action::ScheduledAction) -> crate::victron_controller::dashboard::scheduled_action::ScheduledAction {
    crate::victron_controller::dashboard::scheduled_action::ScheduledAction {
        label: from.label.clone(),
        source: from.source.clone(),
        next_fire_epoch_ms: from.next_fire_epoch_ms.clone(),
        period_ms: from.period_ms.clone(),
    }
}