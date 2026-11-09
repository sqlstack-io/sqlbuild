//! Check, audit and SQL test observations are claim-bound facts, never implicit runnable selection members.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::{
    change_run::change_run, claim_next_run::claim_next_run, list_run_nodes::list_run_nodes,
    list_runs::list_runs,
};
use crate::runs::models::{
    NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunOutcome, RunStatus,
};
use crate::runs::tests::helpers::{queue, request, snapshot};
use crate::runs::tests::test_types::RunLifecycleTestCase;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotNode;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::helpers::read_recall_events;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_attached_checks_when_a_claim_reports_observations_then_frozen_membership_and_execution_history_are_preserved()
 {
    let test_cases = [
        RunLifecycleTestCase {
            description: "SQLite check observations",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Failed,
        },
        RunLifecycleTestCase {
            description: "PostgreSQL check observations",
            backend: Backend::Postgres,
            expected_status: RunStatus::Failed,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required store");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            for name in [
                "failed",
                "warned",
                "skipped",
                "cancelled",
                "passed",
                "blocked",
            ] {
                input.nodes.insert(
                    format!("check:{name}"),
                    SnapshotNode {
                        kind: "check".into(),
                        definition: json!({"name": name}),
                        raw_sql: None,
                        compiled_sql: None,
                        python_source: Some("def orders_valid(ctx): return ctx.pass_()".into()),
                    },
                );
            }
            for (key, kind) in [
                ("audit:not_null:model:orders:id", "audit"),
                ("sql_test:orders_keeps_amount", "sql_test"),
                ("sql_test:mislabelled", "audit"),
            ] {
                input.nodes.insert(
                    key.into(),
                    SnapshotNode {
                        kind: kind.into(),
                        definition: json!({"name": key}),
                        raw_sql: None,
                        compiled_sql: Some("SELECT 1".into()),
                        python_source: None,
                    },
                );
            }
            input
                .nodes
                .insert("check:products".into(), input.nodes["products"].clone());
            let published = publish_snapshot(store, &input).await.expect("snapshot");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            queue(
                store,
                &request((location, published.snapshot_id), 1, 1, &["orders"]),
            )
            .await;
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("available");
            let worker = RunAuthority::Worker {
                run_id: claim.request.run_id,
                claim_id: claim.claim_id,
            };
            let report = |change| RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: claim.request.run_id,
                claim_id: Some(claim.claim_id),
                change,
            };
            change_run(store, worker, &report(RunChange::Started))
                .await
                .expect("started");
            let before = read_recall_events(store, 0, 100)
                .await
                .expect("before rejected reports");
            for change in [
                RunChange::CheckFinished {
                    node_key: "check:passed".into(),
                    outcome: NodeOutcome::Success,
                },
                RunChange::CheckStarted {
                    node_key: "check:unknown".into(),
                },
                RunChange::CheckStarted {
                    node_key: "check:products".into(),
                },
                RunChange::CheckStarted {
                    node_key: "sql_test:mislabelled".into(),
                },
                RunChange::CheckStarted {
                    node_key: "orders".into(),
                },
            ] {
                assert!(change_run(store, worker, &report(change)).await.is_err());
            }
            assert_eq!(
                read_recall_events(store, 0, 100)
                    .await
                    .expect("rejected facts rolled back"),
                before
            );
            for (key, outcome) in [
                ("check:failed", NodeOutcome::Failed),
                ("check:warned", NodeOutcome::Warn),
                ("check:skipped", NodeOutcome::Skipped),
                ("check:cancelled", NodeOutcome::Cancelled),
                ("check:passed", NodeOutcome::Success),
                ("audit:not_null:model:orders:id", NodeOutcome::Failed),
                ("sql_test:orders_keeps_amount", NodeOutcome::Success),
            ] {
                let started = report(RunChange::CheckStarted {
                    node_key: key.into(),
                });
                assert!(
                    change_run(store, RunAuthority::Scheduler(&lease), &started)
                        .await
                        .is_err()
                );
                assert!(
                    change_run(
                        store,
                        worker,
                        &RunChangeRequest {
                            claim_id: Some(Uuid::now_v7()),
                            ..started.clone()
                        }
                    )
                    .await
                    .is_err()
                );
                assert!(
                    change_run(store, worker, &started)
                        .await
                        .expect("actual check start")
                );
                assert!(
                    !change_run(store, worker, &started)
                        .await
                        .expect("idempotent start")
                );
                let finished = report(RunChange::CheckFinished {
                    node_key: key.into(),
                    outcome,
                });
                assert!(
                    change_run(store, worker, &finished)
                        .await
                        .expect("actual check result")
                );
                assert!(
                    !change_run(store, worker, &finished)
                        .await
                        .expect("idempotent completion")
                );
                assert!(
                    change_run(store, worker, &report(finished.change.clone()))
                        .await
                        .is_err()
                );
                assert!(
                    change_run(store, worker, &report(started.change.clone()))
                        .await
                        .is_err()
                );
            }
            change_run(
                store,
                worker,
                &report(RunChange::CheckFinished {
                    node_key: "check:blocked".into(),
                    outcome: NodeOutcome::Skipped,
                }),
            )
            .await
            .expect("blocked check was not executed");
            let nodes = list_run_nodes(store, claim.request.run_id)
                .await
                .expect("frozen nodes");
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].node_key, "orders");
            assert_eq!(nodes[0].status, "pending");
            let facts = read_recall_events(store, 0, 100)
                .await
                .expect("check facts");
            assert_eq!(
                facts
                    .iter()
                    .filter(|row| row["event_type"] == "run_check_started")
                    .count(),
                7
            );
            assert_eq!(
                facts
                    .iter()
                    .filter(|row| row["event_type"] == "run_check_finished")
                    .count(),
                8
            );
            for change in [
                RunChange::NodeStarted {
                    node_key: "orders".into(),
                },
                RunChange::NodeFinished {
                    node_key: "orders".into(),
                    outcome: NodeOutcome::Success,
                    rows_affected: Some(1),
                    error_code: None,
                    skipped_because_of: None,
                },
                RunChange::Finished {
                    outcome: RunOutcome::Failed,
                    reason: "Attached check failed".into(),
                    exit_code: Some(1),
                },
            ] {
                change_run(store, worker, &report(change))
                    .await
                    .expect("finish execution without new membership");
            }
            let runs = list_runs(store, (location, "development"), 10)
                .await
                .expect("runs");
            assert_eq!(
                runs[0].status, test_case.expected_status,
                "{}",
                test_case.description
            );
            assert_eq!(
                runs[0].node_counts_json,
                BTreeMap::from([("success".into(), 1)])
            );
            assert_eq!(runs[0].selection_json, claim.request.selection);
            close_test_store(fixture).await;
        });
    }
}
