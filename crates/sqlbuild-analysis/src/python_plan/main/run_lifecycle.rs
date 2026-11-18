//! Classify selected run nodes into lifecycle-aware execution regions.

use std::collections::{BTreeSet, HashMap};

use crate::assembly::project::types::ObjectKey;
use crate::python_plan::main::upstream_closure::upstream_closure;
use crate::python_plan::models::{LifecycleNode, RoundTripLoader, RunLifecycle};
use crate::python_selection::constants::{ASSET_KIND, LOADER_KIND, TASK_KIND};

/// Ingress is each selected loader whose selected upstream reads no SQL, with that upstream; the rest load after the SQL.
#[must_use]
pub fn run_lifecycle(
    selected: &BTreeSet<String>,
    nodes: &HashMap<String, LifecycleNode>,
    upstream: &HashMap<String, Vec<String>>,
) -> RunLifecycle {
    let kind = |name: &str| nodes.get(name).map_or("", |node| node.kind.as_str());
    let runs = |name: &str| [TASK_KIND, ASSET_KIND].contains(&kind(name));
    let mut ingress_loaders: BTreeSet<String> = BTreeSet::new();
    let mut round_trip_loaders: Vec<RoundTripLoader> = Vec::new();
    for loader in selected.iter().filter(|name| kind(name) == LOADER_KIND) {
        let closure: Vec<String> = upstream_closure(loader, upstream)
            .into_iter()
            .filter(|name| selected.contains(name))
            .collect();
        let waits_for: BTreeSet<ObjectKey> = closure
            .iter()
            .filter(|name| runs(name))
            .filter_map(|name| nodes.get(name))
            .flat_map(|node| node.sql_deps.iter().cloned())
            .collect();
        if waits_for.is_empty() {
            ingress_loaders.insert(loader.clone());
            continue;
        }
        round_trip_loaders.push(RoundTripLoader {
            name: loader.clone(),
            waits_for: waits_for.into_iter().collect(),
            gates: closure.into_iter().filter(|name| runs(name)).collect(),
        });
    }
    let mut ingress: BTreeSet<String> = ingress_loaders.clone();
    for loader in &ingress_loaders {
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
        .filter(|name| !ingress.contains(*name) && runs(name))
        .cloned()
        .collect();
    RunLifecycle {
        ingress_names: ingress.into_iter().collect(),
        ingress_loader_names: ingress_loaders.into_iter().collect(),
        read_side_names: read_side,
        round_trip_loaders,
    }
}
