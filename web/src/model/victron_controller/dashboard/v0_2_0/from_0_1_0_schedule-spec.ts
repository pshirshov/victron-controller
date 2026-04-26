// @ts-nocheck
import {ScheduleSpec as v0_1_0_ScheduleSpec} from '../v0_1_0/ScheduleSpec'
import {ScheduleSpec as v0_2_0_ScheduleSpec} from './ScheduleSpec'

export function convert__schedule_spec__from__0_1_0(from: v0_1_0_ScheduleSpec): v0_2_0_ScheduleSpec {
    return new v0_2_0_ScheduleSpec (
        from.start_s,
        from.duration_s,
        from.discharge,
        from.soc,
        from.days
    )
}