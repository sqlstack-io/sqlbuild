//! Freeze job boundaries from snapshot lineage, rejecting cycles.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::_helpers::group_waits::{describe_cycle, record_wait, upstream_keys};
use crate::declarations::_helpers::node_waits::{GroupMembers, node_wait_plan};
use crate::declarations::models::{GroupOrder, GroupWaits, JobDefinition};
use crate::snapshots::models::SnapshotInput;
use crate::store::errors::ControlError;

/// Each member's upstream member jobs, and the ones a member that waits per node waits for whole because they share a pool; a group whose members all wait for jobs must order them without a cycle, and one with members that wait per node must order its nodes.
pub fn group_dependencies(
    (group, input): (&str, &SnapshotInput),
    jobs: &BTreeMap<String, JobDefinition>,
    (members, node_waits): (&BTreeSet<String>, &BTreeSet<String>),
) -> Result<GroupOrder, ControlError> {
    if members.is_empty()
        || members.iter().any(|name| !jobs.contains_key(name))
        || !node_waits.is_subset(members)
    {
        return Err(ControlError::Invalid("group needs known jobs".into()));
    }
    let waits = lineage_dependencies(input, jobs, members);
    if node_waits.is_empty() {
        ensure_acyclic(group, &waits.edges, &waits.reasons)?;
        return Ok(GroupOrder {
            jobs: waits.edges,
            pool_waits: BTreeMap::new(),
        });
    }
    let pool_waits = node_wait_plan(&GroupMembers {
        input,
        jobs,
        edges: &waits.edges,
        node_waits,
    })
    .map_err(|cycle| {
        ControlError::Invalid(format!(
            "group dependencies contain a cycle in job group {group}: {cycle}"
        ))
    })?;
    Ok(GroupOrder {
        jobs: waits.edges,
        pool_waits,
    })
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
