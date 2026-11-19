//! The build arguments a server job run passes, leaving the job's own settings to the build.

use std::collections::BTreeMap;

use serde_json::Value as Json;
use sqlbuild_analysis::job_settings::main::job_setting_spec::job_setting_spec;
use sqlbuild_analysis::job_settings::models::SettingScope;

use crate::argv::main::job_settings_arguments::job_settings_arguments;

/// `--select job:<job>` and its build-wide settings and warehouse, the run's own, as flags.
pub fn job_run_arguments(
    job: &str,
    settings: &BTreeMap<String, Json>,
) -> Result<Vec<String>, String> {
    let build_wide: BTreeMap<String, Json> = settings
        .iter()
        .filter(|(name, _)| {
            job_setting_spec(name)
                .is_some_and(|spec| spec.scope == SettingScope::Build || spec.whole_run)
        })
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    let mut arguments: Vec<String> = job_settings_arguments(&build_wide)?;
    arguments.extend(["--select".to_owned(), format!("job:{job}")]);
    Ok(arguments)
}
