//! A committed but unlaunchable claim must not poison every replacement scheduler.

use std::time::Duration;

use crate::helpers::{api_ready, start_control, wait_for_exit};
use crate::worker::helpers::{claim_worker_run, start_recovery_scheduler, wait_for_worker_log};
use crate::worker::test_types::WorkerRecoveryTestCase;

#[test]
fn given_an_unlaunchable_committed_claim_when_recovering_then_rejection_retries_keep_ownership() {
    let test_cases = [
        WorkerRecoveryTestCase {
            description: "snapshot without an image pin",
            image_id: None,
            failure_event: "run_terminated",
            expected_pending_observations: 1,
            expected_status: "failed",
        },
        WorkerRecoveryTestCase {
            description: "snapshot with a malformed image pin",
            image_id: Some("sha256:invalid"),
            failure_event: "run_terminated",
            expected_pending_observations: 1,
            expected_status: "failed",
        },
        WorkerRecoveryTestCase {
            description: "missing image pin retains a rolled-back launch observation",
            image_id: None,
            failure_event: "platform_launch_failed",
            expected_pending_observations: 0,
            expected_status: "failed",
        },
        WorkerRecoveryTestCase {
            description: "invalid image pin retains a rolled-back launch observation",
            image_id: Some("sha256:invalid"),
            failure_event: "platform_launch_failed",
            expected_pending_observations: 0,
            expected_status: "failed",
        },
    ];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("recovery test runtime");
    for test_case in test_cases {
        runtime.block_on(async {
        let mut control = start_control(&["server", "--port", "0", "--no-scheduler"]);
        let url = api_ready(&mut control).await;
        let (worker, _, _) = claim_worker_run(&control, test_case.image_id).await;
        let connection =
            rusqlite::Connection::open(control.directory.path().join(".sqlbuild/control.sqlite3"))
                .expect("observe durable ownership");
        connection
            .busy_timeout(Duration::from_secs(5))
            .expect("database contention timeout");
        connection.execute_batch(&format!(
            "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00Z';
             CREATE TRIGGER reject_recovery_publication BEFORE INSERT ON recall_events
             WHEN NEW.event_type='{}' BEGIN SELECT RAISE(ABORT, 'synthetic unavailable publication'); END;", test_case.failure_event
        )).expect("expire the departed leader and interrupt durable publication");
        let mut replacement = start_recovery_scheduler(&control, &url, "orders");
        wait_for_worker_log(&mut replacement, "Retrying worker shutdown publication").await;
        let held: (i64, i64) = connection.query_row(
            "SELECT (SELECT COUNT(*) FROM sqb_write_claims), (SELECT COUNT(*) FROM sqb_runs WHERE worker_instance IS NOT NULL)",
            [], |row| Ok((row.get(0)?, row.get(1)?)),
        ).expect("retained claim while publication is unavailable");
        assert_eq!(held, (1, 1), "{}", test_case.description);
        for event_type in ["run_launch_failed", "platform_launch_failed"] {
            let pending: i64 = connection.query_row("SELECT COUNT(*) FROM recall_events WHERE event_type=?1", [event_type], |row| row.get(0)).expect("pending source/platform pair");
            assert_eq!(pending, test_case.expected_pending_observations, "{}: {event_type}", test_case.description);
        }
        connection
            .execute_batch("DROP TRIGGER reject_recovery_publication")
            .expect("restore terminal publication");
        let exit = wait_for_exit(&mut replacement).await;
        assert!(
            !exit.success(),
            "the original publication failure remains visible"
        );
        let finished: (String, i64, i64, i64) = connection.query_row(
            "SELECT status, (SELECT COUNT(*) FROM sqb_write_claims),
             (SELECT COUNT(*) FROM sqb_runs WHERE worker_instance IS NOT NULL),
             (SELECT COUNT(*) FROM recall_events WHERE event_type='run_terminated') FROM sqb_runs WHERE run_id=?1",
            [worker.identity.0.to_string()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).expect("terminal fact and released durable claims");
        assert_eq!(
            finished,
            (test_case.expected_status.into(), 0, 0, 1),
            "{}",
            test_case.description
        );
        let (producer, payload, count): (String, String, i64) = connection.query_row(
            "SELECT producer, payload, COUNT(*) OVER () FROM recall_events WHERE event_type='platform_launch_failed'",
            [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).expect("durable launch observation from the real replacement scheduler");
        assert_eq!(producer, "platform");
        assert_eq!(count, 1, "{}", test_case.description);
        let source_count: i64 = connection.query_row("SELECT COUNT(*) FROM recall_events WHERE event_type='run_launch_failed'", [], |row| row.get(0)).expect("retried source fact");
        assert_eq!(source_count, 1, "{}", test_case.description);
        let event: serde_json::Value = serde_json::from_str(&payload).expect("typed event payload");
        assert_eq!(event["platform"]["kind"], "launch_failed");
        assert_eq!(event["platform"]["run_id"], worker.identity.0.to_string());
        assert_eq!(event["platform"]["claim_id"], worker.identity.1.to_string());
        assert_eq!(event["platform"]["code"], "docker_configuration_rejected");
        assert!(
            std::fs::read_to_string(&replacement.stderr)
                .expect("shutdown recovery log")
                .contains("Worker shutdown recovered")
        );
        });
    }
}
