//! Execution edges in project order, including SQL tests and injected reads.

use std::collections::{HashMap, HashSet};

use crate::assembly::project::types::ObjectKey;
use crate::graph::models::OrderedEdges;
use crate::planner_identity::constants::{TABLE_FN_RESOURCE, UDF_RESOURCE};
use crate::planner_identity::models::ExecutionTest;

/// Weave tests before their targets and append distinct gate and hook dependencies.
pub fn execution_upstream(
    resources: Vec<(ObjectKey, Vec<ObjectKey>)>,
    tests: &[ExecutionTest],
    injected: &[(ObjectKey, ObjectKey)],
) -> Vec<(ObjectKey, Vec<ObjectKey>)> {
    let mut upstream = OrderedEdges::default();
    for (key, deps) in resources {
        *upstream.list(&key) = deps;
    }
    for test in tests {
        let mut seen: HashSet<ObjectKey> = HashSet::new();
        let deps: Vec<ObjectKey> = if test.table_function {
            test.scope_deps
                .iter()
                .filter(|key| matches!(key.0.as_str(), UDF_RESOURCE | TABLE_FN_RESOURCE))
                .cloned()
                .collect()
        } else {
            test.scope_deps
                .iter()
                .filter_map(|target| upstream.get(target))
                .flatten()
                .filter(|key| matches!(key.0.as_str(), UDF_RESOURCE | TABLE_FN_RESOURCE))
                .filter(|key| seen.insert((*key).clone()))
                .cloned()
                .collect()
        };
        *upstream.list(&test.key) = deps;
        if !test.table_function {
            for target in &test.scope_deps {
                if upstream.get(target).is_some() {
                    upstream.list(target).push(test.key.clone());
                }
            }
        }
    }
    let mut injected_seen: HashMap<ObjectKey, HashSet<ObjectKey>> = HashMap::new();
    for (gated, read) in injected {
        let deps = upstream.list(gated);
        let seen = injected_seen
            .entry(gated.clone())
            .or_insert_with(|| deps.iter().cloned().collect());
        if seen.insert(read.clone()) {
            deps.push(read.clone());
        }
    }
    upstream.entries
}
