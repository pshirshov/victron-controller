

pub fn convert__command__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::command::Command) -> crate::victron_controller::dashboard::command::Command {
    match from {
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetBoolKnob(x) => crate::victron_controller::dashboard::command::Command::SetBoolKnob(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetFloatKnob(x) => crate::victron_controller::dashboard::command::Command::SetFloatKnob(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetUintKnob(x) => crate::victron_controller::dashboard::command::Command::SetUintKnob(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetDischargeTime(x) => crate::victron_controller::dashboard::command::Command::SetDischargeTime(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetDebugFullCharge(x) => crate::victron_controller::dashboard::command::Command::SetDebugFullCharge(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetForecastDisagreementStrategy(x) => crate::victron_controller::dashboard::command::Command::SetForecastDisagreementStrategy(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetChargeBatteryExtendedMode(x) => crate::victron_controller::dashboard::command::Command::SetChargeBatteryExtendedMode(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetExtendedChargeMode(x) => crate::victron_controller::dashboard::command::Command::SetExtendedChargeMode(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetMode(x) => crate::victron_controller::dashboard::command::Command::SetMode(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetKillSwitch(x) => crate::victron_controller::dashboard::command::Command::SetKillSwitch(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::command::Command::SetBookkeeping(x) => crate::victron_controller::dashboard::command::Command::SetBookkeeping(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
    }
}