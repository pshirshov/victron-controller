// @ts-nocheck
import {SocHistorySample as v0_2_0_SocHistorySample} from './v0_2_0/SocHistorySample'
import {SocHistorySample as dashboard_SocHistorySample} from './SocHistorySample'

export function convert__soc_history_sample__from__0_2_0(from: v0_2_0_SocHistorySample): dashboard_SocHistorySample {
    return new dashboard_SocHistorySample (
        from.epoch_ms,
        from.soc_pct
    )
}