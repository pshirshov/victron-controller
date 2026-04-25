

pub fn convert__command__set_bool_knob__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::command::SetBoolKnob) -> crate::victron_controller::dashboard::command::SetBoolKnob {
    crate::victron_controller::dashboard::command::SetBoolKnob {
        knob_name: from.knob_name.clone(),
        value: from.value.clone(),
    }
}