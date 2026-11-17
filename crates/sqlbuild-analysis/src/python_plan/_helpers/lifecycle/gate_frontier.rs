//! The gates a round-trip loader reads directly or through other loaders.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;

use crate::python_plan::models::LifecycleNode;
use crate::python_selection::constants::LOADER_KIND;

/// The `gates` reached upstream of `loader` by walking only through loaders, in `gates` order.
pub(crate) fn gate_frontier(
    loader: &str,
    gates: &[String],
    nodes: &HashMap<String, LifecycleNode>,
    upstream: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let mut reached: HashSet<&str> = HashSet::new();
    let Ok(()) = breadth_first_reach([loader], None, &mut reached, |current| {
        let through: bool = nodes
            .get(*current)
            .is_some_and(|node| node.kind == LOADER_KIND);
        Ok::<_, Infallible>(
            upstream
                .get(*current)
                .filter(|_| through)
                .into_iter()
                .flatten()
                .map(String::as_str),
        )
    });
    gates
        .iter()
        .filter(|gate| reached.contains(gate.as_str()))
        .cloned()
        .collect()
}
