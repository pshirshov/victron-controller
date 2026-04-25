// @ts-nocheck
import {Forecasts as v0_1_0_Forecasts} from './v0_1_0/Forecasts'
import {Forecasts as dashboard_Forecasts} from './Forecasts'

export function convert__forecasts__from__0_1_0(from: v0_1_0_Forecasts): dashboard_Forecasts {
    return new dashboard_Forecasts (
        JSON.parse(JSON.stringify(from.solcast)),
        JSON.parse(JSON.stringify(from.forecast_solar)),
        JSON.parse(JSON.stringify(from.open_meteo))
    )
}