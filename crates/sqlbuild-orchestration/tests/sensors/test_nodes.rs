//! Real lifecycle facts exercise any-run rebuilds, early delivery and paged atomic recovery.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::TriggerChange;
use crate::runs::main::{
    change_run::change_run, claim_next_run::claim_next_run, list_runs::list_runs,
    request_job_run::request_job_run,
};
use crate::runs::models::{
    JobRunRequest, NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunOutcome,
};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::{
    begin_sensor_evaluation::begin_sensor_evaluation,
    complete_builtin_sensor::complete_builtin_sensor,
};
use crate::sensors::tests::helpers::{change, finish_run, snapshot};
use crate::sensors::tests::test_types::SensorTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_any_run_rebuilds_when_consuming_then_only_genuine_scoped_successes_queue_once_per_finish()
{
    let test_cases = [
        SensorTestCase {
            description: "SQLite any-run node successes",
            backend: Backend::Sqlite,
            expected_runs: 8,
        },
        SensorTestCase {
            description: "PostgreSQL any-run node successes",
            backend: Backend::Postgres,
            expected_runs: 8,
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
            input.documents.insert("jobs".into(), json!({
                "orders": {"description": "Source orders", "selection": ["orders", "customers"]},
                "inventory": {"description": "Sensor destination", "selection": ["orders", "customers"]}
            }));
            input.documents.insert("groups".into(), json!({
                "daily_orders": {"description": "Source group", "selection": ["orders", "customers"], "jobs": {"orders": []}}
            }));
            input.documents.insert("triggers".into(), json!({
                "rebuilt_orders": {"description": "Each genuine rebuild", "job_key": "inventory",
                    "when": {"kind": "node_event", "selection": ["orders", "customers"], "kinds": ["success"]},
                    "minimum_interval_seconds": 1}
            }));
            publish_snapshot(store, &input).await.expect("publish");
            let mut isolated = input.clone();
            isolated.location_id = other;
            publish_snapshot(store, &isolated)
                .await
                .expect("other location snapshot");
            isolated.location_id = location;
            isolated.target = "staging".into();
            publish_snapshot(store, &isolated)
                .await
                .expect("other target snapshot");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            for (outcome, nodes, started) in [
                (RunOutcome::Success, [NodeOutcome::Success; 2], [true; 2]),
                (RunOutcome::Success, [NodeOutcome::Success; 2], [false; 2]),
                (
                    RunOutcome::Warn,
                    [NodeOutcome::Warn, NodeOutcome::Success],
                    [true; 2],
                ),
                (RunOutcome::Failed, [NodeOutcome::Failed; 2], [true; 2]),
                (
                    RunOutcome::Partial,
                    [NodeOutcome::Skipped, NodeOutcome::Success],
                    [false, true],
                ),
                (
                    RunOutcome::Cancelled,
                    [NodeOutcome::Cancelled; 2],
                    [true; 2],
                ),
            ] {
                finish_run(
                    store,
                    &lease,
                    (location, "development"),
                    "orders",
                    outcome,
                    nodes,
                    started,
                )
                .await;
            }
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
            finish_run(
                store,
                &lease,
                (location, "development"),
                "daily_orders",
                RunOutcome::Success,
                [NodeOutcome::Success; 2],
                [true; 2],
            )
            .await;
            for _ in 0..46 {
                finish_run(
                    store,
                    &lease,
                    (location, "development"),
                    "orders",
                    RunOutcome::Success,
                    [NodeOutcome::Success; 2],
                    [false; 2],
                )
                .await;
            }
            let run_id = Uuid::now_v7();
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            request_job_run(
                store,
                &context,
                &JobRunRequest {
                    run_id,
                    location_id: location,
                    target: "development".into(),
                    job_key: "orders".into(),
                    reason: "Deliver before whole-run completion".into(),
                    priority: 10,
                    window: None,
                },
            )
            .await
            .expect("request running source");
            let claim = claim_next_run(
                store,
                &lease,
                &BTreeMap::new(),
                Some((location, "development")),
            )
            .await
            .expect("claim")
            .expect("ready");
            assert_eq!(claim.request.run_id, run_id);
            let authority = RunAuthority::Worker {
                run_id,
                claim_id: claim.claim_id,
            };
            let changes = [
                RunChange::Started,
                RunChange::NodeStarted {
                    node_key: "orders".into(),
                },
                RunChange::NodeFinished {
                    node_key: "orders".into(),
                    outcome: NodeOutcome::Success,
                    rows_affected: None,
                    error_code: None,
                    skipped_because_of: None,
                },
            ];
            for change in changes {
                let request = RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id,
                    claim_id: Some(claim.claim_id),
                    change,
                };
                let mut wrong = request.clone();
                wrong.claim_id = Some(Uuid::now_v7());
                assert!(
                    change_run(store, authority, &wrong).await.is_err(),
                    "another claim cannot supply start/finish evidence"
                );
                assert!(
                    change_run(store, authority, &request)
                        .await
                        .expect("worker fact")
                );
                assert!(
                    !change_run(store, authority, &request)
                        .await
                        .expect("duplicate worker fact")
                );
            }
            let pending = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("request evaluation")
                .expect("due");
            let replica = second_store(store).await;
            let fault = TransactionFault {
                description: "node cursor rollback",
                table: "sqb_trigger_state",
                operation: "UPDATE",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT, 'injected node cursor failure')",
                postgres_body: "RAISE EXCEPTION 'injected node cursor failure'",
            };
            install_fault(store, &fault).await;
            assert!(
                complete_builtin_sensor(store, &lease, &pending)
                    .await
                    .is_err()
            );
            remove_fault(store, &fault).await;
            assert!(
                list_runs(store, (location, "development"), 1000)
                    .await
                    .expect("rollback")
                    .iter()
                    .all(|run| run.job_key.as_deref() != Some("inventory"))
            );
            assert_eq!(
                begin_sensor_evaluation(&replica, &lease, trigger.trigger_id)
                    .await
                    .expect("recover pending"),
                Some(pending.clone())
            );
            assert_eq!(
                tokio::time::timeout(
                    Duration::from_secs(5),
                    complete_builtin_sensor(&replica, &lease, &pending),
                )
                .await
                .expect("bounded 100-row page")
                .expect("first page"),
                7
            );
            assert_eq!(
                complete_builtin_sensor(store, &lease, &pending)
                    .await
                    .expect("duplicate completion"),
                0
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
            let next = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("second evaluation")
                .expect("due");
            assert!(next.cursor.event_storage_id > 0);
            assert_eq!(
                complete_builtin_sensor(store, &lease, &next)
                    .await
                    .expect("second page"),
                0,
                "unstarted finishes never fill a page"
            );
            for change in [
                RunChange::NodeStarted {
                    node_key: "customers".into(),
                },
                RunChange::NodeFinished {
                    node_key: "customers".into(),
                    outcome: NodeOutcome::Success,
                    rows_affected: None,
                    error_code: None,
                    skipped_because_of: None,
                },
            ] {
                change_run(
                    store,
                    authority,
                    &RunChangeRequest {
                        event_id: Uuid::now_v7(),
                        run_id,
                        claim_id: Some(claim.claim_id),
                        change,
                    },
                )
                .await
                .expect("later node in the same run");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            let later = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("later evaluation")
                .expect("due");
            assert_eq!(
                complete_builtin_sensor(store, &lease, &later)
                    .await
                    .expect("later finish"),
                1,
                "distinct successful finishes in the same run must not collapse"
            );
            let runs = list_runs(store, (location, "development"), 1000)
                .await
                .expect("runs");
            assert_eq!(
                runs.iter()
                    .filter(|run| run.job_key.as_deref() == Some("inventory"))
                    .count(),
                test_case.expected_runs,
                "{}",
                test_case.description
            );
            assert_eq!(
                runs.iter()
                    .find(|run| run.run_id == run_id)
                    .expect("source retained")
                    .status,
                crate::runs::models::RunStatus::Running
            );
            change(store, trigger.trigger_id, TriggerChange::ResetCursor).await;
            for _ in 0..2 {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let replay = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                    .await
                    .expect("replay")
                    .expect("due");
                complete_builtin_sensor(store, &lease, &replay)
                    .await
                    .expect("deduplicated replay");
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
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
