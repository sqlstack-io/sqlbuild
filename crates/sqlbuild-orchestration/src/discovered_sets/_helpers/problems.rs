//! Why a discover function's result cannot be a set: bad or repeated IDs, unknown items or a cycle.

use std::collections::{BTreeMap, BTreeSet};

use crate::discovered_sets::_helpers::item_graph::{cycle_member, item_parents};
use crate::discovered_sets::constants::{ITEM_ID_MAX_CHARS, ITEM_ID_PUNCTUATION};
use crate::discovered_sets::models::DiscoveredResult;

/// The first problem with `result`, or `None` when its items form a set.
pub(crate) fn discovered_problem(result: &DiscoveredResult) -> Option<String> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for item in &result.items {
        if let Some(problem) = id_problem(&item.id) {
            return Some(problem);
        }
        if !seen.insert(&item.id) {
            return Some(format!("item ID '{}' appears more than once", item.id));
        }
    }
    let parents: BTreeMap<&str, BTreeSet<&str>> = item_parents(result);
    for (id, of) in &parents {
        if let Some(missing) = of.iter().find(|parent| !seen.contains(*parent)) {
            return Some(format!(
                "item '{id}' depends on '{missing}', which is not an item of the set"
            ));
        }
        if of.contains(id) {
            return Some(format!("item '{id}' depends on itself"));
        }
    }
    if let Some(member) = cycle_member(&parents) {
        return Some(format!("item '{member}' is on a dependency cycle"));
    }
    if result.max_parallel == Some(0) {
        return Some("max_parallel must be at least 1".to_owned());
    }
    None
}

/// Why `id` is not an item ID: empty, too long, or using a character outside `[A-Za-z0-9_.:-]`.
fn id_problem(id: &str) -> Option<String> {
    if id.is_empty() {
        return Some("an item ID is empty".to_owned());
    }
    if id.chars().count() > ITEM_ID_MAX_CHARS {
        return Some(format!(
            "item ID '{id}' is longer than {ITEM_ID_MAX_CHARS} characters"
        ));
    }
    let allowed = |character: char| {
        character.is_ascii_alphanumeric() || ITEM_ID_PUNCTUATION.contains(&character)
    };
    id.chars()
        .find(|character| !allowed(*character))
        .map(|character| {
            format!("item ID '{id}' uses '{character}'; IDs use only letters, digits and _ . : -")
        })
}
