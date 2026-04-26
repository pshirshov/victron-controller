// @ts-nocheck
import {SocProjection as v0_2_0_SocProjection} from './v0_2_0/SocProjection'
import {SocProjection as dashboard_SocProjection} from './SocProjection'

export function convert__soc_projection__from__0_2_0(from: v0_2_0_SocProjection): dashboard_SocProjection {
    return new dashboard_SocProjection (
        JSON.parse(JSON.stringify(from.segments)),
        from.net_power_w,
        from.capacity_wh,
        from.charge_rate_w
    )
}