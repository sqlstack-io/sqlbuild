use std::num::NonZeroUsize;

use crate::scheduler::models::CatchUp;
use crate::scheduler::tests::helpers::simulate;
use crate::scheduler::tests::test_types::SimulationTestCase;

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
