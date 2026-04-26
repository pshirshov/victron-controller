// @ts-nocheck
import {ScheduledActions as v0_2_0_ScheduledActions} from './v0_2_0/ScheduledActions'
import {ScheduledActions as dashboard_ScheduledActions} from './ScheduledActions'

export function convert__scheduled_actions__from__0_2_0(from: v0_2_0_ScheduledActions): dashboard_ScheduledActions {
    return new dashboard_ScheduledActions (
        JSON.parse(JSON.stringify(from.entries))
    )
}