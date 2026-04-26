// @ts-nocheck
import {BookkeepingValue as v0_2_0_BookkeepingValue, NaiveDateTime as v0_2_0_NaiveDateTime, Cleared as v0_2_0_Cleared} from './v0_2_0/BookkeepingValue'
import {BookkeepingValue as dashboard_BookkeepingValue, NaiveDateTime as dashboard_NaiveDateTime, Cleared as dashboard_Cleared} from './BookkeepingValue'

export function convert__bookkeeping_value__from__0_2_0(from: v0_2_0_BookkeepingValue): dashboard_BookkeepingValue {
    if (from instanceof v0_2_0_NaiveDateTime) {
        return JSON.parse(JSON.stringify(from)) as dashboard_NaiveDateTime
    }
    if (from instanceof v0_2_0_Cleared) {
        return JSON.parse(JSON.stringify(from)) as dashboard_Cleared
    }

    throw new Error("Unknown ADT branch: " + from);
}