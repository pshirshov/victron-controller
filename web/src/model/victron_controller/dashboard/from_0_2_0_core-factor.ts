// @ts-nocheck
import {CoreFactor as v0_2_0_CoreFactor} from './v0_2_0/CoreFactor'
import {CoreFactor as dashboard_CoreFactor} from './CoreFactor'

export function convert__core_factor__from__0_2_0(from: v0_2_0_CoreFactor): dashboard_CoreFactor {
    return new dashboard_CoreFactor (
        from.name,
        from.value
    )
}