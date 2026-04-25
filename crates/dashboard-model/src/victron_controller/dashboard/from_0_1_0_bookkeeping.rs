// PR-gamma-hold-redesign: 0.1.0 → 0.2.0 manual converter for
// `Bookkeeping`. 0.2.0 adds four `weather_soc_*` slots. Initialise with
// the safe-default values that match `Knobs::safe_defaults` (80 / 80 /
// 80 / false). The first 0.2.0 tick rewrites these unless weather_soc
// fails to evaluate (no fresh forecast) — in which case the safe
// defaults remain a sensible fallback.

pub fn convert__bookkeeping__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::bookkeeping::Bookkeeping) -> crate::victron_controller::dashboard::bookkeeping::Bookkeeping {
    crate::victron_controller::dashboard::bookkeeping::Bookkeeping {
        next_full_charge_iso: from.next_full_charge_iso.clone(),
        above_soc_date_iso: from.above_soc_date_iso.clone(),
        prev_ess_state: from.prev_ess_state,
        zappi_active: from.zappi_active,
        charge_to_full_required: from.charge_to_full_required,
        soc_end_of_day_target: from.soc_end_of_day_target,
        effective_export_soc_threshold: from.effective_export_soc_threshold,
        battery_selected_soc_target: from.battery_selected_soc_target,
        charge_battery_extended_today: from.charge_battery_extended_today,
        charge_battery_extended_today_date_iso: from.charge_battery_extended_today_date_iso.clone(),
        // PR-gamma-hold-redesign — initialise the four weather_soc slots
        // with the corresponding safe-default values from
        // `Knobs::safe_defaults` (SPEC §7).
        weather_soc_export_soc_threshold: 80.0,
        weather_soc_discharge_soc_target: 80.0,
        weather_soc_battery_soc_target: 80.0,
        weather_soc_disable_night_grid_discharge: false,
    }
}
