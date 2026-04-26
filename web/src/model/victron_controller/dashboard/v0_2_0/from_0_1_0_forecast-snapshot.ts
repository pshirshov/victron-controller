// @ts-nocheck
import {ForecastSnapshot as v0_2_0_ForecastSnapshot} from './ForecastSnapshot'
import {ForecastSnapshot as v0_1_0_ForecastSnapshot} from '../v0_1_0/ForecastSnapshot'

export function convert__forecast_snapshot__from__0_1_0(from: v0_1_0_ForecastSnapshot): v0_2_0_ForecastSnapshot {
    return new v0_2_0_ForecastSnapshot (
        from.today_kwh,
        from.tomorrow_kwh,
        from.fetched_at_epoch_ms,
        []
    )
}