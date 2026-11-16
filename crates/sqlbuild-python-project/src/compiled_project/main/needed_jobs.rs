//! The command's jobs when `selectors` name one and `graph` has none attached yet.

use crate::compiled_project::_helpers::job_catalog::job_catalog;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::graph::main::selectors_need_jobs::selectors_need_jobs;
use sqlbuild_analysis::graph::models::{JobSelection, ProjectGraph};
use std::collections::BTreeMap;

/// The command's jobs when `selectors` name one and `graph` has none attached yet.
pub fn needed_jobs(
    py: Python<'_>,
    graph: &ProjectGraph,
    selectors: &[String],
    also_known: &[&str],
) -> PyResult<Option<BTreeMap<String, JobSelection>>> {
    if graph.jobs.is_some() || !selectors_need_jobs(graph, selectors, also_known) {
        return Ok(None);
    }
    Ok(job_catalog(py)?.map(|(catalog, _)| catalog))
}
