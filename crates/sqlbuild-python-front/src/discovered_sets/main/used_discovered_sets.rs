//! The discovered sets the newest compile of this process used.

use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;

use std::sync::PoisonError;

use crate::discovered_sets::_helpers::state::USED;

/// The sets the newest expansion used; empty before any compile.
pub fn used_discovered_sets() -> DiscoveredSets {
    USED.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_default()
}
