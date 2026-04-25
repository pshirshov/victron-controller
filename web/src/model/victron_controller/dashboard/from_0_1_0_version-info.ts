// @ts-nocheck
import {VersionInfo as v0_1_0_VersionInfo} from './v0_1_0/VersionInfo'
import {VersionInfo as dashboard_VersionInfo} from './VersionInfo'

export function convert__version_info__from__0_1_0(from: v0_1_0_VersionInfo): dashboard_VersionInfo {
    return new dashboard_VersionInfo (
        from.current_version,
        from.min_supported_version,
        from.git_sha
    )
}