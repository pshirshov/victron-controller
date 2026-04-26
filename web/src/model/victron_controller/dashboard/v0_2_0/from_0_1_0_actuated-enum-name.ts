// @ts-nocheck
import {ActuatedEnumName as v0_2_0_ActuatedEnumName} from './ActuatedEnumName'
import {ActuatedEnumName as v0_1_0_ActuatedEnumName} from '../v0_1_0/ActuatedEnumName'

export function convert__actuated_enum_name__from__0_1_0(from: v0_1_0_ActuatedEnumName): v0_2_0_ActuatedEnumName {
    return new v0_2_0_ActuatedEnumName (
        from.target_value,
        JSON.parse(JSON.stringify(from.target_owner)),
        JSON.parse(JSON.stringify(from.target_phase)),
        from.target_since_epoch_ms,
        from.actual_value,
        JSON.parse(JSON.stringify(from.actual_freshness)),
        from.actual_since_epoch_ms
    )
}