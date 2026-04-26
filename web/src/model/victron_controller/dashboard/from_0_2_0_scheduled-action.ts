// @ts-nocheck
import {ScheduledAction as v0_2_0_ScheduledAction} from './v0_2_0/ScheduledAction'
import {ScheduledAction as dashboard_ScheduledAction} from './ScheduledAction'

export function convert__scheduled_action__from__0_2_0(from: v0_2_0_ScheduledAction): dashboard_ScheduledAction {
    return new dashboard_ScheduledAction (
        from.label,
        from.source,
        from.next_fire_epoch_ms,
        from.period_ms
    )
}