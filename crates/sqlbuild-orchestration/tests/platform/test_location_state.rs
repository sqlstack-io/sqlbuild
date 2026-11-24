//! A location's health is its newest instance's: starting until that instance's first load ends, then from that load.

use crate::platform::models::LocationState;
use crate::platform::tests::helpers::health_history;
use crate::platform::tests::test_types::{
    Instance, LocationStateTestCase, Reported, ReportedHealth,
};

const fn reported(instance: Instance, seconds_ago: i64, health: ReportedHealth) -> Reported {
    Reported {
        instance,
        seconds_ago,
        health,
    }
}

const PREVIOUS_PUBLISHED: Reported =
    reported(Instance::Previous, 10_000, ReportedHealth::Published(11));
const PREVIOUS_CHECKED_IN: Reported = reported(Instance::Previous, 30, ReportedHealth::CheckIn);
const RESTARTED_CHECKED_IN: Reported = reported(Instance::Restarted, 5, ReportedHealth::CheckIn);

#[test]
fn given_a_location_restart_when_reading_health_then_the_newest_instance_decides() {
    let test_cases = [
        LocationStateTestCase {
            description: "a restarted instance loading its first snapshot is starting, though the previous one published",
            reported: vec![
                PREVIOUS_PUBLISHED,
                PREVIOUS_CHECKED_IN,
                RESTARTED_CHECKED_IN,
            ],
            later_reports: 0,
            expected_state: LocationState::Starting,
        },
        LocationStateTestCase {
            description: "its first load failed: falling back to the previous snapshot",
            reported: vec![
                PREVIOUS_PUBLISHED,
                RESTARTED_CHECKED_IN,
                reported(Instance::Restarted, 4, ReportedHealth::Failed(12)),
            ],
            later_reports: 0,
            expected_state: LocationState::FallingBack,
        },
        LocationStateTestCase {
            description: "its first load published: healthy, though the previous instance's last load failed",
            reported: vec![
                PREVIOUS_PUBLISHED,
                reported(Instance::Previous, 600, ReportedHealth::Failed(12)),
                RESTARTED_CHECKED_IN,
                reported(Instance::Restarted, 4, ReportedHealth::Published(13)),
            ],
            later_reports: 0,
            expected_state: LocationState::Healthy,
        },
        LocationStateTestCase {
            description: "the newest instance's start is no longer kept: its location's newest reports decide",
            reported: vec![
                reported(Instance::LongRunning, 5, ReportedHealth::CheckIn),
                reported(Instance::Previous, 600, ReportedHealth::Failed(12)),
            ],
            later_reports: 0,
            expected_state: LocationState::FallingBack,
        },
        LocationStateTestCase {
            description: "the history was cut after the newest instance started: its location's newest reports decide",
            reported: vec![
                PREVIOUS_PUBLISHED,
                PREVIOUS_CHECKED_IN,
                RESTARTED_CHECKED_IN,
            ],
            later_reports: 100,
            expected_state: LocationState::Healthy,
        },
        LocationStateTestCase {
            description: "a session ID without a start time: its location's newest reports decide",
            reported: vec![
                PREVIOUS_PUBLISHED,
                reported(Instance::Unordered, 5, ReportedHealth::CheckIn),
            ],
            later_reports: 0,
            expected_state: LocationState::Healthy,
        },
        LocationStateTestCase {
            description: "no instance checked in for 90 seconds: broken, never starting",
            reported: vec![
                PREVIOUS_PUBLISHED,
                reported(Instance::Restarted, 120, ReportedHealth::CheckIn),
            ],
            later_reports: 0,
            expected_state: LocationState::Broken,
        },
    ];
    for test_case in test_cases {
        let (observations, now) = health_history(&test_case.reported, test_case.later_reports);
        assert_eq!(
            LocationState::of(&observations, ("production", now)),
            test_case.expected_state,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_loads_of_one_generation_when_reading_health_then_the_latest_to_arrive_decides() {
    let test_cases = [
        LocationStateTestCase {
            description: "failed attempts at a generation, then its publication: healthy",
            reported: vec![
                reported(Instance::Restarted, 20, ReportedHealth::Failed(12)),
                reported(Instance::Restarted, 15, ReportedHealth::Failed(12)),
                RESTARTED_CHECKED_IN,
                reported(Instance::Restarted, 4, ReportedHealth::Published(12)),
            ],
            later_reports: 0,
            expected_state: LocationState::Healthy,
        },
        LocationStateTestCase {
            description: "a generation's publication, then a failed reload at it: falling back",
            reported: vec![
                reported(Instance::Restarted, 20, ReportedHealth::Published(12)),
                RESTARTED_CHECKED_IN,
                reported(Instance::Restarted, 4, ReportedHealth::Failed(12)),
            ],
            later_reports: 0,
            expected_state: LocationState::FallingBack,
        },
        LocationStateTestCase {
            description: "a previous instance's failures at the generation the next one published, with its start no longer kept",
            reported: vec![
                reported(Instance::Previous, 600, ReportedHealth::Failed(12)),
                reported(Instance::Previous, 300, ReportedHealth::Failed(12)),
                reported(Instance::LongRunning, 5, ReportedHealth::CheckIn),
                reported(Instance::LongRunning, 4, ReportedHealth::Published(12)),
            ],
            later_reports: 0,
            expected_state: LocationState::Healthy,
        },
    ];
    for test_case in test_cases {
        let (observations, now) = health_history(&test_case.reported, test_case.later_reports);
        assert_eq!(
            LocationState::of(&observations, ("production", now)),
            test_case.expected_state,
            "{}",
            test_case.description
        );
    }
}
