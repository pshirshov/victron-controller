// @ts-nocheck
import {Command as v0_1_0_Command, SetBoolKnob as v0_1_0_SetBoolKnob, SetFloatKnob as v0_1_0_SetFloatKnob, SetUintKnob as v0_1_0_SetUintKnob, SetDischargeTime as v0_1_0_SetDischargeTime, SetDebugFullCharge as v0_1_0_SetDebugFullCharge, SetForecastDisagreementStrategy as v0_1_0_SetForecastDisagreementStrategy, SetChargeBatteryExtendedMode as v0_1_0_SetChargeBatteryExtendedMode, SetKillSwitch as v0_1_0_SetKillSwitch} from '../v0_1_0/Command'
import {Command as v0_2_0_Command, SetBoolKnob as v0_2_0_SetBoolKnob, SetFloatKnob as v0_2_0_SetFloatKnob, SetUintKnob as v0_2_0_SetUintKnob, SetDischargeTime as v0_2_0_SetDischargeTime, SetDebugFullCharge as v0_2_0_SetDebugFullCharge, SetForecastDisagreementStrategy as v0_2_0_SetForecastDisagreementStrategy, SetChargeBatteryExtendedMode as v0_2_0_SetChargeBatteryExtendedMode, SetKillSwitch as v0_2_0_SetKillSwitch} from './Command'

export function convert__command__from__0_1_0(from: v0_1_0_Command): v0_2_0_Command {
    if (from instanceof v0_1_0_SetBoolKnob) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_SetBoolKnob
    }
    if (from instanceof v0_1_0_SetFloatKnob) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_SetFloatKnob
    }
    if (from instanceof v0_1_0_SetUintKnob) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_SetUintKnob
    }
    if (from instanceof v0_1_0_SetDischargeTime) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_SetDischargeTime
    }
    if (from instanceof v0_1_0_SetDebugFullCharge) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_SetDebugFullCharge
    }
    if (from instanceof v0_1_0_SetForecastDisagreementStrategy) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_SetForecastDisagreementStrategy
    }
    if (from instanceof v0_1_0_SetChargeBatteryExtendedMode) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_SetChargeBatteryExtendedMode
    }
    if (from instanceof v0_1_0_SetKillSwitch) {
        return JSON.parse(JSON.stringify(from)) as v0_2_0_SetKillSwitch
    }

    throw new Error("Unknown ADT branch: " + from);
}