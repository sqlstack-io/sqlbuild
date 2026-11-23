//! Collections of discovered sets.

use std::collections::BTreeMap;

use crate::discovered_sets::models::DiscoveredSet;

/// Every discovered set of a snapshot, by discover function name.
pub type DiscoveredSets = BTreeMap<String, DiscoveredSet>;
