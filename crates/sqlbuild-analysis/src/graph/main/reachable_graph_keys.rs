//! Reachable keys over the project's existing edge indexes.

use std::collections::HashSet;
use std::convert::Infallible;

use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;

use crate::assembly::project::types::ObjectKey;
use crate::graph::main::edge_keys::edge_keys;
use crate::graph::models::{EdgeDirection, ProjectGraph};

/// The starts and every key reachable in the given direction.
pub fn reachable_graph_keys(
    starts: &[ObjectKey],
    graph: &ProjectGraph,
    direction: EdgeDirection,
) -> HashSet<ObjectKey> {
    let mut visited: HashSet<&ObjectKey> = starts.iter().collect();
    let Ok(()) = breadth_first_reach(starts, None, &mut visited, |current| {
        Ok::<_, Infallible>(edge_keys(graph, current, direction))
    });
    visited.into_iter().cloned().collect()
}
