// @ts-nocheck
import {SetForecastDisagreementStrategy as v0_1_0_SetForecastDisagreementStrategy} from './v0_1_0/Command'
import {SetForecastDisagreementStrategy as dashboard_SetForecastDisagreementStrategy} from './Command'

export function convert__command__set_forecast_disagreement_strategy__from__0_1_0(from: v0_1_0_SetForecastDisagreementStrategy): dashboard_SetForecastDisagreementStrategy {
    return new dashboard_SetForecastDisagreementStrategy (
        JSON.parse(JSON.stringify(from.value))
    )
}