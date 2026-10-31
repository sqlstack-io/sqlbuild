//! Real scheduler cycles detect silent workers without surrendering their physical write claims.

use std::collections::BTreeMap;
use std::time::Duration;

use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::heartbeat_run::heartbeat_run;
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::runs::main::monitor_runs::monitor_runs;
use sqlbuild_orchestration::runs::models::{
    RunAuthority, RunChange, RunChangeRequest, RunOutcome, RunStatus,
};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::scheduler::main::renew_lease::renew_lease;
use sqlbuild_orchestration::scheduler::main::scheduler_cycle::scheduler_cycle;
use sqlbuild_orchestration::scheduler::models::SchedulerState;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::read_recall_events::read_recall_events;
use uuid::Uuid;

use crate::fault_helpers::{install_fault, remove_fault};
use crate::helpers::{queue, request, snapshot};
use crate::lease_helpers::expire_lease;
use crate::location_helpers::create_location;
use crate::monitor_helpers::{age_heartbeat, start_run};
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::{MonitorBacklogTestCase, RunMonitorTestCase, TransactionFault};

#[test]
fn given_silent_and_live_workers_when_the_scheduler_monitors_then_fenced_atomic_facts_retain_claims_and_allow_recovery()
 {
    let test_cases = [
        RunMonitorTestCase {
            description: "SQLite observes only running workers with expired heartbeats",
            backend: Backend::Sqlite,
            expected_statuses: [
                RunStatus::Unresponsive,
                RunStatus::Running,
                RunStatus::Launched,
                RunStatus::Failed,
                RunStatus::Queued,
            ],
        },
        RunMonitorTestCase {
            description: "PostgreSQL observes only running workers with expired heartbeats",
            backend: Backend::Postgres,
            expected_statuses: [
                RunStatus::Unresponsive,
                RunStatus::Running,
                RunStatus::Launched,
                RunStatus::Failed,
                RunStatus::Queued,
            ],
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
            let holder = Uuid::now_v7();
            let lease = acquire_lease(store, holder)
                .await
                .expect("elect")
                .expect("leader");
            let silent = start_run(store, &lease, &request(scope, 1, 0, &["orders"])).await;
            start_run(store, &lease, &request(scope, 2, 0, &["customers"])).await;
            age_heartbeat(store, Uuid::from_u128(2), 40).await;
            queue(store, &request(scope, 3, 0, &["inventory"])).await;
            let launching = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("launching");
            change_run(
                store,
                RunAuthority::Scheduler(&lease),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: launching.request.run_id,
                    claim_id: Some(launching.claim_id),
                    change: RunChange::Launched {
                        container_id: Some("orders-worker".into()),
                    },
                },
            )
            .await
            .expect("launcher acknowledges");
            let failed = start_run(store, &lease, &request(scope, 4, 0, &["products"])).await;
            change_run(
                store,
                RunAuthority::Worker {
                    run_id: failed.request.run_id,
                    claim_id: failed.claim_id,
                },
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: failed.request.run_id,
                    claim_id: Some(failed.claim_id),
                    change: RunChange::Finished {
                        outcome: RunOutcome::Failed,
                        reason: "worker failed".into(),
                        exit_code: Some(1),
                    },
                },
            )
            .await
            .expect("terminal run");
            queue(store, &request(scope, 5, 0, &["orders"])).await;
            age_heartbeat(store, silent.request.run_id, 50).await;
            age_heartbeat(store, launching.request.run_id, 50).await;
            let before = read_recall_events(store, 0, 100)
                .await
                .expect("before monitoring");
            let projection = list_runs(store, (location, "development"), 100)
                .await
                .expect("before monitoring");
            let fault = TransactionFault {
                description: "monitor projection failure rolls back the observation",
                table: "sqb_runs",
                operation: "UPDATE",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected monitor failure')",
                postgres_body: "RAISE EXCEPTION 'injected monitor failure'",
            };
            install_fault(store, &fault).await;
            assert!(
                monitor_runs(store, &lease).await.is_err(),
                "{}",
                test_case.description
            );
            remove_fault(store, &fault).await;
            assert_eq!(
                read_recall_events(store, 0, 100)
                    .await
                    .expect("fact rollback"),
                before
            );
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("projection rollback"),
                projection
            );
            expire_lease(store).await;
            let (state, observed) = scheduler_cycle(store, SchedulerState::new(holder))
                .await
                .expect("monitoring cycle");
            assert_eq!(observed.unresponsive, 1, "{}", test_case.description);
            let statuses: BTreeMap<_, _> = list_runs(store, (location, "development"), 100)
                .await
                .expect("monitored statuses")
                .into_iter()
                .map(|record| (record.run_id, record.status))
                .collect();
            assert_eq!(
                std::array::from_fn(|index| statuses[&Uuid::from_u128(index as u128 + 1)]),
                test_case.expected_statuses,
                "{}",
                test_case.description
            );
            let recorded = read_recall_events(store, 0, 100)
                .await
                .expect("observation");
            assert_eq!(recorded.len(), before.len() + 1);
            assert_eq!(
                recorded.last().expect("fact")["event_type"],
                "run_unresponsive"
            );
            let (state, repeated) = scheduler_cycle(store, state)
                .await
                .expect("repeated monitor");
            assert_eq!(repeated.unresponsive, 0);
            assert_eq!(
                read_recall_events(store, 0, 100)
                    .await
                    .expect("no repeated facts"),
                recorded
            );
            let (_, follower) = scheduler_cycle(store, SchedulerState::new(Uuid::now_v7()))
                .await
                .expect("follower");
            assert!(!follower.leader);
            assert_eq!(follower.unresponsive, 0);
            assert!(
                !heartbeat_run(store, (silent.request.run_id, silent.claim_id))
                    .await
                    .expect("worker recovers")
            );
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("recovered projection")
                    .into_iter()
                    .find(|record| record.run_id == silent.request.run_id)
                    .expect("recovered run")
                    .status,
                RunStatus::Running
            );
            age_heartbeat(store, silent.request.run_id, 50).await;
            let (_, second_loss) = scheduler_cycle(store, state)
                .await
                .expect("new heartbeat loss");
            assert_eq!(second_loss.unresponsive, 1);
            let events = read_recall_events(store, 0, 100)
                .await
                .expect("successive losses");
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event["event_type"] == "run_unresponsive")
                    .count(),
                2
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event["event_type"] == "run_recovered")
                    .count(),
                1
            );
            expire_lease(store).await;
            assert!(matches!(
                monitor_runs(store, &lease).await,
                Err(ControlError::Fenced)
            ));
            assert_eq!(
                read_recall_events(store, 0, 100)
                    .await
                    .expect("fenced monitor"),
                events
            );
            let successor = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("takeover")
                .expect("successor");
            assert!(
                claim_next_run(store, &successor, &BTreeMap::new(), None)
                    .await
                    .expect("write claims remain held after heartbeat loss and takeover")
                    .is_none()
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_silent_worker_backlog_when_monitoring_then_bounded_transactions_continue_immediately_until_drained()
 {
    let test_cases = [
        MonitorBacklogTestCase {
            description: "SQLite drains more than one monitor page",
            backend: Backend::Sqlite,
            run_count: 105,
            expected_batches: [100, 5, 0],
        },
        MonitorBacklogTestCase {
            description: "PostgreSQL drains more than one monitor page",
            backend: Backend::Postgres,
            run_count: 105,
            expected_batches: [100, 5, 0],
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
            let holder = Uuid::now_v7();
            let mut lease = acquire_lease(store, holder)
                .await
                .expect("elect")
                .expect("leader");
            for index in 0..test_case.run_count {
                let mut input = request(
                    (location, published.snapshot_id),
                    index as u128 + 1,
                    0,
                    &["orders"],
                );
                input.write_set.clear();
                start_run(store, &lease, &input).await;
                age_heartbeat(store, input.run_id, 50).await;
                lease = renew_lease(store, &lease)
                    .await
                    .expect("renew during setup");
            }
            expire_lease(store).await;
            let mut state = SchedulerState::new(holder);
            let mut batches = Vec::new();
            for expected in test_case.expected_batches {
                let cycle;
                (state, cycle) = scheduler_cycle(store, state).await.expect("monitor batch");
                batches.push(cycle.unresponsive);
                assert_eq!(
                    cycle.wait_for,
                    if expected == 100 {
                        Duration::ZERO
                    } else {
                        Duration::from_secs(5)
                    }
                );
            }
            assert_eq!(
                batches, test_case.expected_batches,
                "{}",
                test_case.description
            );
            let events = read_recall_events(store, 0, 1000)
                .await
                .expect("monitor facts");
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event["event_type"] == "run_unresponsive")
                    .count(),
                test_case.run_count
            );
            close_test_store(fixture).await;
        });
    }
}
