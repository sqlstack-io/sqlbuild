//! Record why one group member waits for another, find the members that wait on each other, and name a cycle among them.

use std::collections::{BTreeMap, BTreeSet};

/// Keep the first reason found for a wait: the first lineage edge, as the node and the upstream it reads.
pub(crate) fn record_wait(
    mut reasons: BTreeMap<(String, String), (String, String)>,
    (job, dependency): (&str, &str),
    reason: impl FnOnce() -> (String, String),
) -> BTreeMap<(String, String), (String, String)> {
    reasons
        .entry((job.to_owned(), dependency.to_owned()))
        .or_insert_with(reason);
    reasons
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
