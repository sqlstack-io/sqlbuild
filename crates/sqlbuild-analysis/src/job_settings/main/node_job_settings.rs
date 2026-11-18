//! The settings each selected node takes from the jobs it is selected through.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::graph::models::ProjectGraph;
use crate::job_settings::_helpers::node_values::node_values;
use crate::job_settings::_helpers::selection::node_jobs;
use crate::job_settings::constants::JOB_LIMIT_SETTING;
use crate::job_settings::errors::NodeJobSettingsError;
use crate::job_settings::models::{DeclaredRun, NodeJobSettings, TokenNodes};

/// Each node's named jobs, the settings they give it, and their limits.
pub fn node_job_settings(
    graph: &ProjectGraph,
    selection: (&[String], &[String]),
    declared: &BTreeMap<String, DeclaredRun>,
    token_nodes: TokenNodes<'_>,
) -> Result<NodeJobSettings, NodeJobSettingsError> {
    let mut result = NodeJobSettings {
        node_jobs: node_jobs(graph, selection, declared, token_nodes)
            .map_err(NodeJobSettingsError::Selector)?,
        ..NodeJobSettings::default()
    };
    for (key, jobs) in &result.node_jobs {
        let (values, retry) =
            node_values(key, jobs, declared).map_err(NodeJobSettingsError::Conflict)?;
        if !values.is_empty() {
            result.values.insert(key.clone(), values);
        }
        if let Some(retry) = retry {
            result.retries.insert(key.clone(), retry);
        }
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
