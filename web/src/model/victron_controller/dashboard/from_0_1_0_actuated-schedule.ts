// @ts-nocheck
import {ActuatedSchedule as v0_1_0_ActuatedSchedule} from './v0_1_0/ActuatedSchedule'
import {ActuatedSchedule as dashboard_ActuatedSchedule} from './ActuatedSchedule'

export function convert__actuated_schedule__from__0_1_0(from: v0_1_0_ActuatedSchedule): dashboard_ActuatedSchedule {
    return new dashboard_ActuatedSchedule (
        JSON.parse(JSON.stringify(from.target)),
        JSON.parse(JSON.stringify(from.target_owner)),
        JSON.parse(JSON.stringify(from.target_phase)),
        from.target_since_epoch_ms,
        JSON.parse(JSON.stringify(from.actual)),
        JSON.parse(JSON.stringify(from.actual_freshness)),
        from.actual_since_epoch_ms
    )
}