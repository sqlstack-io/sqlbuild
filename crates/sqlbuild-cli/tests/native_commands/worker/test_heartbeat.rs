//! The standalone server remains runnable while the worker's runtime is deliberately blocked.

use std::time::{Duration, Instant};

use crate::helpers::{api_ready, start_control};
use crate::worker::helpers::{
    cancel_worker, disconnect_worker, revoke_worker, start_claimed_worker,
};
use crate::worker::test_types::WorkerHeartbeatTestCase;

#[test]
fn given_blocking_worker_work_when_control_changes_then_heartbeats_remain_independent() {
    let test_cases = [
        WorkerHeartbeatTestCase {
            description: "cancellation reaches a blocked worker runtime",
            interruption: cancel_worker,
            expected_cancelled: true,
        },
        WorkerHeartbeatTestCase {
            description: "credential revocation reaches a blocked worker runtime",
            interruption: revoke_worker,
            expected_cancelled: true,
        },
        WorkerHeartbeatTestCase {
            description: "temporary API unavailability does not cancel warehouse work",
            interruption: disconnect_worker,
            expected_cancelled: false,
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("worker runtime");
    for test_case in test_cases {
        runtime.block_on(async {
            let mut control = start_control(&["server", "--port", "0", "--no-scheduler"]);
            let url = api_ready(&mut control).await;
            let (session, worker) = start_claimed_worker(&control, &url).await;
            let operator = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(2));
                (test_case.interruption)(worker, &mut control);
                control
            });
            // No Tokio IO can run on this caller until after the assertion. A heartbeat spawned
            // on this runtime, or sharing its connection pool, cannot deliver cancellation.
            std::thread::sleep(Duration::from_secs(12));
            assert_eq!(
                session.cancel_requested(),
                test_case.expected_cancelled,
                "{}",
                test_case.description
            );
            let stopping = Instant::now();
            drop(session);
            assert!(
                stopping.elapsed() < Duration::from_secs(2),
                "{}",
                test_case.description
            );
            drop(operator.join().expect("operator completed"));
        });
    }
}
