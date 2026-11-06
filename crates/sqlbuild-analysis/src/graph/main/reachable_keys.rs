//! Reachable compiled resource keys without projecting an intermediate host graph.

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;

use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;

use crate::assembly::project::types::ObjectKey;

/// The starts and every key reachable through the ordered dependency lists.
pub fn reachable_keys(
    starts: &[ObjectKey],
    edges: &[(ObjectKey, Vec<ObjectKey>)],
) -> HashSet<ObjectKey> {
    let index: HashMap<_, _> = edges.iter().map(|(key, deps)| (key, deps)).collect();
    let mut visited: HashSet<&ObjectKey> = starts.iter().collect();
    let Ok(()) = breadth_first_reach(starts, None, &mut visited, |current| {
        Ok::<_, Infallible>(index.get(current).map_or(&[][..], |deps| deps.as_slice()))
    });
    visited.into_iter().cloned().collect()
}
