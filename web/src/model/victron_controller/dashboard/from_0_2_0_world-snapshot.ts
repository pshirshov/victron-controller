// @ts-nocheck
import {WorldSnapshot as v0_2_0_WorldSnapshot} from './v0_2_0/WorldSnapshot'
import {WorldSnapshot as dashboard_WorldSnapshot} from './WorldSnapshot'

export function convert__world_snapshot__from__0_2_0(from: v0_2_0_WorldSnapshot): dashboard_WorldSnapshot {
    return new dashboard_WorldSnapshot (
        from.captured_at_epoch_ms,
        from.captured_at_naive_iso,
        JSON.parse(JSON.stringify(from.sensors)),
        JSON.parse(JSON.stringify(from.sensors_meta)),
        JSON.parse(JSON.stringify(from.actuated)),
        JSON.parse(JSON.stringify(from.knobs)),
        JSON.parse(JSON.stringify(from.bookkeeping)),
        JSON.parse(JSON.stringify(from.forecasts)),
        JSON.parse(JSON.stringify(from.decisions)),
        JSON.parse(JSON.stringify(from.cores_state)),
        JSON.parse(JSON.stringify(from.timers)),
        from.timezone,
        JSON.parse(JSON.stringify(from.soc_chart)),
        JSON.parse(JSON.stringify(from.scheduled_actions)),
        [],
        undefined,
        undefined
    )
}