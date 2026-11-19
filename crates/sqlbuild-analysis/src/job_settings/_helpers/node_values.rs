//! The node-level settings and retry one node takes from the jobs it is selected through.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::assembly::project::types::ObjectKey;
use crate::definition_changes::constants::PYTHON_NODE_KINDS;
use crate::job_settings::constants::{JOB_SETTINGS, RETRY_SETTING};
use crate::job_settings::errors::JobSettingConflict;
use crate::job_settings::main::conflict_help::conflict_help;
use crate::job_settings::models::{DeclaredRun, JobRetry, SettingScope};

/// One node's settings, and its retry, each from whichever of its jobs sets it.
pub(crate) type NodeValues = (BTreeMap<String, Json>, Option<JobRetry>);

fn conflict(
    key: &ObjectKey,
    (first, second): (&str, &str),
    setting: &str,
    values: (String, String),
) -> JobSettingConflict {
    JobSettingConflict {
        message: format!(
            "{}:{} is selected through jobs {first} and {second}, which set {setting} differently \
             ({} and {})",
            key.0, key.1, values.0, values.1
        ),
        help: conflict_help(setting),
    }
}

fn shown(retry: JobRetry) -> String {
    format!(
        "{} attempts, {}s apart",
        retry.attempts, retry.delay_seconds
    )
}

/// The settings `jobs` give `key`; two jobs giving it different values of one setting conflict.
pub(crate) fn node_values(
    key: &ObjectKey,
    jobs: &[String],
    declared: &BTreeMap<String, DeclaredRun>,
) -> Result<NodeValues, JobSettingConflict> {
    let python: bool = PYTHON_NODE_KINDS.contains(&key.0.as_str());
    let node_settings: Vec<&str> = JOB_SETTINGS
        .iter()
        .filter(|spec| spec.scope == SettingScope::Node && (spec.python_nodes || !python))
        .map(|spec| spec.name)
        .collect();
    let mut values: BTreeMap<String, (&String, &Json)> = BTreeMap::new();
    let mut retry: Option<(&String, JobRetry)> = None;
    for job in jobs {
        let Some(DeclaredRun::Job {
            settings,
            retry: job_retry,
        }) = declared.get(job)
        else {
            continue;
        };
        for &setting in &node_settings {
            let Some(value) = settings.get(setting) else {
                continue;
            };
            match values.get(setting) {
                Some((other, seen)) if *seen != value => {
                    let shown_values = (seen.to_string(), value.to_string());
                    return Err(conflict(key, (other, job), setting, shown_values));
                }
                Some(_) => {}
                None => {
                    values.insert(setting.to_owned(), (job, value));
                }
            }
        }
        match (retry, job_retry.as_ref().filter(|_| !python)) {
            (Some((other, seen)), Some(own)) if seen != *own => {
                return Err(conflict(
                    key,
                    (other, job),
                    RETRY_SETTING,
                    (shown(seen), shown(*own)),
                ));
            }
            (None, Some(own)) => retry = Some((job, *own)),
            _ => {}
        }
    }
    let values: BTreeMap<String, Json> = values
        .into_iter()
        .map(|(name, (_, value))| (name, value.clone()))
        .collect();
    Ok((values, retry.map(|(_, retry)| retry)))
}
