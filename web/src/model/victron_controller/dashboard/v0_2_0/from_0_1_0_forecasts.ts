// @ts-nocheck
import {Forecasts as v0_1_0_Forecasts} from '../v0_1_0/Forecasts'
import {Forecasts as v0_2_0_Forecasts} from './Forecasts'

export function convert__forecasts__from__0_1_0(from: v0_1_0_Forecasts): v0_2_0_Forecasts {
    return new v0_2_0_Forecasts (
        JSON.parse(JSON.stringify(from.solcast)),
        JSON.parse(JSON.stringify(from.forecast_solar)),
        JSON.parse(JSON.stringify(from.open_meteo))
    )
}