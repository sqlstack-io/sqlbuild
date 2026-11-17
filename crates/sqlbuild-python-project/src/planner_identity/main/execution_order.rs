//! Stable native execution order, projecting no Python graph or key objects.

use crate::planner_identity::_helpers::execution::ordered_keys;
use crate::planner_identity::_helpers::graph::planner_input_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Stable native execution order, projecting no Python graph or key objects.
pub fn execution_order(
    py: Python<'_>,
    upstream: &[(ObjectKey, Vec<ObjectKey>)],
    origins: &[((ObjectKey, ObjectKey), String)],
) -> PyResult<Vec<ObjectKey>> {
    ordered_keys(upstream, origins).map_err(|message| planner_input_error(py, message))
}
