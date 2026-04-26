// @ts-nocheck
import {Timer as v0_2_0_Timer} from './v0_2_0/Timer'
import {Timer as dashboard_Timer} from './Timer'

export function convert__timer__from__0_2_0(from: v0_2_0_Timer): dashboard_Timer {
    return new dashboard_Timer (
        from.id,
        from.description,
        from.period_ms,
        from.last_fire_epoch_ms,
        from.next_fire_epoch_ms,
        from.status
    )
}