//! Freeze job boundaries from snapshot lineage, rejecting cycles.

use std::collections::{BTreeMap, BTreeSet};
use crate::declarations::main::group_cycle_message::group_cycle_message;
use crate::declarations::main::group_wait_graph::group_wait_graph;
use crate::declarations::models::JobDefinition;
use crate::snapshots::models::SnapshotInput;
use crate::store::errors::ControlError;
use crate::declarations::_helpers::group_waits::{describe_cycle, record_wait, upstream_keys};
use crate::declarations::models::{GroupWaits, JobDefinition};

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
        .iter()
        .map(|(key, node)| {
            let upstream: BTreeSet<String> = node
                .definition
                .get("deps")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
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

/// Each member waits for the members that build the first nodes it reaches upstream, and no further:
/// those members wait for whatever their own nodes read. A job that builds a node itself, such as a
/// shared function, never waits for its other builders and keeps walking past it.
fn lineage_dependencies(
    input: &SnapshotInput,
    jobs: &BTreeMap<String, JobDefinition>,
    members: &BTreeSet<String>,
) -> GroupWaits {
    let mut owners: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for name in members {
        for key in &jobs[name].selection {
            owners.entry(key).or_default().insert(name);
        }
    }
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut reasons: BTreeMap<(String, String), String> = BTreeMap::new();
    for name in members {
        let mut dependencies: BTreeSet<String> = BTreeSet::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut pending: Vec<&str> = jobs[name].selection.iter().map(String::as_str).collect();
        while let Some(key) = pending.pop() {
            if !seen.insert(key) {
                continue;
            }
            for upstream in upstream_keys(input, key) {
                if let Some(owners) = owners.get(upstream)
                    && !owners.contains(name.as_str())
                {
                    for owner in owners {
                        dependencies.insert((*owner).to_owned());
                        reasons = record_wait(reasons, (name, owner), || {
                            format!("{key} reads {upstream}")
                        });
                    }
                    continue;
                }
                pending.push(upstream);
            }
        }
        edges.insert(name.clone(), dependencies);
    }
    GroupWaits { edges, reasons }
}

fn ensure_acyclic(
    group: &str,
    edges: &BTreeMap<String, BTreeSet<String>>,
    reasons: &BTreeMap<(String, String), String>,
) -> Result<(), ControlError> {
    let mut remaining = edges.clone();
    while !remaining.is_empty() {
        let ready: BTreeSet<String> = remaining
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(name, _)| name.clone())
            .collect();
        if ready.is_empty() {
            return Err(ControlError::Invalid(format!(
                "group dependencies contain a cycle in job group {group}: {}",
                describe_cycle(&remaining, reasons)
            )));
        }
        remaining.retain(|name, deps| {
            deps.retain(|dependency| !ready.contains(dependency));
            !ready.contains(name)
        });
    }
    Ok(())
}
