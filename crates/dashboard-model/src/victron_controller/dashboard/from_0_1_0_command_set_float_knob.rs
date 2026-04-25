

pub fn convert__command__set_float_knob__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::command::SetFloatKnob) -> crate::victron_controller::dashboard::command::SetFloatKnob {
    crate::victron_controller::dashboard::command::SetFloatKnob {
        knob_name: from.knob_name.clone(),
        value: from.value.clone(),
    }
}