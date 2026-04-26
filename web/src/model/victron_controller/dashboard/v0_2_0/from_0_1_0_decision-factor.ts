// @ts-nocheck
import {DecisionFactor as v0_2_0_DecisionFactor} from './DecisionFactor'
import {DecisionFactor as v0_1_0_DecisionFactor} from '../v0_1_0/DecisionFactor'

export function convert__decision_factor__from__0_1_0(from: v0_1_0_DecisionFactor): v0_2_0_DecisionFactor {
    return new v0_2_0_DecisionFactor (
        from.name,
        from.value
    )
}