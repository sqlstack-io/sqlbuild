//! What `sqb location health` exits with for each state.

use sqlbuild_orchestration::platform::models::LocationState;

use crate::native_commands::constants::STARTING_EXIT_CODE;

/// 0 only when healthy; 1 when anything failed or is down; 3 while the newest instance's first load runs.
pub(crate) fn health_exit_code(state: LocationState) -> i32 {
    match state {
        LocationState::Healthy => 0,
        LocationState::Starting => STARTING_EXIT_CODE,
        LocationState::Broken
        | LocationState::FallingBack
        | LocationState::MissingSecrets
        | LocationState::ChangePlanFailed
        | LocationState::DiscoveryFailed
        | LocationState::Degraded => 1,
    }
}
