//! The build arguments a server job run passes, leaving the job's own settings to the build.

use std::collections::BTreeMap;

use serde_json::Value as Json;
use sqlbuild_analysis::job_settings::main::setting_scope::setting_scope;
use sqlbuild_analysis::job_settings::models::SettingScope;

use crate::argv::main::job_settings_arguments::job_settings_arguments;

/// `--select job:<job>` and the job's build-wide settings as flags; the build applies the rest.
pub fn job_run_arguments(
    job: &str,
    settings: &BTreeMap<String, Json>,
) -> Result<Vec<String>, String> {
    let build_wide: BTreeMap<String, Json> = settings
        .iter()
        .filter(|(name, _)| setting_scope(name) == SettingScope::Build)
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    let mut arguments: Vec<String> = job_settings_arguments(&build_wide)?;
    arguments.extend(["--select".to_owned(), format!("job:{job}")]);
    Ok(arguments)
}
