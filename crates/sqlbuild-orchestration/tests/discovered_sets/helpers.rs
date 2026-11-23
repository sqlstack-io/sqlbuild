//! Discovered sets built from item IDs and the items each waits on.

use std::collections::BTreeMap;

use chrono::{DateTime, TimeZone, Utc};

use crate::discovered_sets::models::{DiscoveredItem, DiscoveredResult, ItemDependency};

/// A set of `(id, the item IDs it waits on)`.
pub(crate) fn set(items: &[(&str, &[&str])]) -> DiscoveredResult {
    DiscoveredResult {
        items: items
            .iter()
            .map(|(id, parents)| DiscoveredItem {
                id: (*id).to_owned(),
                depends_on: parents
                    .iter()
                    .map(|parent| ItemDependency::Item((*parent).to_owned()))
                    .collect(),
                args: BTreeMap::new(),
            })
            .collect(),
        tags: BTreeMap::new(),
        max_parallel: None,
    }
}

/// 09:00 UTC on `day` November 2026.
pub(crate) fn november(day: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, day, 9, 0, 0)
        .single()
        .expect("a valid November time")
}

/// The item IDs of `result`, in order.
pub(crate) fn ids(result: Option<&DiscoveredResult>) -> Vec<String> {
    result
        .map(|result| result.items.iter().map(|item| item.id.clone()).collect())
        .unwrap_or_default()
}
