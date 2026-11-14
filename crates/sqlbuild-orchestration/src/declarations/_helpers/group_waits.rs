//! Record why one group member waits for another, and name the waits that close a cycle.

use std::collections::{BTreeMap, BTreeSet};

/// Keep the first reason found for a wait: the first lineage edge.
pub(crate) fn record_wait(
    mut reasons: BTreeMap<(String, String), String>,
    (job, dependency): (&str, &str),
    reason: impl FnOnce() -> String,
) -> BTreeMap<(String, String), String> {
    reasons
        .entry((job.to_owned(), dependency.to_owned()))
        .or_insert_with(reason);
    reasons
}

/// Every job left waits on another, so following first dependencies from any of them closes a cycle.
pub(crate) fn describe_cycle(
    remaining: &BTreeMap<String, BTreeSet<String>>,
    reasons: &BTreeMap<(String, String), String>,
) -> String {
    let mut path: Vec<&str> = Vec::new();
    let mut current = remaining.keys().next().map(String::as_str);
    while let Some(name) = current {
        if let Some(start) = path.iter().position(|seen| *seen == name) {
            path.drain(..start);
            break;
        }
        path.push(name);
        current = remaining
            .get(name)
            .and_then(|deps| deps.iter().next())
            .map(String::as_str);
    }
    path.iter()
        .zip(path.iter().cycle().skip(1))
        .map(|(job, dependency)| {
            let reason = reasons
                .get(&((*job).to_owned(), (*dependency).to_owned()))
                .map_or("", String::as_str);
            format!("{job} waits for {dependency} ({reason})")
        })
        .collect::<Vec<_>>()
        .join("; ")
}
