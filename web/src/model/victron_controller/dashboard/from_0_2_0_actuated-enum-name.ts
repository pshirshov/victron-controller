// @ts-nocheck
import {ActuatedEnumName as v0_2_0_ActuatedEnumName} from './v0_2_0/ActuatedEnumName'
import {ActuatedEnumName as dashboard_ActuatedEnumName} from './ActuatedEnumName'

export function convert__actuated_enum_name__from__0_2_0(from: v0_2_0_ActuatedEnumName): dashboard_ActuatedEnumName {
    return new dashboard_ActuatedEnumName (
        from.target_value,
        JSON.parse(JSON.stringify(from.target_owner)),
        JSON.parse(JSON.stringify(from.target_phase)),
        from.target_since_epoch_ms,
        from.actual_value,
        JSON.parse(JSON.stringify(from.actual_freshness)),
        from.actual_since_epoch_ms
    )
}