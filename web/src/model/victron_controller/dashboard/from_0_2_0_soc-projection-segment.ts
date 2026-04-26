// @ts-nocheck
import {SocProjectionSegment as v0_2_0_SocProjectionSegment} from './v0_2_0/SocProjectionSegment'
import {SocProjectionSegment as dashboard_SocProjectionSegment} from './SocProjectionSegment'

export function convert__soc_projection_segment__from__0_2_0(from: v0_2_0_SocProjectionSegment): dashboard_SocProjectionSegment {
    return new dashboard_SocProjectionSegment (
        from.start_epoch_ms,
        from.end_epoch_ms,
        from.start_soc_pct,
        from.end_soc_pct,
        JSON.parse(JSON.stringify(from.kind))
    )
}