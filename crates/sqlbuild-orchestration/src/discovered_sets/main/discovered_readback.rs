//! Each discovered set as a deploy reads it back: status, times, digest, tags and item count.

use std::collections::BTreeMap;

use crate::discovered_sets::_helpers::item_graph::item_count;
use crate::discovered_sets::models::DiscoveredResult;
use crate::discovered_sets::types::DiscoveredSets;
use crate::platform::models::DiscoveredSetReadback;

/// The sets by name, each with the tags of the result it serves.
pub fn discovered_readback(sets: &DiscoveredSets) -> Vec<DiscoveredSetReadback> {
    sets.iter()
        .map(|(name, set)| DiscoveredSetReadback {
            name: name.clone(),
            status: set.status,
            fetched_at: set.fetched_at,
            last_good_at: set.last_good_at,
            digest: set.digest.clone(),
            tags: set.result.as_ref().map(served_tags).unwrap_or_default(),
            items: item_count(set),
            error: set.error.clone(),
            check_every_seconds: set.check_every_seconds,
        })
        .collect()
}

/// The tags of the result a set serves.
fn served_tags(result: &DiscoveredResult) -> BTreeMap<String, serde_json::Value> {
    result.tags.clone()
}
