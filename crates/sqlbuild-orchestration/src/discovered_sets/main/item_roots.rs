//! The roots of a set: the items that wait on no other item of it.

use std::collections::BTreeSet;

use crate::discovered_sets::_helpers::item_graph::item_parents;
use crate::discovered_sets::models::DiscoveredResult;

/// The IDs of the items of `result` that depend on no other item of it.
pub fn item_roots(result: &DiscoveredResult) -> BTreeSet<String> {
    item_parents(result)
        .into_iter()
        .filter(|(_, parents)| parents.is_empty())
        .map(|(id, _)| id.to_owned())
        .collect()
}
