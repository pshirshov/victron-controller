// @ts-nocheck
import {Bookkeeping as v0_1_0_Bookkeeping} from './v0_1_0/Bookkeeping'
import {Bookkeeping as dashboard_Bookkeeping} from './Bookkeeping'

export function convert__bookkeeping__from__0_1_0(from: v0_1_0_Bookkeeping): dashboard_Bookkeeping {
    return new dashboard_Bookkeeping (
        from.next_full_charge_iso,
        from.above_soc_date_iso,
        from.prev_ess_state,
        from.zappi_active,
        from.charge_to_full_required,
        from.soc_end_of_day_target,
        from.effective_export_soc_threshold,
        from.battery_selected_soc_target,
        from.charge_battery_extended_today,
        from.charge_battery_extended_today_date_iso
    )
}