//! Each discovered set as a deploy reads it back: status, times, digest, tags, items and their nodes.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::discovered_sets::_helpers::item_graph::item_count;
use crate::discovered_sets::constants::PLACEHOLDER_ITEM;
use crate::discovered_sets::main::item_node_name::item_node_name;
use crate::discovered_sets::models::DiscoveredSet;
use crate::discovered_sets::types::DiscoveredSets;
use crate::platform::models::{DiscoveredNodeReadback, DiscoveredSetReadback};

/// The sets by name, each with the tags of the result it serves and the nodes `assets` made of it.
pub fn discovered_readback(
    sets: &DiscoveredSets,
    assets: &BTreeMap<String, Vec<String>>,
) -> Vec<DiscoveredSetReadback> {
    let mut readback: Vec<DiscoveredSetReadback> = Vec::with_capacity(sets.len());
    for (name, set) in sets {
        let readers: Vec<String> = assets.get(name).cloned().unwrap_or_default();
        let mut nodes: Vec<DiscoveredNodeReadback> = Vec::new();
        for asset in &readers {
            nodes.extend(item_nodes(set, asset));
        }
        readback.push(DiscoveredSetReadback {
            name: name.clone(),
            status: set.status,
            fetched_at: set.fetched_at,
            last_good_at: set.last_good_at,
            digest: set.digest.clone(),
            tags: set
                .result
                .as_ref()
                .map(|result| texts(&result.tags))
                .unwrap_or_default(),
            items: item_count(set),
            error: set.error.clone(),
            check_every_seconds: set.check_every_seconds,
            assets: readers,
            nodes,
        });
    }
    readback
}

/// The node each item of `set` became under `asset`, or the asset's one placeholder.
fn item_nodes(set: &DiscoveredSet, asset: &str) -> Vec<DiscoveredNodeReadback> {
    let node = |step: &str, args: BTreeMap<String, String>| DiscoveredNodeReadback {
        key: format!("asset:{}", item_node_name(asset, Some(step))),
        asset: asset.to_owned(),
        step: step.to_owned(),
        args,
    };
    match &set.result {
        Some(result) => result
            .items
            .iter()
            .map(|item| node(&item.id, texts(&item.args)))
            .collect(),
        None => vec![node(PLACEHOLDER_ITEM, BTreeMap::new())],
    }
}

/// Each value as text: a string as it is, anything else as its JSON.
fn texts(values: &BTreeMap<String, Value>) -> BTreeMap<String, String> {
    values
        .iter()
        .map(|(key, value)| {
            let text: String = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            (key.clone(), text)
        })
        .collect()
}
