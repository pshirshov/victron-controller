// @ts-nocheck
import {NaiveDateTime as v0_2_0_NaiveDateTime} from './v0_2_0/BookkeepingValue'
import {NaiveDateTime as dashboard_NaiveDateTime} from './BookkeepingValue'

export function convert__bookkeeping_value__naive_date_time__from__0_2_0(from: v0_2_0_NaiveDateTime): dashboard_NaiveDateTime {
    return new dashboard_NaiveDateTime (
        from.iso
    )
}