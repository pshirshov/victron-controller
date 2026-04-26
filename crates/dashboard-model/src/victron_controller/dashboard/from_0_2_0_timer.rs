

pub fn convert__timer__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::timer::Timer) -> crate::victron_controller::dashboard::timer::Timer {
    crate::victron_controller::dashboard::timer::Timer {
        id: from.id.clone(),
        description: from.description.clone(),
        period_ms: from.period_ms.clone(),
        last_fire_epoch_ms: from.last_fire_epoch_ms.clone(),
        next_fire_epoch_ms: from.next_fire_epoch_ms.clone(),
        status: from.status.clone(),
    }
}