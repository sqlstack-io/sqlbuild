//! What a location's health reports of its discovered sets.

use crate::discovered_sets::_helpers::item_graph::item_count;
use crate::discovered_sets::types::DiscoveredSets;
use crate::platform::models::DiscoveryHealth;

/// Each set's status, error, item count and last good time, by name.
pub fn discovery_health(sets: &DiscoveredSets) -> Vec<DiscoveryHealth> {
    sets.iter()
        .map(|(name, set)| DiscoveryHealth {
            name: name.clone(),
            status: set.status,
            error: set.error.clone(),
            items: item_count(set),
            last_good_at: set.last_good_at,
        })
        .collect()
}
