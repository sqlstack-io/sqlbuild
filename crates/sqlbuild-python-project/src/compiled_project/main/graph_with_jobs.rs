//! `graph` with the command's jobs attached when `selectors` name a job, otherwise `graph` itself.

use crate::compiled_project::main::needed_jobs::needed_jobs;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::graph::main::attach_jobs::attach_jobs;
use sqlbuild_analysis::graph::models::ProjectGraph;
use std::borrow::Cow;

/// `graph` with the command's jobs attached when `selectors` name a job, otherwise `graph` itself.
pub fn graph_with_jobs<'g>(
    py: Python<'_>,
    graph: &'g ProjectGraph,
    selectors: &[String],
    also_known: &[&str],
) -> PyResult<Cow<'g, ProjectGraph>> {
    Ok(match needed_jobs(py, graph, selectors, also_known)? {
        Some(catalog) => Cow::Owned(attach_jobs(graph.clone(), catalog)),
        None => Cow::Borrowed(graph),
    })
}
