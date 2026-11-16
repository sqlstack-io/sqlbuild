//! Stable native execution order, projecting no Python graph or key objects.

use crate::planner_identity::_helpers::graph;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Stable native execution order, projecting no Python graph or key objects.
#[inline]
pub fn execution_order(
    py: Python<'_>,
    upstream: &[(ObjectKey, Vec<ObjectKey>)],
    origins: &[((ObjectKey, ObjectKey), String)],
) -> PyResult<Vec<ObjectKey>> {
    graph::execution_order(py, upstream, origins)
}
