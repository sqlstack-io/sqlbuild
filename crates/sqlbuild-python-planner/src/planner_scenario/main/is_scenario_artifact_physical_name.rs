//! Is scenario artifact physical name.

use crate::planner_scenario::_helpers::artifacts;

/// Is scenario artifact physical name.
#[inline]
pub fn is_scenario_artifact_physical_name(name: &str) -> bool {
    artifacts::is_scenario_artifact_physical_name(name)
}
