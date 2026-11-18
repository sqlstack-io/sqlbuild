//! Position indexes over the received facts, keyed as the warehouse folds case.

use std::collections::HashMap;
use std::hash::Hash;

use crate::warehouse_state::constants::MODEL_NODE_TYPE;
use crate::warehouse_state::models::{WarehouseState, WarehouseStateParts};

/// An identifier as the warehouse compares it.
pub(crate) fn folded(value: &str) -> String {
    value.to_lowercase()
}

/// Positions of `items` grouped by `key`, each group in source order.
fn positions<T, K: Eq + Hash>(
    items: &[T],
    key: impl Fn(&T) -> Option<K>,
) -> HashMap<K, Vec<usize>> {
    let mut grouped: HashMap<K, Vec<usize>> = HashMap::new();
    for (position, item) in items.iter().enumerate() {
        if let Some(key) = key(item) {
            grouped.entry(key).or_default().push(position);
        }
    }
    grouped
}

/// The state over `parts`, with every index built once.
pub(crate) fn indexed(parts: WarehouseStateParts) -> WarehouseState {
    WarehouseState {
        relations_by_name: positions(&parts.relations, |relation| Some(folded(&relation.name))),
        fingerprints_by_schema: positions(&parts.fingerprints, |fingerprint| {
            Some(folded(&fingerprint.location.schema))
        }),
        latest_model_runs: positions(&parts.latest_runs, |run| {
            (run.node_type == MODEL_NODE_TYPE).then(|| run.node_name.clone())
        }),
        observations_by_node: positions(&parts.observations, |observation| {
            Some((observation.kind.clone(), observation.node.clone()))
        }),
        consumption_by_schema: positions(&parts.consumption, |row| {
            Some(folded(&row.location.schema))
        }),
        parts,
    }
}
