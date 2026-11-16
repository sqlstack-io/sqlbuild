//! Each job a schedule or sensor starts with the nodes it refreshes, less those of jobs `selectors` run.

use crate::compiled_project::_helpers::job_catalog::job_catalog;
use crate::compiled_project::_helpers::job_catalog::selected_jobs;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::attach_jobs::attach_jobs;
use sqlbuild_analysis::graph::models::ProjectGraph;
use std::collections::BTreeSet;

/// Each job a schedule or sensor starts with the nodes it refreshes, less those of jobs `selectors` run.
pub fn triggered_jobs(
    py: Python<'_>,
    graph: &ProjectGraph,
    selectors: &[String],
) -> PyResult<Vec<(String, BTreeSet<ObjectKey>)>> {
    let Some((catalog, triggered)) = job_catalog(py)? else {
        return Ok(Vec::new());
    };
    if triggered.is_empty() {
        return Ok(Vec::new());
    }
    let running: BTreeSet<String> = selected_jobs(selectors, &catalog);
    let mut graph: ProjectGraph = attach_jobs(graph.clone(), catalog);
    let mut resolved = graph.jobs.take().unwrap_or_default();
    let mut running_nodes: BTreeSet<ObjectKey> = BTreeSet::new();
    for name in &running {
        if let Some(Ok(keys)) = resolved.get(name) {
            running_nodes.extend(keys.iter().cloned());
        }
    }
    Ok(triggered
        .into_iter()
        .filter(|name| !running.contains(name))
        .filter_map(|name| match resolved.remove(&name) {
            Some(Ok(keys)) => Some((name, &keys - &running_nodes)),
            _ => None,
        })
        .collect())
}
