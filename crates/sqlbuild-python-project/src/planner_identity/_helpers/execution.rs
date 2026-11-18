//! Execution order over native keys, or the cycle that prevents one.

use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::planner_identity::main::topological_order::topological_order;
use sqlbuild_analysis::planner_identity::models::GraphKey;
use std::collections::HashMap;

/// The execution order of `upstream`, or the cycle that prevents one.
pub(crate) fn ordered_keys(
    upstream: &[(ObjectKey, Vec<ObjectKey>)],
    origins: &[((ObjectKey, ObjectKey), String)],
) -> Result<Vec<ObjectKey>, String> {
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
    Ok(topological_order(&edges, &origins)?
        .into_iter()
        .map(|key| (key.kind, key.name))
        .collect())
}
