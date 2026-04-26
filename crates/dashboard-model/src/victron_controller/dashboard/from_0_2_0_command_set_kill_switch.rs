

pub fn convert__command__set_kill_switch__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::command::SetKillSwitch) -> crate::victron_controller::dashboard::command::SetKillSwitch {
    crate::victron_controller::dashboard::command::SetKillSwitch {
        value: from.value.clone(),
    }
}