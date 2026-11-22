//! Whether a fresh attempt found a different result than a set's last good one.

use crate::discovered_sets::models::{DiscoveredSet, DiscoveredSetStatus};

/// True when `next` succeeded with a digest `previous` did not have; a failure never counts.
pub fn result_changed(previous: Option<&DiscoveredSet>, next: &DiscoveredSet) -> bool {
    next.status == DiscoveredSetStatus::Ok
        && previous.and_then(|set| set.digest.as_deref()) != next.digest.as_deref()
}
