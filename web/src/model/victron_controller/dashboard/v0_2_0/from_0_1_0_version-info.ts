// @ts-nocheck
import {VersionInfo as v0_2_0_VersionInfo} from './VersionInfo'
import {VersionInfo as v0_1_0_VersionInfo} from '../v0_1_0/VersionInfo'

export function convert__version_info__from__0_1_0(from: v0_1_0_VersionInfo): v0_2_0_VersionInfo {
    return new v0_2_0_VersionInfo (
        from.current_version,
        from.min_supported_version,
        from.git_sha
    )
}