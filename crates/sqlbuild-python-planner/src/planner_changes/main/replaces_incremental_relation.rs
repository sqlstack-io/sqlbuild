//! Incremental replacements, whose bounds must ignore the destination maximum.

use crate::planner_changes::_helpers::actions;

/// Incremental replacements, whose bounds must ignore the destination maximum.
#[inline]
pub fn replaces_incremental_relation(materialization_type: &str, action: &str) -> bool {
    actions::replaces_incremental_relation(materialization_type, action)
}
