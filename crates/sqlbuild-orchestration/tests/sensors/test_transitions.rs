//! Independent check health survives non-decisive outcomes, other scopes and cursor replay.

use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::TriggerChange;
use crate::runs::main::list_runs::list_runs;
use crate::runs::models::{NodeOutcome, RunOutcome};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::{
    begin_sensor_evaluation::begin_sensor_evaluation,
    complete_builtin_sensor::complete_builtin_sensor,
};
use crate::sensors::tests::helpers::{change, finish_run, snapshot};
use crate::sensors::tests::test_types::SensorTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::helpers::read_recall_events;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_independent_check_failures_when_nondecisive_results_arrive_then_only_executed_success_recovers()
 {
    let test_cases = [
        SensorTestCase {
            description: "SQLite check transitions",
            backend: Backend::Sqlite,
            expected_runs: 5,
        },
        SensorTestCase {
            description: "PostgreSQL check transitions",
            backend: Backend::Postgres,
            expected_runs: 5,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required store");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            migrate_store(store).await.expect("warm migration");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let other = create_location(store, "products", "test-admin")
                .await
                .expect("other location");
            let mut input = snapshot(location);
            input.nodes.get_mut("orders").expect("orders check").kind = "check".into();
            input
                .nodes
                .get_mut("customers")
                .expect("customers check")
                .kind = "check".into();
            input.documents.insert("jobs".into(), json!({
                "orders": {"description": "Source checks", "selection": ["orders", "customers"], "overlap": "allow"},
                "inventory": {"description": "React to a transition", "selection": ["inventory"], "overlap": "allow"}
            }));
            input.documents.insert("triggers".into(), json!({
                "failed_checks": {"description": "Actual failures", "job_key": "inventory",
                    "when": {"kind": "node_event", "selection": ["orders", "customers"], "kinds": ["failed"]}, "minimum_interval_seconds": 1},
                "recovered_checks": {"description": "Actual recoveries", "job_key": "inventory",
                    "when": {"kind": "node_event", "selection": ["orders", "customers"], "kinds": ["recovered"]}, "minimum_interval_seconds": 1}
            }));
            publish_snapshot(store, &input).await.expect("publish");
            for scope in [(other, "development"), (location, "staging")] {
                let mut isolated = input.clone();
                isolated.location_id = scope.0;
                isolated.target = scope.1.into();
                publish_snapshot(store, &isolated)
                    .await
                    .expect("isolated snapshot");
            }
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/check_transitions.json");
            let transitions: Vec<serde_json::Value> = serde_json::from_str(
                &std::fs::read_to_string(fixture_path).expect("DuckShop fixture"),
            )
            .expect("DuckShop independent check lifecycle");
            for transition in transitions {
                finish_run(
                    store,
                    &lease,
                    (location, "development"),
                    "orders",
                    serde_json::from_value(transition["outcome"].clone()).expect("run outcome"),
                    serde_json::from_value(transition["nodes"].clone()).expect("check outcomes"),
                    serde_json::from_value(transition["started"].clone()).expect("actual starts"),
                )
                .await;
                // Another location/target cannot clear either check's actual failure.
                for scope in [(other, "development"), (location, "staging")] {
                    finish_run(
                        store,
                        &lease,
                        scope,
                        "orders",
                        RunOutcome::Success,
                        [NodeOutcome::Success; 2],
                        [true; 2],
                    )
                    .await;
                }
            }
            let history = read_recall_events(store, 0, 1000).await.expect("history");
            assert_eq!(
                history
                    .iter()
                    .filter(|event| event["event_type"] == "run_node_finished"
                        && event["payload"]["change"]["change"]["outcome"] == "cancelled")
                    .count(),
                1
            );
            let triggers = list_triggers(store, location, "development")
                .await
                .expect("triggers");
            let mut queued = 0;
            for trigger in &triggers {
                let pending = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                    .await
                    .expect("evaluate")
                    .expect("due");
                let count = tokio::time::timeout(
                    Duration::from_secs(5),
                    complete_builtin_sensor(store, &lease, &pending),
                )
                .await
                .expect("bounded transition page")
                .expect("complete");
                assert_eq!(
                    count,
                    if trigger.name == "failed_checks" {
                        3
                    } else {
                        2
                    },
                    "{}",
                    test_case.description
                );
                queued += count;
                change(store, trigger.trigger_id, TriggerChange::ResetCursor).await;
            }
            assert_eq!(queued, test_case.expected_runs);
            tokio::time::sleep(Duration::from_secs(1)).await;
            for trigger in triggers {
                let replay = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                    .await
                    .expect("replay")
                    .expect("due");
                assert_eq!(
                    complete_builtin_sensor(store, &lease, &replay)
                        .await
                        .expect("deduplicated replay"),
                    if trigger.name == "failed_checks" {
                        3
                    } else {
                        2
                    }
                );
            }
            assert_eq!(
                list_runs(store, (location, "development"), 1000)
                    .await
                    .expect("replayed runs")
                    .iter()
                    .filter(|run| run.job_key.as_deref() == Some("inventory"))
                    .count(),
                test_case.expected_runs
            );
            close_test_store(fixture).await;
        });
    }
}
