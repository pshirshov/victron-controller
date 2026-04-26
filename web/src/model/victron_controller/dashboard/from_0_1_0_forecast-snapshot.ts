// @ts-nocheck
import {ForecastSnapshot as v0_1_0_ForecastSnapshot} from './v0_1_0/ForecastSnapshot'
import {ForecastSnapshot as dashboard_ForecastSnapshot} from './ForecastSnapshot'

export function convert__forecast_snapshot__from__0_1_0(from: v0_1_0_ForecastSnapshot): dashboard_ForecastSnapshot {
    return new dashboard_ForecastSnapshot (
        from.today_kwh,
        from.tomorrow_kwh,
        from.fetched_at_epoch_ms,
        []
    )
}