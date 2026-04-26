// @ts-nocheck
import {DecisionFactor as v0_2_0_DecisionFactor} from './v0_2_0/DecisionFactor'
import {DecisionFactor as dashboard_DecisionFactor} from './DecisionFactor'

export function convert__decision_factor__from__0_2_0(from: v0_2_0_DecisionFactor): dashboard_DecisionFactor {
    return new dashboard_DecisionFactor (
        from.name,
        from.value
    )
}