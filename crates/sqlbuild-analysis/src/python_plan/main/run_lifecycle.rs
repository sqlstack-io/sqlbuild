//! Classify selected run nodes into lifecycle-aware execution regions.

use std::collections::{BTreeSet, HashMap};

use crate::python_plan::main::upstream_closure::upstream_closure;
use crate::python_plan::models::{LifecycleNode, RunLifecycle};
use crate::python_selection::constants::{ASSET_KIND, LOADER_KIND, TASK_KIND};

/// Ingress is every selected loader and its selected upstream tasks, assets and loaders.
#[must_use]
pub fn run_lifecycle(
    selected: &BTreeSet<String>,
    nodes: &HashMap<String, LifecycleNode>,
    upstream: &HashMap<String, Vec<String>>,
) -> RunLifecycle {
    let kind = |name: &str| nodes.get(name).map_or("", |node| node.kind.as_str());
    let loaders: BTreeSet<String> = selected
        .iter()
        .filter(|name| {
            nodes
                .get(name.as_str())
                .is_some_and(|node| node.kind == LOADER_KIND)
        })
        .cloned()
        .collect();
    let mut ingress: BTreeSet<String> = loaders.clone();
    for loader in &loaders {
        for name in upstream_closure(loader, upstream) {
            if selected.contains(&name)
                && [TASK_KIND, ASSET_KIND, LOADER_KIND].contains(&kind(&name))
            {
                ingress.insert(name);
            }
        }
    }
    let read_side: Vec<String> = selected
        .iter()
        .filter(|name| !ingress.contains(*name) && [TASK_KIND, ASSET_KIND].contains(&kind(name)))
        .cloned()
        .collect();
    RunLifecycle {
        ingress_names: ingress.into_iter().collect(),
        ingress_loader_names: loaders.into_iter().collect(),
        read_side_names: read_side,
    }
}
