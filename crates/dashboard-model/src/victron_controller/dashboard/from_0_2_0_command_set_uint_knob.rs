

pub fn convert__command__set_uint_knob__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::command::SetUintKnob) -> crate::victron_controller::dashboard::command::SetUintKnob {
    crate::victron_controller::dashboard::command::SetUintKnob {
        knob_name: from.knob_name.clone(),
        value: from.value.clone(),
    }
}