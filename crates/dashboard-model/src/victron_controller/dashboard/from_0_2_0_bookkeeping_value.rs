

pub fn convert__bookkeeping_value__from__0_2_0(from: &crate::victron_controller::dashboard::v0_2_0::bookkeeping_value::BookkeepingValue) -> crate::victron_controller::dashboard::bookkeeping_value::BookkeepingValue {
    match from {
        crate::victron_controller::dashboard::v0_2_0::bookkeeping_value::BookkeepingValue::NaiveDateTime(x) => crate::victron_controller::dashboard::bookkeeping_value::BookkeepingValue::NaiveDateTime(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
        crate::victron_controller::dashboard::v0_2_0::bookkeeping_value::BookkeepingValue::Cleared(x) => crate::victron_controller::dashboard::bookkeeping_value::BookkeepingValue::Cleared(serde_json::from_value(serde_json::to_value(x).unwrap()).unwrap()),
    }
}