//! `graph` with the command's jobs attached when `selectors` name a job, otherwise `graph` itself.

use crate::compiled_project::_helpers::job_catalog;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_analysis::graph::models::ProjectGraph;
use std::borrow::Cow;

/// `graph` with the command's jobs attached when `selectors` name a job, otherwise `graph` itself.
#[inline]
pub fn graph_with_jobs<'g>(
    py: Python<'_>,
    graph: &'g ProjectGraph,
    selectors: &[String],
    also_known: &[&str],
) -> PyResult<Cow<'g, ProjectGraph>> {
    job_catalog::graph_with_jobs(py, graph, selectors, also_known)
}
