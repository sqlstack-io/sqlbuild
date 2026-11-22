//! A set no attempt has reached, such as every set of a compile outside a location.

use crate::discovered_sets::models::{DiscoveredSet, DiscoveredSetStatus};

/// A set with no attempt and no result, checked every `check_every_seconds` if set.
pub fn never_discovered(check_every_seconds: Option<u64>) -> DiscoveredSet {
    DiscoveredSet {
        status: DiscoveredSetStatus::Never,
        fetched_at: None,
        last_good_at: None,
        digest: None,
        error: None,
        check_every_seconds,
        result: None,
    }
}
