//! The line `sqb dev`'s ready card shows of a publication's discovered sets.

use crate::discovered_sets::_helpers::item_graph::{item_count, item_count_text};
use crate::discovered_sets::models::DiscoveredSetStatus;
use crate::discovered_sets::types::DiscoveredSets;

/// `orders_plan 27 items · returns_plan ✗ ssh timeout (using last good, 11 items)`; `None` without sets.
pub fn discovery_summary(sets: &DiscoveredSets) -> Option<String> {
    let parts: Vec<String> = sets
        .iter()
        .map(|(name, set)| {
            let items: u64 = item_count(set);
            let error: &str = set.error.as_deref().unwrap_or("failed");
            match (set.status, &set.result) {
                (DiscoveredSetStatus::Ok, _) => format!("{name} {}", item_count_text(items)),
                (DiscoveredSetStatus::Failed, Some(_)) => {
                    format!(
                        "{name} \u{2717} {error} (using last good, {})",
                        item_count_text(items)
                    )
                }
                (DiscoveredSetStatus::Failed, None) => {
                    format!("{name} \u{2717} {error} (never discovered)")
                }
                (DiscoveredSetStatus::Never, _) => format!("{name} not discovered"),
            }
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(" \u{b7} "))
}
