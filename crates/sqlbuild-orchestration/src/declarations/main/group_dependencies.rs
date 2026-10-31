//! Freeze job boundaries from snapshot lineage plus explicit waits, rejecting cycles.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::models::JobDefinition;
use crate::snapshots::models::SnapshotInput;
use crate::store::errors::ControlError;

pub fn group_dependencies(
    input: &SnapshotInput,
    jobs: &BTreeMap<String, JobDefinition>,
    (members, after): (&BTreeSet<String>, &BTreeMap<String, BTreeSet<String>>),
) -> Result<BTreeMap<String, BTreeSet<String>>, ControlError> {
    if members.is_empty()
        || members.iter().any(|name| !jobs.contains_key(name))
        || after
            .iter()
            .any(|(name, dependencies)| !members.contains(name) || !dependencies.is_subset(members))
    {
        return Err(ControlError::Invalid(
            "group needs known jobs and dependencies within its membership".into(),
        ));
    }
    let edges = lineage_dependencies(input, jobs, (members, after));
    ensure_acyclic(&edges)?;
    Ok(edges)
}

fn lineage_dependencies(
    input: &SnapshotInput,
    jobs: &BTreeMap<String, JobDefinition>,
    (members, after): (&BTreeSet<String>, &BTreeMap<String, BTreeSet<String>>),
) -> BTreeMap<String, BTreeSet<String>> {
    let mut owners: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for name in members {
        for key in &jobs[name].selection {
            owners.entry(key).or_default().insert(name);
        }
    }
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for name in members {
        let mut dependencies = after.get(name).cloned().unwrap_or_default();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut pending: Vec<&str> = jobs[name].selection.iter().map(String::as_str).collect();
        while let Some(key) = pending.pop() {
            if !seen.insert(key) {
                continue;
            }
            if let Some(node) = input.nodes.get(key)
                && let Some(upstream) = node
                    .definition
                    .get("deps")
                    .and_then(serde_json::Value::as_array)
            {
                for upstream in upstream.iter().filter_map(serde_json::Value::as_str) {
                    if let Some(owners) = owners.get(upstream) {
                        dependencies.extend(
                            owners
                                .iter()
                                .filter(|owner| **owner != name)
                                .map(|owner| (*owner).to_owned()),
                        );
                    }
                    pending.push(upstream);
                }
            }
        }
        edges.insert(name.clone(), dependencies);
    }
    edges
}

fn ensure_acyclic(edges: &BTreeMap<String, BTreeSet<String>>) -> Result<(), ControlError> {
    let mut remaining = edges.clone();
    while !remaining.is_empty() {
        let ready: BTreeSet<String> = remaining
            .iter()
            .filter(|(_, deps)| deps.is_empty())
            .map(|(name, _)| name.clone())
            .collect();
        if ready.is_empty() {
            return Err(ControlError::Invalid(
                "group dependencies contain a cycle".into(),
            ));
        }
        remaining.retain(|name, deps| {
            deps.retain(|dependency| !ready.contains(dependency));
            !ready.contains(name)
        });
    }
    Ok(())
}
