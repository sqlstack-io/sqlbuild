//! `sqb location health` exits non-zero while a restarted location loads, so a deploy waits for the new code.

use sqlbuild_orchestration::platform::models::LocationState;

use crate::native_commands::_helpers::location::health::health_exit_code;
use crate::native_commands::tests::test_types::HealthExitTestCase;

#[test]
fn given_each_location_state_when_health_exits_then_only_running_states_exit_zero() {
    let test_cases = [
        HealthExitTestCase {
            description: "the newest instance is loading",
            state: LocationState::Starting,
            expected_code: 3,
        },
        HealthExitTestCase {
            description: "its load failed",
            state: LocationState::FallingBack,
            expected_code: 1,
        },
        HealthExitTestCase {
            description: "no instance checked in",
            state: LocationState::Broken,
            expected_code: 1,
        },
        HealthExitTestCase {
            description: "a provider secret is unset",
            state: LocationState::MissingSecrets,
            expected_code: 1,
        },
        HealthExitTestCase {
            description: "loaded",
            state: LocationState::Healthy,
            expected_code: 0,
        },
        HealthExitTestCase {
            description: "loaded, but the connection check failed",
            state: LocationState::Degraded,
            expected_code: 0,
        },
        HealthExitTestCase {
            description: "loaded, but a discover function failed",
            state: LocationState::DiscoveryFailed,
            expected_code: 0,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            health_exit_code(test_case.state),
            test_case.expected_code,
            "{}",
            test_case.description
        );
    }
}
