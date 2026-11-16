//! The command's jobs when `selectors` name one and `graph` has none attached yet.

use crate::compiled_project::_helpers::job_catalog;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_analysis::graph::models::JobSelection;
use sqlbuild_analysis::graph::models::ProjectGraph;
use std::collections::BTreeMap;

/// The command's jobs when `selectors` name one and `graph` has none attached yet.
#[inline]
pub fn needed_jobs(
    py: Python<'_>,
    graph: &ProjectGraph,
    selectors: &[String],
    also_known: &[&str],
) -> PyResult<Option<BTreeMap<String, JobSelection>>> {
    job_catalog::needed_jobs(py, graph, selectors, also_known)
}
