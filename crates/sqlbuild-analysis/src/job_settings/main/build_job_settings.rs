//! The build-wide settings of the named jobs, which must all agree.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::job_settings::constants::CONFLICT_HELP;
use crate::job_settings::errors::JobSettingConflict;
use crate::job_settings::main::setting_scope::setting_scope;
use crate::job_settings::models::{DeclaredRun, SettingScope};

fn settings_of<'a>(
    job: &str,
    declared: &'a BTreeMap<String, DeclaredRun>,
) -> Option<&'a BTreeMap<String, Json>> {
    match declared.get(job) {
        Some(DeclaredRun::Job { settings, .. }) => Some(settings),
        _ => None,
    }
}

fn shown(value: Option<&Json>) -> String {
    value.map_or_else(|| "not set".to_owned(), Json::to_string)
}

/// Each build-wide setting the named jobs give the build; a job that leaves one unset differs from one that sets it.
pub fn build_job_settings(
    named: &[String],
    declared: &BTreeMap<String, DeclaredRun>,
) -> Result<BTreeMap<String, Json>, JobSettingConflict> {
    let mut keys: Vec<&String> = named
        .iter()
        .filter_map(|job| settings_of(job, declared))
        .flat_map(BTreeMap::keys)
        .filter(|key| setting_scope(key) == SettingScope::Build)
        .collect();
    keys.sort_unstable();
    keys.dedup();
    let mut agreed: BTreeMap<String, Json> = BTreeMap::new();
    for key in keys {
        let mut first: Option<(&String, Option<&Json>)> = None;
        for job in named {
            let value: Option<&Json> =
                settings_of(job, declared).and_then(|settings| settings.get(key));
            match first {
                None => first = Some((job, value)),
                Some((other, seen)) if seen != value => {
                    return Err(JobSettingConflict {
                        message: format!(
                            "jobs {other} and {job} set {key} differently ({} and {}); a build \
                             applies {key} to everything it runs",
                            shown(seen),
                            shown(value)
                        ),
                        help: CONFLICT_HELP,
                    });
                }
                Some(_) => {}
            }
        }
        if let Some((_, Some(value))) = first {
            agreed.insert(key.clone(), value.clone());
        }
    }
    Ok(agreed)
}
