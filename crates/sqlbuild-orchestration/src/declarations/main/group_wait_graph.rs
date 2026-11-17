//! Derive group member waits from node lineage and report every set of members that wait on each other.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::_helpers::group_waits::{cycle_path, strongly_connected};
use crate::declarations::models::{GroupCycle, GroupWaits};

/// `selections` maps each member job to the node keys it builds; `deps` maps a node key to the keys it reads.
/// A member waits for every other member that builds a node its nodes read, through any number of
/// nodes; a member that builds a node itself, such as a shared function, never waits for its other builders.
pub fn group_wait_graph(
    selections: &BTreeMap<String, BTreeSet<String>>,
    deps: &BTreeMap<String, BTreeSet<String>>,
) -> GroupWaits {
    let mut owners: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (name, selection) in selections {
        for key in selection {
            owners.entry(key).or_default().insert(name);
        }
    }
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut reasons: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
    for (name, selection) in selections {
        let mut dependencies: BTreeSet<&str> = BTreeSet::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut pending: Vec<&str> = selection.iter().map(String::as_str).collect();
        while let Some(key) = pending.pop() {
            if !seen.insert(key) {
                continue;
            }
            for upstream in deps.get(key).into_iter().flatten() {
                if let Some(owners) = owners.get(upstream.as_str())
                    && !owners.contains(name.as_str())
                {
                    // The first lineage edge that makes a wait is its reason.
                    for owner in owners {
                        if dependencies.insert(owner) {
                            reasons.insert(
                                (name.clone(), (*owner).to_owned()),
                                (key.to_owned(), upstream.clone()),
                            );
                        }
                    }
                }
                if !seen.contains(upstream.as_str()) {
                    pending.push(upstream);
                }
            }
        }
        edges.insert(
            name.clone(),
            dependencies.into_iter().map(str::to_owned).collect(),
        );
    }
    let cycles: Vec<GroupCycle> = strongly_connected(&edges)
        .iter()
        .map(|members| {
            let path = cycle_path(members, &edges);
            let waits: Vec<(&str, &str, &(String, String))> = path
                .iter()
                .zip(path.iter().cycle().skip(1))
                .map(|(job, dependency)| {
                    (
                        *job,
                        *dependency,
                        &reasons[&((*job).to_owned(), (*dependency).to_owned())],
                    )
                })
                .collect();
            GroupCycle {
                description: waits
                    .iter()
                    .map(|(job, dependency, (key, upstream))| {
                        format!("{job} waits for {dependency} ({key} reads {upstream})")
                    })
                    .collect::<Vec<_>>()
                    .join("; "),
                shared_read: waits.iter().any(|(_, _, (_, upstream))| {
                    upstream.starts_with("seed:")
                        || owners
                            .get(upstream.as_str())
                            .is_some_and(|owners| owners.len() > 1)
                }),
            }
        })
        .collect();
    GroupWaits {
        edges,
        reasons,
        cycles,
    }
}
