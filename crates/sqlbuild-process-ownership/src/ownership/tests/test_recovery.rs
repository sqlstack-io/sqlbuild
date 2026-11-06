use std::time::Duration;

use crate::ownership::main::recover_worker::recover_worker;
use crate::ownership::main::stop_worker::stop_worker;
use crate::ownership::main::worker_exited::worker_exited;
use crate::ownership::tests::helpers::{
    claims, exited_within, reap, recovered_exited_within, spawn,
};
use crate::ownership::tests::test_types::RecoveryTestCase;

#[test]
fn given_a_restarted_scheduler_when_it_recovers_and_stops_a_claim_then_only_its_live_owner_is_stopped()
 {
    let test_cases = [
        RecoveryTestCase {
            description: "a live owned worker is recovered as running and stopped gracefully",
            holder: Some(0),
            marker: 0,
            lifetime_seconds: 60,
            force: false,
            expected_exited_before_recovery: false,
            expected_recovered_exited: false,
            expected_stopped: true,
        },
        RecoveryTestCase {
            description: "a live owned worker is stopped forcefully",
            holder: Some(0),
            marker: 0,
            lifetime_seconds: 60,
            force: true,
            expected_exited_before_recovery: false,
            expected_recovered_exited: false,
            expected_stopped: true,
        },
        RecoveryTestCase {
            description: "a worker that already exited releases its claim",
            holder: Some(0),
            marker: 0,
            lifetime_seconds: 0,
            force: true,
            expected_exited_before_recovery: true,
            expected_recovered_exited: true,
            expected_stopped: true,
        },
        RecoveryTestCase {
            description: "another claim's worker carrying this claim's ID, as a reused PID would, is never stopped",
            holder: Some(1),
            marker: 0,
            lifetime_seconds: 60,
            force: true,
            expected_exited_before_recovery: false,
            expected_recovered_exited: true,
            expected_stopped: false,
        },
        RecoveryTestCase {
            description: "an unowned process carrying this claim's ID is never stopped",
            holder: None,
            marker: 0,
            lifetime_seconds: 60,
            force: true,
            expected_exited_before_recovery: false,
            expected_recovered_exited: true,
            expected_stopped: false,
        },
        RecoveryTestCase {
            description: "a holder of this claim's ownership carrying another claim's ID is stopped only where the job is the proof",
            holder: Some(0),
            marker: 1,
            lifetime_seconds: 60,
            force: true,
            expected_exited_before_recovery: false,
            expected_recovered_exited: false,
            expected_stopped: cfg!(windows),
        },
    ];
    for test_case in test_cases {
        let project = tempfile::tempdir().expect("project");
        let claims = claims(project.path());
        let mut worker = spawn(
            &claims,
            (test_case.holder, test_case.marker),
            test_case.lifetime_seconds,
        );
        assert_eq!(
            exited_within(&mut worker, Duration::from_secs(1)),
            test_case.expected_exited_before_recovery,
            "{}",
            test_case.description
        );
        let owner = recover_worker(&claims[0]).expect("recover worker");
        assert_eq!(
            worker_exited(&owner).expect("observe recovered worker"),
            test_case.expected_recovered_exited,
            "{}",
            test_case.description
        );
        stop_worker(&owner, test_case.force).expect("stop recovered worker");
        assert_eq!(
            exited_within(&mut worker, Duration::from_secs(2)),
            test_case.expected_stopped,
            "{}",
            test_case.description
        );
        assert_eq!(
            recovered_exited_within(&owner, Duration::from_secs(2)),
            test_case.expected_stopped || test_case.expected_recovered_exited,
            "{}",
            test_case.description
        );
        reap(worker);
    }
}
