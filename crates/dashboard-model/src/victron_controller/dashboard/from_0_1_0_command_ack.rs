

pub fn convert__command_ack__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::command_ack::CommandAck) -> crate::victron_controller::dashboard::command_ack::CommandAck {
    crate::victron_controller::dashboard::command_ack::CommandAck {
        accepted: from.accepted.clone(),
        error_message: from.error_message.clone(),
    }
}