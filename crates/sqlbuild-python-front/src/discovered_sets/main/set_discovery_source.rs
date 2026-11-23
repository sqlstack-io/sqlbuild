//! Where the next compiles of this process take their discovered sets from.

use std::sync::PoisonError;

use crate::discovered_sets::_helpers::state::{FALLEN_BACK, SOURCE, TAG_FAILURES};
use crate::discovered_sets::models::DiscoverySource;

/// Expand `for_each` assets from `source` until another source is set.
pub fn set_discovery_source(source: DiscoverySource) {
    *SOURCE.lock().unwrap_or_else(PoisonError::into_inner) = Some(source);
    TAG_FAILURES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
    FALLEN_BACK
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}
