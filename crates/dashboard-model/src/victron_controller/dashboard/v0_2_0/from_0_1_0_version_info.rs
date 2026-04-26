

pub fn convert__version_info__from__0_1_0(from: &crate::victron_controller::dashboard::v0_1_0::version_info::VersionInfo) -> crate::victron_controller::dashboard::v0_2_0::version_info::VersionInfo {
    crate::victron_controller::dashboard::v0_2_0::version_info::VersionInfo {
        current_version: from.current_version.clone(),
        min_supported_version: from.min_supported_version.clone(),
        git_sha: from.git_sha.clone(),
    }
}