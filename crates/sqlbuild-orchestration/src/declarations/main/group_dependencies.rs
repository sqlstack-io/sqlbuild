//! Freeze job boundaries from snapshot lineage, rejecting cycles.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::_helpers::group_waits::upstream_keys;
use crate::declarations::main::group_cycle_message::group_cycle_message;
use crate::declarations::main::group_wait_graph::group_wait_graph;
use crate::declarations::models::JobDefinition;
use crate::snapshots::models::SnapshotInput;
use crate::store::errors::ControlError;

pub fn group_dependencies(
    (group, input): (&str, &SnapshotInput),
    jobs: &BTreeMap<String, JobDefinition>,
    members: &BTreeSet<String>,
) -> Result<BTreeMap<String, BTreeSet<String>>, ControlError> {
    if members.is_empty() || members.iter().any(|name| !jobs.contains_key(name)) {
        return Err(ControlError::Invalid("group needs known jobs".into()));
    }
    let selections: BTreeMap<String, BTreeSet<String>> = members
        .iter()
        .map(|name| (name.clone(), jobs[name].selection.clone()))
        .collect();
    let deps: BTreeMap<String, BTreeSet<String>> = input
        .nodes
        .keys()
        .map(|key| {
            let upstream: BTreeSet<String> = upstream_keys(input, key)
                .into_iter()
                .map(str::to_owned)
                .collect();
            (key.clone(), upstream)
        })
        .collect();
    let waits = group_wait_graph(&selections, &deps);
    if let Some(message) = group_cycle_message(group, &waits.cycles) {
        return Err(ControlError::Invalid(message));
    }
    Ok(waits.edges)
}
