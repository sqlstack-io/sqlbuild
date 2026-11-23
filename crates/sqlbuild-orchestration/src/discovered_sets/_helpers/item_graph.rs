//! The item IDs of a set and the items each one waits on inside it.

use std::collections::{BTreeMap, BTreeSet};

use crate::discovered_sets::models::{DiscoveredResult, DiscoveredSet, ItemDependency};

/// Each item's ID with the IDs of the items of its set it waits on.
pub(crate) fn item_parents(result: &DiscoveredResult) -> BTreeMap<&str, BTreeSet<&str>> {
    let mut parents: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for item in &result.items {
        let of: &mut BTreeSet<&str> = parents.entry(item.id.as_str()).or_default();
        for dependency in &item.depends_on {
            if let ItemDependency::Item(id) = dependency {
                of.insert(id.as_str());
            }
        }
    }
    parents
}

/// An item ID on a dependency cycle, if any.
pub(crate) fn cycle_member<'a>(parents: &BTreeMap<&'a str, BTreeSet<&'a str>>) -> Option<&'a str> {
    let mut waiting: BTreeMap<&str, usize> =
        parents.iter().map(|(id, of)| (*id, of.len())).collect();
    let mut ready: Vec<&str> = waiting
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    while let Some(done) = ready.pop() {
        waiting.remove(done);
        for (id, of) in parents {
            if of.contains(done)
                && let Some(count) = waiting.get_mut(id)
            {
                *count -= 1;
                if *count == 0 {
                    ready.push(id);
                }
            }
        }
    }
    waiting.keys().next().copied()
}

/// How many items a set serves: its newest result, or its last good one after a failure.
pub(crate) fn item_count(set: &DiscoveredSet) -> u64 {
    set.result.as_ref().map_or(0, |result| {
        u64::try_from(result.items.len()).unwrap_or(u64::MAX)
    })
}

/// `1 item` or `n items`.
pub(crate) fn item_count_text(items: u64) -> String {
    match items {
        1 => "1 item".to_owned(),
        _ => format!("{items} items"),
    }
}
