//! Source writers a loading build plans for one node besides the node itself.

use crate::assembly::project::types::ObjectKey;
use crate::planner_scope::constants::{MODEL, SOURCE};
use crate::planner_scope::models::LoaderGraph;
use std::collections::{BTreeMap, BTreeSet};

/// Managed sources each model's execution deps read, and each managed source's loader chain keyed as `expand_loaders` keys it; nodes with none are absent.
pub fn node_loads(
    graph: &LoaderGraph,
    upstream: &[(ObjectKey, Vec<ObjectKey>)],
) -> BTreeMap<ObjectKey, BTreeSet<ObjectKey>> {
    let mut loads: BTreeMap<ObjectKey, BTreeSet<ObjectKey>> = BTreeMap::new();
    for (key, deps) in upstream {
        let writers: Vec<ObjectKey> = if key.0 == MODEL {
            deps.iter()
                .filter(|dep| dep.0 == SOURCE && graph.source_loader(dep).is_some())
                .cloned()
                .collect()
        } else if let Some(chain) = graph
            .source_loader(key)
            .and_then(|loader| graph.loaders.get(loader))
        {
            graph
                .upstream(chain)
                .iter()
                .map(|name| graph.key(name))
                .collect()
        } else {
            continue;
        };
        if !writers.is_empty() {
            loads.entry(key.clone()).or_default().extend(writers);
        }
    }
    loads
}
