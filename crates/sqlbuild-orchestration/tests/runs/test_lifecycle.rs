use std::collections::BTreeMap;

use crate::credentials::models::AuditContext;
use crate::platform::models::PlatformEvent;
use crate::runs::main::change_run::change_run;
use crate::runs::main::claim_next_run::claim_next_run;
use crate::runs::main::heartbeat_run::heartbeat_run;
use crate::runs::main::list_run_nodes::list_run_nodes;
use crate::runs::main::list_runs::list_runs;
use crate::runs::models::{
    NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunOutcome, RunStatus,
};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::helpers::read_recall_events;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::runs::tests::helpers::{queue, request, snapshot};
use crate::runs::tests::test_types::{RunLifecycleTestCase, TransactionFault};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_claimed_worker_when_reports_are_retried_and_fail_between_writes_then_facts_outcomes_and_claims_are_atomic()
 {
    let test_cases = [
        RunLifecycleTestCase {
            description: "SQLite preserves facts, outcomes and claims through worker failures",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Partial,
        },
        RunLifecycleTestCase {
            description: "PostgreSQL preserves facts, outcomes and claims through worker failures",
            backend: Backend::Postgres,
            expected_status: RunStatus::Partial,
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
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("snapshot");
            let scope = (location, published.snapshot_id);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            queue(store, &request(scope, 1, 2, &["customers", "orders"])).await;
            queue(store, &request(scope, 2, 1, &["customers", "orders"])).await;
            let limits = BTreeMap::new();
            let claim = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("claim")
                .expect("available");
            let worker = RunAuthority::Worker {
                run_id: claim.request.run_id,
                claim_id: claim.claim_id,
            };
            let started = RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: claim.request.run_id,
                claim_id: Some(claim.claim_id),
                change: RunChange::Started,
            };
            let other_worker = RunAuthority::Worker {
                run_id: Uuid::from_u128(2),
                claim_id: Uuid::now_v7(),
            };
            let operator = AuditContext {
                actor: "test-operator".into(),
                granting_role: "operator".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            assert!(change_run(store, other_worker, &started).await.is_err());
            assert!(
                change_run(store, RunAuthority::Operator(&operator), &started)
                    .await
                    .is_err()
            );
            assert!(
                change_run(store, worker, &started)
                    .await
                    .expect("worker can start before launcher acknowledgement")
            );
            assert!(
                !change_run(store, worker, &started)
                    .await
                    .expect("identical retry")
            );
            let launched = RunChangeRequest {
                event_id: Uuid::now_v7(),
                change: RunChange::Launched {
                    container_id: Some("synthetic-orders-worker".into()),
                },
                ..started.clone()
            };
            change_run(store, RunAuthority::Scheduler(&lease), &launched)
                .await
                .expect("late launcher acknowledgement");
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("runs")
                    .iter()
                    .find(|run| run.run_id == claim.request.run_id)
                    .expect("started run")
                    .status,
                RunStatus::Running
            );
            let launch_failed = RunChangeRequest {
                event_id: Uuid::now_v7(),
                change: RunChange::LaunchFailed {
                    code: "docker_launch_failed".into(),
                },
                ..started.clone()
            };
            assert!(change_run(store, worker, &launch_failed).await.is_err());
            assert!(
                change_run(store, RunAuthority::Operator(&operator), &launch_failed)
                    .await
                    .is_err()
            );
            let wrong_claim = RunChangeRequest {
                claim_id: Some(Uuid::now_v7()),
                ..launch_failed.clone()
            };
            assert!(
                change_run(store, RunAuthority::Scheduler(&lease), &wrong_claim)
                    .await
                    .is_err()
            );
            let before_launch_failure = read_recall_events(store, 0, 100)
                .await
                .expect("before launch failure");
            let fault = TransactionFault {
                description: "platform launch observation must commit with its run fact",
                table: "recall_events",
                operation: "INSERT",
                condition: "WHEN NEW.event_type='platform_launch_failed'",
                sqlite_body: "SELECT RAISE(ABORT,'injected launch observation failure')",
                postgres_body: "RAISE EXCEPTION 'injected launch observation failure'",
            };
            install_fault(store, &fault).await;
            assert!(
                change_run(store, RunAuthority::Scheduler(&lease), &launch_failed)
                    .await
                    .is_err()
            );
            remove_fault(store, &fault).await;
            assert_eq!(
                read_recall_events(store, 0, 100)
                    .await
                    .expect("rolled back launch fact"),
                before_launch_failure
            );
            assert!(
                change_run(store, RunAuthority::Scheduler(&lease), &launch_failed)
                    .await
                    .expect("launch observation")
            );
            assert!(
                !change_run(store, RunAuthority::Scheduler(&lease), &launch_failed)
                    .await
                    .expect("idempotent retry")
            );
            let observations: Vec<_> = read_recall_events(store, 0, 100)
                .await
                .expect("health facts")
                .into_iter()
                .filter(|row| row["event_type"] == "platform_launch_failed")
                .collect();
            assert_eq!(observations.len(), 1);
            let platform: PlatformEvent =
                serde_json::from_value(observations[0]["payload"]["platform"].clone())
                    .expect("typed launch failure");
            assert_eq!(
                platform,
                PlatformEvent::LaunchFailed {
                    run_id: started.run_id,
                    claim_id: claim.claim_id,
                    code: "docker_launch_failed".into()
                }
            );
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("live run")
                    .into_iter()
                    .find(|run| run.run_id == started.run_id)
                    .expect("original run")
                    .status,
                RunStatus::Running
            );
            assert!(
                claim_next_run(store, &lease, &limits, None)
                    .await
                    .expect("physical ownership retained")
                    .is_none()
            );
            let success = RunChangeRequest {
                event_id: Uuid::now_v7(),
                change: RunChange::Finished {
                    outcome: RunOutcome::Success,
                    reason: "worker complete".into(),
                    exit_code: Some(0),
                },
                ..started.clone()
            };
            assert!(
                change_run(store, worker, &success).await.is_err(),
                "unfinished nodes cannot be marked successful"
            );
            for (node, outcome) in [
                ("customers", NodeOutcome::Success),
                ("orders", NodeOutcome::Failed),
            ] {
                let node_start = RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    change: RunChange::NodeStarted {
                        node_key: node.into(),
                    },
                    ..started.clone()
                };
                change_run(store, worker, &node_start)
                    .await
                    .expect("node started");
                let completed = RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    change: RunChange::NodeFinished {
                        node_key: node.into(),
                        outcome,
                        rows_affected: Some(17),
                        error_code: (outcome == NodeOutcome::Failed).then(|| "E100".into()),
                        skipped_because_of: None,
                    },
                    ..started.clone()
                };
                let before_nodes = list_run_nodes(store, started.run_id)
                    .await
                    .expect("node before fault");
                let before_events = read_recall_events(store, 0, 100)
                    .await
                    .expect("events before node fault");
                for table in ["sqb_run_nodes", "sqb_runs"] {
                    let point = TransactionFault {
                        description: "node projection and counter failure",
                        table,
                        operation: "UPDATE",
                        condition: "",
                        sqlite_body: "SELECT RAISE(ABORT,'injected node failure')",
                        postgres_body: "RAISE EXCEPTION 'injected node failure'",
                    };
                    install_fault(store, &point).await;
                    assert!(
                        change_run(store, worker, &completed).await.is_err(),
                        "{} at {table}",
                        point.description
                    );
                    remove_fault(store, &point).await;
                    assert_eq!(
                        list_run_nodes(store, started.run_id)
                            .await
                            .expect("node rollback"),
                        before_nodes
                    );
                    assert_eq!(
                        read_recall_events(store, 0, 100)
                            .await
                            .expect("node fact rollback"),
                        before_events
                    );
                }
                assert!(
                    change_run(store, worker, &completed)
                        .await
                        .expect("node completed")
                );
                assert!(
                    !change_run(store, worker, &completed)
                        .await
                        .expect("node report retry")
                );
            }
            assert!(
                change_run(store, worker, &success).await.is_err(),
                "mixed node outcomes are partial, not success"
            );
            let partial = RunChangeRequest {
                event_id: Uuid::now_v7(),
                change: RunChange::Finished {
                    outcome: RunOutcome::Partial,
                    reason: "independent customer work completed".into(),
                    exit_code: Some(1),
                },
                ..started.clone()
            };
            let before = read_recall_events(store, 0, 100)
                .await
                .expect("before terminal");
            for (table, operation) in [
                ("recall_events", "INSERT"),
                ("sqb_runs", "UPDATE"),
                ("sqb_write_claims", "DELETE"),
                ("sqb_run_liveness", "DELETE"),
            ] {
                let point = TransactionFault {
                    description: "terminal transaction failure",
                    table,
                    operation,
                    condition: "",
                    sqlite_body: "SELECT RAISE(ABORT,'injected terminal failure')",
                    postgres_body: "RAISE EXCEPTION 'injected terminal failure'",
                };
                install_fault(store, &point).await;
                assert!(
                    change_run(store, worker, &partial).await.is_err(),
                    "{} at {table} must roll back the complete transition",
                    point.description
                );
                remove_fault(store, &point).await;
                assert_eq!(
                    read_recall_events(store, 0, 100)
                        .await
                        .expect("rolled back event"),
                    before
                );
                assert!(
                    claim_next_run(store, &lease, &limits, None)
                        .await
                        .expect("claims retained")
                        .is_none()
                );
            }
            let unresponsive = RunChangeRequest {
                event_id: Uuid::now_v7(),
                change: RunChange::Unresponsive,
                ..started.clone()
            };
            change_run(store, RunAuthority::Scheduler(&lease), &unresponsive)
                .await
                .expect("monitor records loss of heartbeat");
            assert!(
                !heartbeat_run(store, (started.run_id, claim.claim_id))
                    .await
                    .expect("heartbeat recovers the worker")
            );
            assert!(
                heartbeat_run(store, (started.run_id, Uuid::now_v7()))
                    .await
                    .is_err(),
                "another claim cannot report this run alive"
            );
            let cancel = RunChangeRequest {
                event_id: Uuid::now_v7(),
                change: RunChange::CancelRequested {
                    actor: "test-operator".into(),
                    reason: "Stop the orders build".into(),
                },
                ..started.clone()
            };
            assert!(
                change_run(store, worker, &cancel).await.is_err(),
                "workers cannot impersonate operators"
            );
            change_run(store, RunAuthority::Operator(&operator), &cancel)
                .await
                .expect("record cancellation");
            assert!(
                heartbeat_run(store, (started.run_id, claim.claim_id))
                    .await
                    .expect("heartbeat reports pending cancellation")
            );
            assert!(
                claim_next_run(store, &lease, &limits, None)
                    .await
                    .expect("cancel request does not release an executing run's claims")
                    .is_none()
            );
            assert!(
                change_run(store, worker, &partial)
                    .await
                    .expect("terminal report atomically releases claims")
            );
            assert!(
                !change_run(store, worker, &partial)
                    .await
                    .expect("terminal retry after liveness cleanup")
            );
            let mut changed = partial.clone();
            changed.change = RunChange::Finished {
                outcome: RunOutcome::Success,
                reason: "changed retry".into(),
                exit_code: Some(0),
            };
            assert!(
                change_run(store, worker, &changed).await.is_err(),
                "same event ID cannot change its content"
            );
            assert!(
                heartbeat_run(store, (started.run_id, claim.claim_id))
                    .await
                    .is_err(),
                "terminal runs cannot be revived by a late heartbeat"
            );
            let projected = list_runs(store, (location, "development"), 100)
                .await
                .expect("terminal projection")
                .into_iter()
                .find(|run| run.run_id == started.run_id)
                .expect("original run");
            assert_eq!(
                projected.status, test_case.expected_status,
                "{}",
                test_case.description
            );
            assert_eq!(
                projected.node_counts_json,
                BTreeMap::from([("success".into(), 1), ("failed".into(), 1)])
            );
            let nodes = list_run_nodes(store, started.run_id)
                .await
                .expect("node evidence");
            assert_eq!(nodes[0].node_key, "customers");
            assert_eq!(nodes[0].status, "success");
            assert_eq!(nodes[0].rows_affected, Some(17));
            assert_eq!(nodes[1].error_code.as_deref(), Some("E100"));
            let late_failure = RunChangeRequest {
                event_id: Uuid::now_v7(),
                ..launch_failed.clone()
            };
            let before_late = read_recall_events(store, 0, 100)
                .await
                .expect("terminal facts");
            let expiry = TransactionFault {
                description: "late launch observation loses leadership before commit",
                sqlite_body: "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
                postgres_body: "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
                ..fault
            };
            for (point, fenced) in [(&fault, false), (&expiry, true)] {
                install_fault(store, point).await;
                let failed =
                    change_run(store, RunAuthority::Scheduler(&lease), &late_failure).await;
                let error = failed.expect_err(point.description);
                assert_eq!(
                    matches!(error, ControlError::Fenced),
                    fenced,
                    "{}",
                    point.description
                );
                remove_fault(store, point).await;
                assert_eq!(
                    read_recall_events(store, 0, 100)
                        .await
                        .expect("terminal rollback"),
                    before_late
                );
            }
            assert!(
                change_run(store, RunAuthority::Scheduler(&lease), &late_failure)
                    .await
                    .expect("late observation")
            );
            assert!(
                !change_run(store, RunAuthority::Scheduler(&lease), &late_failure)
                    .await
                    .expect("late retry")
            );
            assert!(change_run(store, worker, &late_failure).await.is_err());
            assert!(
                change_run(store, RunAuthority::Operator(&operator), &late_failure)
                    .await
                    .is_err()
            );
            for invalid in [
                RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    claim_id: Some(Uuid::now_v7()),
                    ..late_failure.clone()
                },
                RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    claim_id: None,
                    ..late_failure.clone()
                },
                RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: Uuid::from_u128(2),
                    ..late_failure.clone()
                },
                RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: Uuid::from_u128(2),
                    claim_id: Some(Uuid::new_v5(&Uuid::from_u128(2), b"run_claimed")),
                    ..late_failure.clone()
                },
                RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    ..started.clone()
                },
                RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    ..launched.clone()
                },
            ] {
                assert!(
                    change_run(store, RunAuthority::Scheduler(&lease), &invalid)
                        .await
                        .is_err(),
                    "terminal authority remains narrow: {invalid:?}"
                );
            }
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("unchanged terminal projection")
                    .into_iter()
                    .find(|run| run.run_id == started.run_id)
                    .expect("original run"),
                projected
            );
            let recorded = read_recall_events(store, 0, 100)
                .await
                .expect("late fact pair");
            assert_eq!(
                recorded
                    .iter()
                    .filter(|row| row["event_type"] == "platform_launch_failed")
                    .count(),
                2
            );
            expire_lease(store).await;
            assert!(
                matches!(
                    change_run(store, RunAuthority::Scheduler(&lease), &late_failure).await,
                    Err(ControlError::Fenced)
                ),
                "even duplicate observations require a live leader"
            );
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect successor")
                .expect("successor");
            let following = claim_next_run(store, &lease, &limits, None)
                .await
                .expect("claim following run")
                .expect("released capacity");
            assert_eq!(following.request.run_id, Uuid::from_u128(2));
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_unclaimed_work_when_an_operator_cancels_then_a_terminal_fact_prevents_launch_and_preserves_the_actor()
 {
    let test_cases = [
        RunLifecycleTestCase {
            description: "SQLite cancellation before launch preserves the operator and terminal fact",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Cancelled,
        },
        RunLifecycleTestCase {
            description: "PostgreSQL cancellation before launch preserves the operator and terminal fact",
            backend: Backend::Postgres,
            expected_status: RunStatus::Cancelled,
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
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("snapshot");
            queue(
                store,
                &request((location, published.snapshot_id), 1, 0, &["orders"]),
            )
            .await;
            let request = RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: Uuid::from_u128(1),
                claim_id: None,
                change: RunChange::CancelRequested {
                    actor: "test-operator".into(),
                    reason: "Cancel queued orders".into(),
                },
            };
            let operator = AuditContext {
                actor: "test-operator".into(),
                granting_role: "operator".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let other = AuditContext {
                actor: "another-operator".into(),
                ..operator.clone()
            };
            assert!(
                change_run(store, RunAuthority::Operator(&other), &request)
                    .await
                    .is_err()
            );
            let before_events = read_recall_events(store, 0, 100)
                .await
                .expect("facts before audit failure");
            let point = TransactionFault {
                description: "cancellation audit failure rolls back terminal facts and projections",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected cancellation audit failure')",
                postgres_body: "RAISE EXCEPTION 'injected cancellation audit failure'",
            };
            install_fault(store, &point).await;
            assert!(
                change_run(store, RunAuthority::Operator(&operator), &request)
                    .await
                    .is_err(),
                "{}",
                point.description
            );
            remove_fault(store, &point).await;
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("cancel rollback")
                    .remove(0)
                    .status,
                RunStatus::Queued
            );
            assert_eq!(
                read_recall_events(store, 0, 100)
                    .await
                    .expect("fact rollback"),
                before_events
            );
            assert!(
                change_run(store, RunAuthority::Operator(&operator), &request)
                    .await
                    .expect("cancel before launch")
            );
            assert!(
                !change_run(store, RunAuthority::Operator(&operator), &request)
                    .await
                    .expect("retry cancellation")
            );
            let mut changed = request.clone();
            changed.change = RunChange::CancelRequested {
                actor: operator.actor.clone(),
                reason: "Different cancellation reason".into(),
            };
            assert!(
                change_run(store, RunAuthority::Operator(&operator), &changed)
                    .await
                    .is_err(),
                "retry identity includes the cancellation reason"
            );
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            assert!(
                claim_next_run(store, &lease, &BTreeMap::new(), None)
                    .await
                    .expect("cancelled work cannot launch")
                    .is_none()
            );
            let projected = list_runs(store, (location, "development"), 100)
                .await
                .expect("cancelled projection")
                .remove(0);
            assert_eq!(
                projected.status, test_case.expected_status,
                "{}",
                test_case.description
            );
            assert_eq!(
                projected.node_counts_json,
                BTreeMap::from([("cancelled".into(), 1)])
            );
            let events = read_recall_events(store, 0, 100)
                .await
                .expect("immutable cancellation facts");
            assert_eq!(
                events
                    .iter()
                    .map(|row| row["event_type"].as_str().expect("type"))
                    .collect::<Vec<_>>(),
                [
                    "run_requested",
                    "run_queued",
                    "run_cancel_requested",
                    "run_terminated"
                ]
            );
            assert_eq!(
                events[2]["payload"]["change"]["change"]["actor"],
                "test-operator"
            );
            close_test_store(fixture).await;
        });
    }
}
