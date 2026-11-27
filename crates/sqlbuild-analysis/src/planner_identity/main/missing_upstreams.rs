//! Upstream dependencies of selected keys that neither scope nor warehouse provides.

use std::collections::{HashMap, HashSet};

use crate::planner_identity::constants::{
    DBT_REF_RESOURCE, LOAD_RESOURCE, MODEL_RESOURCE, SEED_RESOURCE, SOURCE_RESOURCE,
    SQL_TEST_RESOURCE,
};
use crate::planner_identity::models::{AvailableUpstreams, GraphKey, MissingUpstream};

/// Missing upstreams, most dependents first; a deferring plan checks unselected models and seeds in the deferred target.
pub fn missing_upstreams(
    selected_keys: &[GraphKey],
    upstream_deps: &HashMap<GraphKey, Vec<GraphKey>>,
    available: &AvailableUpstreams,
) -> Vec<MissingUpstream> {
    let selected: HashSet<&GraphKey> = selected_keys.iter().collect();
    let mut missing: HashMap<GraphKey, Vec<GraphKey>> = HashMap::new();
    for selected_key in selected_keys {
        for dep_key in upstream_deps.get(selected_key).into_iter().flatten() {
            if [
                SQL_TEST_RESOURCE,
                DBT_REF_RESOURCE,
                SOURCE_RESOURCE,
                LOAD_RESOURCE,
            ]
            .contains(&dep_key.kind.as_str())
                || selected.contains(dep_key)
                || available.satisfied_keys.contains(dep_key)
                || match &available.deferred_relations {
                    Some(deferred)
                        if [MODEL_RESOURCE, SEED_RESOURCE].contains(&dep_key.kind.as_str()) =>
                    {
                        deferred.contains(&dep_key.name)
                    }
                    _ => available.existing_relations.contains(&dep_key.name),
                }
            {
                continue;
            }
            missing
                .entry(dep_key.clone())
                .or_default()
                .push(selected_key.clone());
        }
    }
    let mut entries: Vec<MissingUpstream> = missing
        .into_iter()
        .map(|(key, mut required_by)| {
            required_by.sort();
            MissingUpstream { key, required_by }
        })
        .collect();
    entries.sort_by(|left, right| {
        right
            .required_by
            .len()
            .cmp(&left.required_by.len())
            .then_with(|| left.key.cmp(&right.key))
    });
    entries
}
