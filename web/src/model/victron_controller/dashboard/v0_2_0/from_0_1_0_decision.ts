// @ts-nocheck
import {Decision as v0_1_0_Decision} from '../v0_1_0/Decision'
import {Decision as v0_2_0_Decision} from './Decision'

export function convert__decision__from__0_1_0(from: v0_1_0_Decision): v0_2_0_Decision {
    return new v0_2_0_Decision (
        from.summary,
        JSON.parse(JSON.stringify(from.factors))
    )
}