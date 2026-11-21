//! A local worker's process group stays registered for a forced stop only until its exit is seen, so a group ID reused afterwards is never killed.

use crate::dev_stop::main::stop_switch::stop_switch;
use crate::workers::models::WorkerExit;
use crate::workers::tests::helpers::worker_leading;
use crate::workers::tests::test_types::RecordExitTestCase;

/// Far above the highest process ID any Unix hands out (Linux caps `pid_max` at 2^22, macOS and the BSDs at 99,999), so signalling these groups can never reach a real process.
const IMPOSSIBLE_GROUP: u32 = 2_000_000_001;

#[test]
fn given_a_registered_worker_group_when_its_exit_is_seen_then_a_forced_stop_no_longer_signals_it() {
    let test_cases = [
        RecordExitTestCase {
            description: "the worker exited and was reaped",
            group: IMPOSSIBLE_GROUP,
            seen: Some(WorkerExit::Code(0)),
            expected_registered: false,
        },
        RecordExitTestCase {
            description: "a recovered worker reported absent",
            group: IMPOSSIBLE_GROUP + 1,
            seen: Some(WorkerExit::Absent),
            expected_registered: false,
        },
        RecordExitTestCase {
            description: "the worker is still running",
            group: IMPOSSIBLE_GROUP + 2,
            seen: None,
            expected_registered: true,
        },
    ];

    for test_case in test_cases {
        let mut worker = worker_leading(test_case.group);

        worker.record_exit(test_case.seen);

        let registered: bool = stop_switch()
            .groups()
            .contains(&i32::try_from(test_case.group).unwrap_or_default());
        assert_eq!(
            registered, test_case.expected_registered,
            "{}",
            test_case.description
        );
        drop(worker);
    }
}
