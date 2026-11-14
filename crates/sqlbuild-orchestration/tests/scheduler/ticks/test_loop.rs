//! Embedded and standalone schedulers share takeover and shutdown semantics.

use std::time::Duration;

use crate::scheduler::main::_scheduler_cycle::scheduler_cycle;
use crate::scheduler::main::run_scheduler::run_scheduler;
use crate::scheduler::models::SchedulerState;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::scheduler::tests::contracts::ticks::helpers::snapshot;
use crate::scheduler::tests::contracts::ticks::test_types::{
    FencedLoopTestCase, FencingLauncher, IdleLauncher, TickTransactionTestCase,
};
use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_embedded_and_standalone_schedulers_when_taking_over_then_only_one_drains_due_work_and_idle_shutdown_is_prompt()
 {
    let test_cases = [
        TickTransactionTestCase {
            description: "SQLite scheduler loop",
            backend: Backend::Sqlite,
            expected_ticks: 1,
            expected_runs: 1,
        },
        TickTransactionTestCase {
            description: "PostgreSQL scheduler loop",
            backend: Backend::Postgres,
            expected_ticks: 1,
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
            let runs =
                crate::runs::main::list_runs::list_runs(store, (location, "development"), 100)
                    .await
                    .expect("queued run");
            assert_eq!(runs.len(), test_case.expected_runs);
            let (stop, shutdown) = tokio::sync::watch::channel(false);
            let mut running =
                Box::pin(run_scheduler(&second, second_state, shutdown, IdleLauncher));
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

#[test]
fn given_a_leader_fenced_mid_cycle_when_the_loop_continues_then_it_rejoins_the_election_instead_of_stopping()
 {
    let test_cases = [
        FencedLoopTestCase {
            description: "SQLite scheduler loop",
            backend: Backend::Sqlite,
            expected_rejoined: true,
        },
        FencedLoopTestCase {
            description: "PostgreSQL scheduler loop",
            backend: Backend::Postgres,
            expected_rejoined: true,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate loop store");
            let mut epochs = Vec::new();
            let (stop, shutdown) = tokio::sync::watch::channel(false);

            let (finished, ()) = tokio::join!(
                run_scheduler(
                    store,
                    SchedulerState::new(Uuid::now_v7()),
                    shutdown,
                    FencingLauncher {
                        epochs: &mut epochs
                    }
                ),
                async {
                    tokio::time::sleep(Duration::from_millis(2_500)).await;
                    stop.send_replace(true);
                }
            );

            finished.expect("a fenced leader keeps the scheduler running");
            assert_eq!(
                epochs.windows(2).any(|pair| pair[1] > pair[0]),
                test_case.expected_rejoined,
                "{}: leads again under a newer epoch, polled with {epochs:?}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
