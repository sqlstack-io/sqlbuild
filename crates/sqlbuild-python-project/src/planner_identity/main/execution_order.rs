//! Stable native execution order, projecting no Python graph or key objects.

use crate::planner_identity::_helpers::graph::planner_input_error;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::planner_identity::main::topological_order::topological_order;
use sqlbuild_analysis::planner_identity::models::GraphKey;
use std::collections::HashMap;

/// Stable native execution order, projecting no Python graph or key objects.
pub fn execution_order(
    py: Python<'_>,
    upstream: &[(ObjectKey, Vec<ObjectKey>)],
    origins: &[((ObjectKey, ObjectKey), String)],
) -> PyResult<Vec<ObjectKey>> {
    let key = |(kind, name): &ObjectKey| GraphKey {
        kind: kind.clone(),
        name: name.clone(),
    };
    let edges: Vec<_> = upstream
        .iter()
        .map(|(node, deps)| (key(node), deps.iter().map(key).collect()))
        .collect();
    let origins: HashMap<(GraphKey, GraphKey), String> = origins
        .iter()
        .map(|((gated, read), origin)| ((key(gated), key(read)), origin.clone()))
        .collect();
    Ok(topological_order(&edges, &origins)
        .map_err(|message| planner_input_error(py, message))?
        .into_iter()
        .map(|key| (key.kind, key.name))
        .collect())
}
