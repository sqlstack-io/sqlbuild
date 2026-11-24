//! What `sqb location health` exits with for each state.

use sqlbuild_orchestration::platform::models::LocationState;

use crate::native_commands::constants::STARTING_EXIT_CODE;

/// 0 when runs start, 1 when down, falling back or missing secrets, 3 while the newest instance's first load runs.
pub(crate) fn health_exit_code(state: LocationState) -> i32 {
    match state {
        LocationState::Starting => STARTING_EXIT_CODE,
        LocationState::Broken | LocationState::FallingBack | LocationState::MissingSecrets => 1,
        LocationState::Healthy | LocationState::Degraded | LocationState::DiscoveryFailed => 0,
    }
}
