//! Read what a group member waits for, record why, and name the waits that close a cycle.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::constants::{LOADER_KIND, SOURCE_KIND, SOURCE_LOADER_POINTER};
use crate::snapshots::models::SnapshotInput;

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

/// A node's published `deps`, and for a source its loader, whose upstream Python it waits for too.
pub(crate) fn upstream_keys<'a>(input: &'a SnapshotInput, key: &str) -> Vec<&'a str> {
    let Some(node) = input.nodes.get(key) else {
        return Vec::new();
    };
    let mut upstream: Vec<&'a str> = node
        .definition
        .get("deps")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .collect();
    if node.kind == SOURCE_KIND
        && let Some(loader) = node
            .definition
            .pointer(SOURCE_LOADER_POINTER)
            .and_then(serde_json::Value::as_str)
        && let Some((loader_key, _)) = input
            .nodes
            .get_key_value(&format!("{LOADER_KIND}:{loader}"))
        && !upstream.contains(&loader_key.as_str())
    {
        upstream.push(loader_key);
    }
    upstream
}
