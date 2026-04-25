// @ts-nocheck
// PR-session-kwh-D01 fix: bridge `sensors` through the hand-written
// converter so the new `session_kwh` field is initialised correctly on
// the back-compat path. The default JSON-roundtrip path leaves
// `session_kwh === undefined`, which then mis-constructs a 0.2.0 Sensors.
import {WorldSnapshot as v0_1_0_WorldSnapshot} from './v0_1_0/WorldSnapshot'
import {WorldSnapshot as dashboard_WorldSnapshot} from './WorldSnapshot'
import {convert__sensors__from__0_1_0} from './from_0_1_0_sensors'

export function convert__world_snapshot__from__0_1_0(from: v0_1_0_WorldSnapshot): dashboard_WorldSnapshot {
    return new dashboard_WorldSnapshot (
        from.captured_at_epoch_ms,
        from.captured_at_naive_iso,
        convert__sensors__from__0_1_0(from.sensors),
        JSON.parse(JSON.stringify(from.sensors_meta)),
        JSON.parse(JSON.stringify(from.actuated)),
        JSON.parse(JSON.stringify(from.knobs)),
        JSON.parse(JSON.stringify(from.bookkeeping)),
        JSON.parse(JSON.stringify(from.forecasts)),
        JSON.parse(JSON.stringify(from.decisions))
    )
}