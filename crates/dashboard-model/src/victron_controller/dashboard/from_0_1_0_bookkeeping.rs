

pub fn convert__bookkeeping__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::bookkeeping::Bookkeeping) -> crate::victron_controller::dashboard::bookkeeping::Bookkeeping {
    crate::victron_controller::dashboard::bookkeeping::Bookkeeping {
        next_full_charge_iso: from.next_full_charge_iso.clone(),
        above_soc_date_iso: from.above_soc_date_iso.clone(),
        prev_ess_state: from.prev_ess_state.clone(),
        zappi_active: from.zappi_active.clone(),
        charge_to_full_required: from.charge_to_full_required.clone(),
        soc_end_of_day_target: from.soc_end_of_day_target.clone(),
        effective_export_soc_threshold: from.effective_export_soc_threshold.clone(),
        battery_selected_soc_target: from.battery_selected_soc_target.clone(),
        charge_battery_extended_today: from.charge_battery_extended_today.clone(),
        charge_battery_extended_today_date_iso: from.charge_battery_extended_today_date_iso.clone(),
    }
}