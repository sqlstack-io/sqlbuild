//! The one place named jobs' settings meet the command line's: command line > named job.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;

use crate::job_settings::_helpers::build_job_settings::build_job_settings;
use crate::job_settings::_helpers::named_jobs::named_jobs;
use crate::job_settings::_helpers::node_job_settings::node_job_settings;
use crate::job_settings::_helpers::without_given_settings::without_given_settings;
use crate::job_settings::constants::JOB_SETTINGS;
use crate::job_settings::errors::NodeJobSettingsError;
use crate::job_settings::models::{DeclaredRun, ResolvedJobSettings};
use crate::job_settings::types::NodeSelection;

/// The value of whole-run setting `name` every named job sets, if they all set the same one.
fn shared_value(
    name: &str,
    named: &[String],
    declared: &BTreeMap<String, DeclaredRun>,
) -> Option<Json> {
    let mut values = named.iter().map(|job| match declared.get(job) {
        Some(DeclaredRun::Job { settings, .. }) => settings.get(name),
        _ => None,
    });
    let first: &Json = values.next()??;
    values
        .all(|value| value == Some(first))
        .then(|| first.clone())
}

/// The named jobs' settings less those `given`, then checked for conflicts; per node with `nodes`.
pub fn resolve_job_settings(
    declared: BTreeMap<String, DeclaredRun>,
    (selectors, given): (&[String], &BTreeSet<String>),
    nodes: Option<NodeSelection<'_>>,
) -> Result<ResolvedJobSettings, NodeJobSettingsError> {
    let declared: BTreeMap<String, DeclaredRun> = without_given_settings(declared, given);
    let named: Vec<String> = named_jobs(selectors, &declared);
    if named.is_empty() {
        return Ok(ResolvedJobSettings {
            declared,
            ..ResolvedJobSettings::default()
        });
    }
    let build: BTreeMap<String, Json> =
        build_job_settings(&named, &declared).map_err(NodeJobSettingsError::Conflict)?;
    let mut whole_run: BTreeMap<String, Json> = BTreeMap::new();
    for spec in JOB_SETTINGS.iter().filter(|spec| spec.whole_run) {
        if let Some(value) = shared_value(spec.name, &named, &declared) {
            whole_run.insert(spec.name.to_owned(), value);
        }
    }
    let nodes = match nodes {
        Some((graph, selection, token_nodes)) => {
            Some(node_job_settings(graph, selection, &declared, token_nodes)?)
        }
        None => None,
    };
    Ok(ResolvedJobSettings {
        declared,
        named,
        build,
        whole_run,
        nodes,
    })
}
