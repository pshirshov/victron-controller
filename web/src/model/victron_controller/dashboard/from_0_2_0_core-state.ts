// @ts-nocheck
import {CoreState as v0_2_0_CoreState} from './v0_2_0/CoreState'
import {CoreState as dashboard_CoreState} from './CoreState'

export function convert__core_state__from__0_2_0(from: v0_2_0_CoreState): dashboard_CoreState {
    return new dashboard_CoreState (
        from.id,
        from.depends_on,
        from.last_run_outcome,
        from.last_payload,
        JSON.parse(JSON.stringify(from.last_inputs)),
        JSON.parse(JSON.stringify(from.last_outputs))
    )
}