

pub fn convert__command__set_bookkeeping__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::command::SetBookkeeping) -> crate::victron_controller::dashboard::command::SetBookkeeping {
    crate::victron_controller::dashboard::command::SetBookkeeping {
        key: serde_json::from_value(serde_json::to_value(&from.key).unwrap()).unwrap(),
        value: serde_json::from_value(serde_json::to_value(&from.value).unwrap()).unwrap(),
    }
}