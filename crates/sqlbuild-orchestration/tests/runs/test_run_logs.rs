//! Run log retention deletes whole worker logs by age or count and keeps runs and their history.

use serde_json::{Value, json};

use crate::credentials::tests::location_helpers::create_location;
use crate::platform::_helpers::retention::prune_run_logs;
use crate::runs::models::RunLogRetention;
use crate::runs::tests::helpers::{logged_worker, rows, seed_logged_runs, snapshot};
use crate::runs::tests::test_types::RunLogRetentionTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_finished_runs_past_age_or_count_when_logs_are_pruned_then_whole_logs_go_and_runs_stay() {
    let test_cases = [
        RunLogRetentionTestCase {
            description: "SQLite: the shared SQL and the kept runs' own SQL stay",
            backend: Backend::Sqlite,
            expected_artifacts: 3,
            expected_runs: 4,
        },
        RunLogRetentionTestCase {
            description: "PostgreSQL: the shared SQL and the kept runs' own SQL stay",
            backend: Backend::Postgres,
            expected_artifacts: 3,
            expected_runs: 4,
        },
    ];
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/run_log_retention.json");
    let case: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("run log fixture"))
            .expect("run log case");
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
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            seed_logged_runs(
                store,
                (location, published.snapshot_id),
                case["ended_hours_ago"].as_array().expect("runs"),
            )
            .await;
            let nodes = "SELECT COUNT(*) AS count FROM sqb_run_nodes";
            let nodes_before = rows(store, nodes, Vec::new()).await;
            let scheduler = "SELECT COUNT(*) AS count FROM recall_events WHERE invocation_id LIKE 'scheduler/%'";
            let scheduler_before = rows(store, scheduler, Vec::new()).await;
            let retention = RunLogRetention::new(
                u16::try_from(case["keep_days"].as_u64().expect("days")).expect("u16"),
                u16::try_from(case["keep_runs"].as_u64().expect("runs")).expect("u16"),
            )
            .expect("retention");

            assert_eq!(
                prune_run_logs(store, retention).await.expect("prune") as u64,
                case["expected_pruned"].as_u64().expect("pruned"),
                "{}",
                test_case.description
            );
            let mut expected: Vec<Value> = case["expected_logged_runs"]
                .as_array()
                .expect("logged")
                .iter()
                .map(|index| logged_worker(u128::from(index.as_u64().expect("index"))))
                .flat_map(|worker| [json!(format!("{worker}/session")), json!(worker)])
                .collect();
            expected.sort_by_key(ToString::to_string);
            let stored: Vec<Value> = rows(
                store,
                "SELECT invocation_id FROM recall_output UNION SELECT invocation_id FROM recall_events \
                 WHERE invocation_id LIKE 'worker/%' UNION SELECT invocation_id FROM recall_artifact_refs \
                 ORDER BY invocation_id",
                Vec::new(),
            )
            .await
            .into_iter()
            .map(|row| row["invocation_id"].clone())
            .collect();
            assert_eq!(
                stored, expected,
                "{}: only the kept runs' logs remain, nested sessions included",
                test_case.description
            );
            assert_eq!(
                rows(
                    store,
                    "SELECT COUNT(*) AS count FROM recall_artifacts",
                    Vec::new()
                )
                .await[0]["count"],
                json!(test_case.expected_artifacts),
                "{}",
                test_case.description
            );
            assert_eq!(
                rows(store, scheduler, Vec::new()).await,
                scheduler_before,
                "{}: the scheduler's run history stays",
                test_case.description
            );
            assert_eq!(
                rows(store, "SELECT COUNT(*) AS count FROM sqb_runs", Vec::new()).await[0]["count"],
                json!(test_case.expected_runs),
                "{}",
                test_case.description
            );
            assert_eq!(
                rows(store, nodes, Vec::new()).await,
                nodes_before,
                "{}: node results stay",
                test_case.description
            );
            assert_eq!(
                prune_run_logs(store, retention).await.expect("repeat"),
                0,
                "{}: a repeated pass finds nothing",
                test_case.description
            );
            assert!(
                RunLogRetention::new(0, 1).is_err() && RunLogRetention::new(1, 0).is_err(),
                "{}: retention keeps at least one day and one run",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
