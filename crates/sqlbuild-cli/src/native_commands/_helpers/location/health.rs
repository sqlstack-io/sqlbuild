//! What `sqb location health` exits with for each state.

use sqlbuild_orchestration::platform::models::LocationState;

use crate::native_commands::constants::STARTING_EXIT_CODE;

/// 0 only when healthy; 1 when anything failed or is down; 3 while the newest instance's first load runs, or 0 then when `allow_starting`, as a container health check counts a starting location alive.
pub(crate) fn health_exit_code(state: LocationState, allow_starting: bool) -> i32 {
    match state {
        LocationState::Healthy => 0,
        LocationState::Starting if allow_starting => 0,
        LocationState::Starting => STARTING_EXIT_CODE,
        LocationState::Broken
        | LocationState::FallingBack
        | LocationState::MissingSecrets
        | LocationState::ChangePlanFailed
        | LocationState::DiscoveryFailed
        | LocationState::Degraded => 1,
    }
}
