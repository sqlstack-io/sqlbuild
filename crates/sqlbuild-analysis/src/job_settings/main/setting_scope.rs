//! How far one job setting reaches.

use crate::job_settings::main::job_setting_spec::job_setting_spec;
use crate::job_settings::models::SettingScope;

/// The setting's scope from the table; build-wide for a name the parser then rejects.
#[must_use]
pub fn setting_scope(name: &str) -> SettingScope {
    job_setting_spec(name).map_or(SettingScope::Build, |spec| spec.scope)
}
