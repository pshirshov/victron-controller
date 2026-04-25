

pub fn convert__schedule_spec__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::schedule_spec::ScheduleSpec) -> crate::victron_controller::dashboard::schedule_spec::ScheduleSpec {
    crate::victron_controller::dashboard::schedule_spec::ScheduleSpec {
        start_s: from.start_s.clone(),
        duration_s: from.duration_s.clone(),
        discharge: from.discharge.clone(),
        soc: from.soc.clone(),
        days: from.days.clone(),
    }
}