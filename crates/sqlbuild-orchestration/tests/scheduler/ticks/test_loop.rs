//! Embedded and standalone schedulers share takeover and shutdown semantics.

use std::time::Duration;

use sqlbuild_orchestration::scheduler::main::run_scheduler::run_scheduler;
use sqlbuild_orchestration::scheduler::main::scheduler_cycle::scheduler_cycle;
use sqlbuild_orchestration::scheduler::models::SchedulerState;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::lease_helpers::expire_lease;
use crate::location_helpers::create_location;
use crate::replica_helpers::second_store;
use crate::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::ticks::helpers::snapshot;
use crate::ticks::test_types::{IdleLauncher, TickTransactionTestCase};

#[test]
fn given_embedded_and_standalone_schedulers_when_taking_over_then_only_one_drains_due_work_and_idle_shutdown_is_prompt()
 {
    let test_cases = [
        TickTransactionTestCase {
            description: "SQLite scheduler loop",
            backend: Backend::Sqlite,
            expected_ticks: 3,
            expected_runs: 1,
        },
        TickTransactionTestCase {
            description: "PostgreSQL scheduler loop",
            backend: Backend::Postgres,
            expected_ticks: 3,
            expected_runs: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate loop store");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish due schedule");
            let second = second_store(store).await;
            let first_state = SchedulerState::new(Uuid::now_v7());
            let second_state = SchedulerState::new(Uuid::now_v7());
            let (first_state, first) = scheduler_cycle(store, first_state)
                .await
                .expect("initial cycle");
            assert!(first.leader, "{}", test_case.description);
            assert_eq!(first.evaluated, 1);
            assert_eq!(first.ticks, test_case.expected_ticks);
            assert_eq!(first.wait_for, Duration::from_secs(5));
            let (second_state, follower) = scheduler_cycle(&second, second_state)
                .await
                .expect("follower cycle");
            assert!(!follower.leader);
            assert_eq!(follower.ticks, 0);
            expire_lease(store).await;
            let (second_state, successor) = scheduler_cycle(&second, second_state)
                .await
                .expect("takeover cycle");
            assert!(successor.leader);
            assert_eq!(successor.ticks, 0, "takeover never repeats durable work");
            assert!(
                !scheduler_cycle(store, first_state)
                    .await
                    .expect("fenced cycle")
                    .1
                    .leader
            );
            let runs = sqlbuild_orchestration::runs::main::list_runs::list_runs(
                store,
                (location, "development"),
                100,
            )
            .await
            .expect("queued run");
            assert_eq!(runs.len(), test_case.expected_runs);
            let (stop, shutdown) = tokio::sync::watch::channel(false);
            let (_changed, changes) = tokio::sync::watch::channel(());
            let mut running = Box::pin(run_scheduler(
                &second,
                second_state,
                (shutdown, changes),
                IdleLauncher,
            ));
            tokio::select! {
                result = &mut running => panic!("loop ended before shutdown: {result:?}"),
                _ = tokio::time::sleep(Duration::from_millis(50)) => {},
            }
            stop.send(true).expect("signal shutdown");
            tokio::time::timeout(Duration::from_secs(2), running)
                .await
                .expect("idle shutdown does not wait for the five-second timer")
                .expect("scheduler exits cleanly");
            close_store(second).await;
            close_test_store(fixture).await;
        });
    }
}
