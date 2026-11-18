//! The settings each selected node takes from the jobs it is selected through.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::assembly::project::types::ObjectKey;
use crate::graph::_helpers::selectors::{Keys, tokens};
use crate::graph::models::ProjectGraph;
use crate::job_settings::constants::{CONFLICT_HELP, JOB_LIMIT_SETTING, NODE_SETTINGS};
use crate::job_settings::errors::{JobSettingConflict, NodeJobSettingsError};
use crate::job_settings::main::named_jobs::token_jobs;
use crate::job_settings::models::{DeclaredRun, JobRetry, NodeJobSettings};

fn conflict(
    key: &ObjectKey,
    (first, second): (&str, &str),
    setting: &str,
    values: (String, String),
) -> NodeJobSettingsError {
    NodeJobSettingsError::Conflict(JobSettingConflict {
        message: format!(
            "{}:{} is selected through jobs {first} and {second}, which set {setting} differently \
             ({} and {})",
            key.0, key.1, values.0, values.1
        ),
        help: CONFLICT_HELP,
    })
}

/// A node is selected through a job when a `--select` token names the job, selects the node, and
/// the job's own selection holds it; `+` around a job and intersections never widen what it covers.
pub fn node_job_settings(
    graph: &ProjectGraph,
    (select, exclude): (&[String], &[String]),
    declared: &BTreeMap<String, DeclaredRun>,
) -> Result<NodeJobSettings, NodeJobSettingsError> {
    let mut result = NodeJobSettings::default();
    let excluded: Keys = tokens(graph, exclude).map_err(NodeJobSettingsError::Selector)?;
    let resolved = graph.jobs.as_ref();
    for token in select
        .iter()
        .flat_map(|selector| selector.split_whitespace())
    {
        let jobs: Vec<String> = token_jobs(token, declared);
        if jobs.is_empty() {
            continue;
        }
        let selected: Keys =
            tokens(graph, &[token.to_owned()]).map_err(NodeJobSettingsError::Selector)?;
        for job in jobs {
            let Some(Ok(own)) = resolved.and_then(|resolved| resolved.get(&job)) else {
                continue;
            };
            for key in selected
                .intersection(own)
                .filter(|key| !excluded.contains(*key))
            {
                let jobs = result.node_jobs.entry(key.clone()).or_default();
                if !jobs.contains(&job) {
                    jobs.push(job.clone());
                }
            }
        }
    }
    for (key, jobs) in &result.node_jobs {
        let mut values: BTreeMap<String, (String, Json)> = BTreeMap::new();
        let mut retry: Option<(&String, JobRetry)> = None;
        for job in jobs {
            let Some(DeclaredRun::Job {
                settings,
                retry: job_retry,
            }) = declared.get(job)
            else {
                continue;
            };
            for setting in NODE_SETTINGS {
                let Some(value) = settings.get(setting) else {
                    continue;
                };
                match values.get(setting) {
                    Some((other, seen)) if seen != value => {
                        return Err(conflict(
                            key,
                            (other, job),
                            setting,
                            (seen.to_string(), value.to_string()),
                        ));
                    }
                    Some(_) => {}
                    None => {
                        values.insert(setting.to_owned(), (job.clone(), value.clone()));
                    }
                }
            }
            if let Some(job_retry) = job_retry {
                match retry {
                    Some((other, seen)) if seen != *job_retry => {
                        let shown = |retry: JobRetry| {
                            format!(
                                "{} attempts, {}s apart",
                                retry.attempts, retry.delay_seconds
                            )
                        };
                        return Err(conflict(
                            key,
                            (other, job),
                            "[retry]",
                            (shown(seen), shown(*job_retry)),
                        ));
                    }
                    Some(_) => {}
                    None => retry = Some((job, *job_retry)),
                }
            }
        }
        if !values.is_empty() {
            result.values.insert(
                key.clone(),
                values
                    .into_iter()
                    .map(|(setting, (_, value))| (setting, value))
                    .collect(),
            );
        }
        if let Some((_, retry)) = retry {
            result.retries.insert(key.clone(), retry);
        }
    }
    for jobs in result.node_jobs.values() {
        for job in jobs {
            if let Some(DeclaredRun::Job { settings, .. }) = declared.get(job)
                && let Some(limit) = settings.get(JOB_LIMIT_SETTING).and_then(Json::as_i64)
            {
                result.job_limits.insert(job.clone(), limit);
            }
        }
    }
    Ok(result)
}
