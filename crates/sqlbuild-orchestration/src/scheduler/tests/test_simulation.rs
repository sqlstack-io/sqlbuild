use std::num::NonZeroUsize;

use crate::scheduler::models::CatchUp;
use crate::scheduler::tests::helpers::{simulate, simulate_cron};
use crate::scheduler::tests::test_types::{CronSimulationTestCase, SimulationTestCase};

#[test]
fn given_random_crashes_and_clock_jumps_when_restarting_then_every_due_tick_commits_once() {
    let test_cases = [
        SimulationTestCase {
            description: "latest catch-up keeps covered tick history through crashes",
            catch_up: CatchUp::Latest,
            expected_success: true,
        },
        SimulationTestCase {
            description: "bounded all-catch-up drains rather than loses the backlog",
            catch_up: CatchUp::All {
                max_runs: NonZeroUsize::new(3).expect("nonzero test batch"),
            },
            expected_success: true,
        },
    ];
    for test_case in test_cases {
        let observed = simulate(test_case.catch_up);
        assert_eq!(
            observed.is_ok(),
            test_case.expected_success,
            "{}: {observed:?}",
            test_case.description
        );
    }
}

#[test]
fn given_dst_transitions_crashes_and_clock_jumps_when_expanding_real_cron_then_every_occurrence_ticks_once()
 {
    let bounded = CatchUp::All {
        max_runs: NonZeroUsize::new(3).expect("nonzero test batch"),
    };
    let test_cases = [
        CronSimulationTestCase {
            description: "a daily time in the autumn fold ticks once, at the later instant",
            expression: "30 1 * * *",
            wall_times: &[(1, 30)],
            catch_up: CatchUp::Latest,
            expected_success: true,
        },
        CronSimulationTestCase {
            description: "a daily time in the spring gap ticks once, at the gap end",
            expression: "30 2 * * *",
            wall_times: &[(2, 30)],
            catch_up: bounded,
            expected_success: true,
        },
        CronSimulationTestCase {
            description: "several times inside the fold and gap keep their distinct instants",
            expression: "*/30 1-2 * * *",
            wall_times: &[(1, 0), (1, 30), (2, 0), (2, 30)],
            catch_up: bounded,
            expected_success: true,
        },
    ];
    for test_case in test_cases {
        let observed = simulate_cron(
            test_case.expression,
            test_case.wall_times,
            test_case.catch_up,
        );
        assert_eq!(
            observed.is_ok(),
            test_case.expected_success,
            "{}: {observed:?}",
            test_case.description
        );
    }
}
