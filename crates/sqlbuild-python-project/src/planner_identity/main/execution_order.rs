//! Stable native execution order, projecting no Python graph or key objects.

use crate::planner_identity::_helpers::graph::planner_input_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::planner_identity::main::topological_order::topological_order;
use sqlbuild_analysis::planner_identity::models::GraphKey;
use std::collections::HashMap;
use crate::planner_identity::_helpers::graph::ordered_keys;

/// Stable native execution order, projecting no Python graph or key objects.
pub fn execution_order(
    py: Python<'_>,
    upstream: &[(ObjectKey, Vec<ObjectKey>)],
    origins: &[((ObjectKey, ObjectKey), String)],
) -> PyResult<Vec<ObjectKey>> {
    ordered_keys(upstream, origins).map_err(|message| planner_input_error(py, message))
}
