// @ts-nocheck
import {Decisions as v0_1_0_Decisions} from './v0_1_0/Decisions'
import {Decisions as dashboard_Decisions} from './Decisions'

export function convert__decisions__from__0_1_0(from: v0_1_0_Decisions): dashboard_Decisions {
    return new dashboard_Decisions (
        JSON.parse(JSON.stringify(from.grid_setpoint)),
        JSON.parse(JSON.stringify(from.input_current_limit)),
        JSON.parse(JSON.stringify(from.schedule_0)),
        JSON.parse(JSON.stringify(from.schedule_1)),
        JSON.parse(JSON.stringify(from.zappi_mode)),
        JSON.parse(JSON.stringify(from.eddi_mode)),
        JSON.parse(JSON.stringify(from.weather_soc))
    )
}