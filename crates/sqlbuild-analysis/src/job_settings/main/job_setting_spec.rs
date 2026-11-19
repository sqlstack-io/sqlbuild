//! The table row of one job setting.

use crate::job_settings::constants::JOB_SETTINGS;
use crate::job_settings::models::JobSettingSpec;

/// The row of setting `name`; `None` for a name no build option has.
#[must_use]
pub fn job_setting_spec(name: &str) -> Option<&'static JobSettingSpec> {
    JOB_SETTINGS.iter().find(|spec| spec.name == name)
}
