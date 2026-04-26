// @ts-nocheck
import {Command as v0_2_0_Command, SetBoolKnob as v0_2_0_SetBoolKnob, SetFloatKnob as v0_2_0_SetFloatKnob, SetUintKnob as v0_2_0_SetUintKnob, SetDischargeTime as v0_2_0_SetDischargeTime, SetDebugFullCharge as v0_2_0_SetDebugFullCharge, SetForecastDisagreementStrategy as v0_2_0_SetForecastDisagreementStrategy, SetChargeBatteryExtendedMode as v0_2_0_SetChargeBatteryExtendedMode, SetExtendedChargeMode as v0_2_0_SetExtendedChargeMode, SetMode as v0_2_0_SetMode, SetKillSwitch as v0_2_0_SetKillSwitch, SetBookkeeping as v0_2_0_SetBookkeeping} from './v0_2_0/Command'
import {Command as dashboard_Command, SetBoolKnob as dashboard_SetBoolKnob, SetFloatKnob as dashboard_SetFloatKnob, SetUintKnob as dashboard_SetUintKnob, SetDischargeTime as dashboard_SetDischargeTime, SetDebugFullCharge as dashboard_SetDebugFullCharge, SetForecastDisagreementStrategy as dashboard_SetForecastDisagreementStrategy, SetChargeBatteryExtendedMode as dashboard_SetChargeBatteryExtendedMode, SetExtendedChargeMode as dashboard_SetExtendedChargeMode, SetMode as dashboard_SetMode, SetKillSwitch as dashboard_SetKillSwitch, SetBookkeeping as dashboard_SetBookkeeping} from './Command'

export function convert__command__from__0_2_0(from: v0_2_0_Command): dashboard_Command {
    if (from instanceof v0_2_0_SetBoolKnob) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetBoolKnob
    }
    if (from instanceof v0_2_0_SetFloatKnob) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetFloatKnob
    }
    if (from instanceof v0_2_0_SetUintKnob) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetUintKnob
    }
    if (from instanceof v0_2_0_SetDischargeTime) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetDischargeTime
    }
    if (from instanceof v0_2_0_SetDebugFullCharge) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetDebugFullCharge
    }
    if (from instanceof v0_2_0_SetForecastDisagreementStrategy) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetForecastDisagreementStrategy
    }
    if (from instanceof v0_2_0_SetChargeBatteryExtendedMode) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetChargeBatteryExtendedMode
    }
    if (from instanceof v0_2_0_SetExtendedChargeMode) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetExtendedChargeMode
    }
    if (from instanceof v0_2_0_SetMode) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetMode
    }
    if (from instanceof v0_2_0_SetKillSwitch) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetKillSwitch
    }
    if (from instanceof v0_2_0_SetBookkeeping) {
        return JSON.parse(JSON.stringify(from)) as dashboard_SetBookkeeping
    }

    throw new Error("Unknown ADT branch: " + from);
}