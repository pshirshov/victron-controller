// @ts-nocheck
import {Actuated as v0_1_0_Actuated} from '../v0_1_0/Actuated'
import {Actuated as v0_2_0_Actuated} from './Actuated'

export function convert__actuated__from__0_1_0(from: v0_1_0_Actuated): v0_2_0_Actuated {
    return new v0_2_0_Actuated (
        JSON.parse(JSON.stringify(from.grid_setpoint)),
        JSON.parse(JSON.stringify(from.input_current_limit)),
        JSON.parse(JSON.stringify(from.zappi_mode)),
        JSON.parse(JSON.stringify(from.eddi_mode)),
        JSON.parse(JSON.stringify(from.schedule_0)),
        JSON.parse(JSON.stringify(from.schedule_1))
    )
}