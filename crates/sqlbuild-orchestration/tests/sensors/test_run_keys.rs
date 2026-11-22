//! A run key is unique per job: two jobs can share one, and repeating a request queues nothing new.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::complete_sensor::complete_sensor;
use crate::sensors::models::{RequestedRun, SensorFunctionOutcome};
use crate::sensors::tests::helpers::{answer_sensor, append_events, snapshot};
use crate::sensors::tests::test_types::RunKeyTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_two_jobs_sharing_a_run_key_when_a_sensor_requests_them_twice_then_each_job_runs_once() {
    let test_cases = [
        RunKeyTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            requests: &[
                ("orders", "2026-11-20"),
                ("payments", "2026-11-20"),
                ("orders", "2026-11-20"),
            ],
            expected_runs: 2,
        },
        RunKeyTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            requests: &[("orders", "2026-11-20"), ("payments", "2026-11-20")],
            expected_runs: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.documents.insert(
                "jobs".into(),
                json!({
                    "orders": {"description": "Refresh orders", "selection": ["orders"]},
                    "payments": {"description": "Refresh payments", "selection": ["orders"]}
                }),
            );
            publish_snapshot(store, &input).await.expect("publish");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            append_events(store, 0..3, chrono::Utc::now()).await;
            let runs: Vec<RequestedRun> = test_case
                .requests
                .iter()
                .map(|(job, run_key)| RequestedRun {
                    job: (*job).into(),
                    run_key: (*run_key).into(),
                })
                .collect();
            for _ in 0..2 {
                let tick = begin_sensor_evaluation(store, &lease, (trigger.trigger_id, None))
                    .await
                    .expect("request")
                    .expect("due");
                answer_sensor(
                    store,
                    &tick,
                    SensorFunctionOutcome::Requested {
                        cursor: None,
                        runs: runs.clone(),
                    },
                )
                .await;
                complete_sensor(store, &lease, &tick)
                    .await
                    .expect("complete");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            let (transaction, queued) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT job_key FROM sqb_runs WHERE origin='sensor' ORDER BY job_key",
                Vec::new(),
            )
            .await
            .expect("runs");
            commit(transaction).await.expect("commit");
            assert_eq!(
                queued.len(),
                test_case.expected_runs,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
