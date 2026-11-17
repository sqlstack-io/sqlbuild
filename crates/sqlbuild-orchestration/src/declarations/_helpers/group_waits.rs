//! Find the group members that wait on each other, name a cycle among them, and say why each waits.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::constants::SEED_KEY_PREFIX;
use crate::declarations::models::GroupCycle;

/// Each member `name` waits for, with the first lineage edge that makes it wait: the node and the upstream it reads.
pub(crate) fn member_waits(
    (name, selection): (&str, &BTreeSet<String>),
    deps: &BTreeMap<String, BTreeSet<String>>,
    owners: &BTreeMap<&str, BTreeSet<&str>>,
) -> BTreeMap<String, (String, String)> {
    let mut waits: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut pending: Vec<&str> = selection.iter().map(String::as_str).collect();
    while let Some(key) = pending.pop() {
        if !seen.insert(key) {
            continue;
        }
        for upstream in deps.get(key).into_iter().flatten() {
            let builders: Option<&BTreeSet<&str>> = owners.get(upstream.as_str());
            for owner in builders
                .filter(|builders| !builders.contains(name))
                .into_iter()
                .flatten()
            {
                waits
                    .entry((*owner).to_owned())
                    .or_insert_with(|| (key.to_owned(), upstream.clone()));
            }
            if !seen.contains(upstream.as_str()) {
                pending.push(upstream);
            }
        }
    }
    waits
}

/// Groups of two or more members that each reach every other through waits, in name order.
pub(crate) fn strongly_connected(
    edges: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<BTreeSet<String>> {
    let reach = |start: &str| {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut pending: Vec<&str> = vec![start];
        while let Some(name) = pending.pop() {
            for next in edges.get(name).into_iter().flatten() {
                if seen.insert(next.clone()) {
                    pending.push(next);
                }
            }
        }
        seen
    };
    let reaches: BTreeMap<&str, BTreeSet<String>> = edges
        .keys()
        .map(|name| (name.as_str(), reach(name)))
        .collect();
    let mut components: Vec<BTreeSet<String>> = Vec::new();
    for (name, reached) in &reaches {
        if !reached.contains(*name) || components.iter().any(|done| done.contains(*name)) {
            continue;
        }
        components.push(
            reached
                .iter()
                .filter(|other| reaches[other.as_str()].contains(*name))
                .cloned()
                .collect(),
        );
    }
    components
}

/// Follow first waits inside `members`, all of which wait on each other, until a member repeats.
pub(crate) fn cycle_path<'a>(
    members: &'a BTreeSet<String>,
    edges: &'a BTreeMap<String, BTreeSet<String>>,
) -> Vec<&'a str> {
    let mut path: Vec<&str> = Vec::new();
    let mut current = members.iter().next().map(String::as_str);
    while let Some(name) = current {
        if let Some(start) = path.iter().position(|seen| *seen == name) {
            path.drain(..start);
            break;
        }
        path.push(name);
        current = edges
            .get(name)
            .and_then(|deps| deps.iter().find(|dep| members.contains(*dep)))
            .map(String::as_str);
    }
    path
}

/// The cycle through `path`, each wait with the node and upstream that cause it, and whether one reads a seed or shared node.
pub(crate) fn named_cycle(
    path: &[&str],
    reasons: &BTreeMap<(String, String), (String, String)>,
    owners: &BTreeMap<&str, BTreeSet<&str>>,
) -> GroupCycle {
    let mut waits: Vec<String> = Vec::new();
    let mut shared_read: bool = false;
    for (index, job) in path.iter().enumerate() {
        let dependency: &str = path[(index + 1) % path.len()];
        let (key, upstream) = &reasons[&((*job).to_owned(), dependency.to_owned())];
        waits.push(format!(
            "{job} waits for {dependency} ({key} reads {upstream})"
        ));
        shared_read = shared_read
            || upstream.starts_with(SEED_KEY_PREFIX)
            || owners
                .get(upstream.as_str())
                .is_some_and(|builders| builders.len() > 1);
    }
    GroupCycle {
        description: waits.join("; "),
        shared_read,
    }
}
