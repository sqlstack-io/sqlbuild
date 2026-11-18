//! How far one job setting reaches.

use crate::job_settings::constants::{JOB_LIMIT_SETTING, NODE_SETTINGS};
use crate::job_settings::models::SettingScope;

/// A node setting, the job limit, or else a build-wide setting.
#[must_use]
pub fn setting_scope(name: &str) -> SettingScope {
    if NODE_SETTINGS.contains(&name) {
        SettingScope::Node
    } else if name == JOB_LIMIT_SETTING {
        SettingScope::JobLimit
    } else {
        SettingScope::Build
    }
}
