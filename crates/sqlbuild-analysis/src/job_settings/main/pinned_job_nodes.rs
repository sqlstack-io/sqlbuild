//! Each named job's nodes that take its settings, as a build of the same selectors gives them.

use std::collections::{BTreeMap, BTreeSet};

use crate::assembly::project::types::ObjectKey;
use crate::graph::models::ProjectGraph;
use crate::job_settings::_helpers::named_jobs::named_jobs;
use crate::job_settings::_helpers::selection::node_jobs;
use crate::job_settings::errors::JobSelectorError;
use crate::job_settings::models::DeclaredRun;
use crate::job_settings::types::TokenNodes;

/// Each job a `--select` token names, with the nodes that token selects which the job holds; a named job may hold none.
pub fn pinned_job_nodes(
    graph: &ProjectGraph,
    selection: (&[String], &[String]),
    declared: &BTreeMap<String, DeclaredRun>,
    token_nodes: TokenNodes<'_>,
) -> Result<BTreeMap<String, BTreeSet<ObjectKey>>, JobSelectorError> {
    let mut pinned: BTreeMap<String, BTreeSet<ObjectKey>> = named_jobs(selection.0, declared)
        .into_iter()
        .map(|job| (job, BTreeSet::new()))
        .collect();
    for (key, jobs) in node_jobs(graph, selection, declared, token_nodes)? {
        for job in jobs {
            pinned.entry(job).or_default().insert(key.clone());
        }
    }
    Ok(pinned)
}
