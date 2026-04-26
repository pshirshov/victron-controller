// @ts-nocheck
import {CoresState as v0_2_0_CoresState} from './v0_2_0/CoresState'
import {CoresState as dashboard_CoresState} from './CoresState'

export function convert__cores_state__from__0_2_0(from: v0_2_0_CoresState): dashboard_CoresState {
    return new dashboard_CoresState (
        JSON.parse(JSON.stringify(from.cores)),
        from.topo_order
    )
}