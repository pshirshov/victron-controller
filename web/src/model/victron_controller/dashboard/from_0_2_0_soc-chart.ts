// @ts-nocheck
import {SocChart as v0_2_0_SocChart} from './v0_2_0/SocChart'
import {SocChart as dashboard_SocChart} from './SocChart'

export function convert__soc_chart__from__0_2_0(from: v0_2_0_SocChart): dashboard_SocChart {
    return new dashboard_SocChart (
        JSON.parse(JSON.stringify(from.history)),
        JSON.parse(JSON.stringify(from.projection)),
        from.now_epoch_ms,
        from.now_soc_pct,
        from.discharge_target_pct,
        from.charge_target_pct
    )
}