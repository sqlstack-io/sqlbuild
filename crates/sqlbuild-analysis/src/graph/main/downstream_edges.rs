//! Reverse ordered execution or lineage edges without projecting Python graph maps.

use crate::assembly::project::types::ObjectKey;
use crate::graph::models::OrderedEdges;

/// Inverted edges keyed in upstream order, each list sorted by `(resource type, name)`.
pub fn downstream_edges(
    upstream: &[(ObjectKey, Vec<ObjectKey>)],
) -> Vec<(ObjectKey, Vec<ObjectKey>)> {
    let mut inverted = OrderedEdges::default();
    for (key, _) in upstream {
        let _ = inverted.list(key);
    }
    for (key, deps) in upstream {
        for dep in deps {
            inverted.list(dep).push(key.clone());
        }
    }
    let mut entries = inverted.entries;
    for (_, keys) in &mut entries {
        keys.sort();
    }
    entries
}
