// @ts-nocheck
import {SensorMeta as v0_1_0_SensorMeta} from './v0_1_0/SensorMeta'
import {SensorMeta as dashboard_SensorMeta} from './SensorMeta'

export function convert__sensor_meta__from__0_1_0(from: v0_1_0_SensorMeta): dashboard_SensorMeta {
    return new dashboard_SensorMeta (
        from.origin,
        from.identifier,
        from.cadence_ms,
        from.staleness_ms
    )
}