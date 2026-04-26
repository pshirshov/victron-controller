// @ts-nocheck
import {Decision as v0_2_0_Decision} from './v0_2_0/Decision'
import {Decision as dashboard_Decision} from './Decision'

export function convert__decision__from__0_2_0(from: v0_2_0_Decision): dashboard_Decision {
    return new dashboard_Decision (
        from.summary,
        JSON.parse(JSON.stringify(from.factors))
    )
}