// @ts-nocheck
import {SetDischargeTime as v0_1_0_SetDischargeTime} from './v0_1_0/Command'
import {SetDischargeTime as dashboard_SetDischargeTime} from './Command'

export function convert__command__set_discharge_time__from__0_1_0(from: v0_1_0_SetDischargeTime): dashboard_SetDischargeTime {
    return new dashboard_SetDischargeTime (
        JSON.parse(JSON.stringify(from.value))
    )
}