//! Each job a schedule or sensor starts with the nodes it refreshes, less those of jobs `selectors` run.

use crate::compiled_project::_helpers::job_catalog;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::models::ProjectGraph;
use std::collections::BTreeSet;

/// Each job a schedule or sensor starts with the nodes it refreshes, less those of jobs `selectors` run.
#[inline]
pub fn triggered_jobs(
    py: Python<'_>,
    graph: &ProjectGraph,
    selectors: &[String],
) -> PyResult<Vec<(String, BTreeSet<ObjectKey>)>> {
    job_catalog::triggered_jobs(py, graph, selectors)
}
