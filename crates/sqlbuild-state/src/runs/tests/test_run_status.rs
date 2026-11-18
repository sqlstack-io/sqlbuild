use crate::runs::main::run_status::run_status;
use crate::runs::models::{RunEnding, RunNodeCounts};
use crate::runs::tests::test_types::RunStatusTestCase;

#[test]
fn given_node_counts_when_finishing_a_run_then_the_status_follows_the_server_rule() {
    let test_cases = [
        RunStatusTestCase {
            description: "every node succeeded, one with a warning",
            counts: RunNodeCounts {
                succeeded: 3,
                warned: 1,
                ..RunNodeCounts::default()
            },
            ending: RunEnding::Completed,
            expected_status: "succeeded",
        },
        RunStatusTestCase {
            description: "a failure beside successes is partial",
            counts: RunNodeCounts {
                succeeded: 2,
                failed: 1,
                ..RunNodeCounts::default()
            },
            ending: RunEnding::Completed,
            expected_status: "partial",
        },
        RunStatusTestCase {
            description: "nodes skipped after the only node failed is a failure",
            counts: RunNodeCounts {
                failed: 1,
                skipped: 2,
                ..RunNodeCounts::default()
            },
            ending: RunEnding::Completed,
            expected_status: "failed",
        },
        RunStatusTestCase {
            description: "planned nodes that never started count against the run",
            counts: RunNodeCounts {
                succeeded: 1,
                not_started: 2,
                ..RunNodeCounts::default()
            },
            ending: RunEnding::Completed,
            expected_status: "partial",
        },
        RunStatusTestCase {
            description: "an interrupt cancels, whatever ran",
            counts: RunNodeCounts {
                succeeded: 1,
                ..RunNodeCounts::default()
            },
            ending: RunEnding::Interrupted,
            expected_status: "cancelled",
        },
        RunStatusTestCase {
            description: "a run that stopped on an error failed, whatever ran",
            counts: RunNodeCounts {
                succeeded: 4,
                ..RunNodeCounts::default()
            },
            ending: RunEnding::Raised,
            expected_status: "failed",
        },
        RunStatusTestCase {
            description: "a run with no nodes succeeded",
            counts: RunNodeCounts::default(),
            ending: RunEnding::Completed,
            expected_status: "succeeded",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            run_status(&test_case.counts, test_case.ending),
            test_case.expected_status,
            "{}",
            test_case.description
        );
    }
}
