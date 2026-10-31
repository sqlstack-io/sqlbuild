use std::collections::BTreeSet;
use std::time::Duration;

use sqlbuild_orchestration::platform::main::run_watchdog::run_watchdog;
use sqlbuild_orchestration::platform::models::PlatformEvent;
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::helpers::{snapshot, wait_for_scans};
use crate::location_helpers::create_location;
use crate::replica_helpers::second_store;
use crate::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::WatchdogTestCase;

#[test]
fn given_two_server_replicas_when_scheduling_is_stalled_then_health_facts_are_durable_deduplicated_and_independent_of_leadership()
 {
    let test_cases = [
        WatchdogTestCase {
            description: "SQLite without a leader",
            backend: Backend::Sqlite,
            leaders: 0,
            expected_no_leader: true,
            expected_overdue: 102,
            expected_low_horizons: 1,
        },
        WatchdogTestCase {
            description: "PostgreSQL without a leader",
            backend: Backend::Postgres,
            leaders: 0,
            expected_no_leader: true,
            expected_overdue: 102,
            expected_low_horizons: 1,
        },
        WatchdogTestCase {
            description: "SQLite with a frozen leader",
            backend: Backend::Sqlite,
            leaders: 1,
            expected_no_leader: false,
            expected_overdue: 102,
            expected_low_horizons: 1,
        },
        WatchdogTestCase {
            description: "PostgreSQL with a frozen leader",
            backend: Backend::Postgres,
            leaders: 1,
            expected_no_leader: false,
            expected_overdue: 102,
            expected_low_horizons: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate health store");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input
                .documents
                .get_mut("triggers")
                .expect("trigger document")["removed_orders"] =
                input.documents["triggers"]["expired_orders"].clone();
            publish_snapshot(store, &input)
                .await
                .expect("old definition");
            input.generation = 2;
            input
                .documents
                .get_mut("triggers")
                .expect("trigger document")
                .as_object_mut()
                .expect("trigger map")
                .remove("removed_orders");
            publish_snapshot(store, &input)
                .await
                .expect("current definition");
            for _ in 0..test_case.leaders {
                acquire_lease(store, Uuid::now_v7())
                    .await
                    .expect("acquire lease")
                    .expect("leader");
            }
            let second = second_store(store).await;
            let replicas = BTreeSet::from([Uuid::now_v7(), Uuid::now_v7()]);
            let (stop, shutdown) = tokio::sync::watch::channel(false);
            let tasks: Vec<_> = [store.clone(), second.clone()]
                .into_iter()
                .zip(replicas.iter())
                .map(|(store, id)| {
                    let id = *id;
                    let shutdown = shutdown.clone();
                    tokio::spawn(async move { run_watchdog(&store, id, shutdown).await })
                })
                .collect();
            let observations = wait_for_scans(store, &replicas, &tasks).await;
            let overdue: BTreeSet<_> = observations
                .iter()
                .filter_map(|row| {
                    let PlatformEvent::TickOverdue { trigger_id, .. } = row.payload.platform else {
                        return None;
                    };
                    Some(trigger_id)
                })
                .collect();
            assert_eq!(
                overdue.len(),
                test_case.expected_overdue,
                "{}",
                test_case.description
            );
            let horizons: BTreeSet<_> = observations
                .iter()
                .filter_map(|row| {
                    let PlatformEvent::CalendarHorizonLow { trigger_id, .. } = row.payload.platform
                    else {
                        return None;
                    };
                    Some(trigger_id)
                })
                .collect();
            assert_eq!(
                horizons.len(),
                test_case.expected_low_horizons,
                "paused and removed calendars are excluded; refresh cadence alone is not a low horizon"
            );
            assert_eq!(
                observations
                    .iter()
                    .any(|row| matches!(row.payload.platform, PlatformEvent::SchedulerNoLeader)),
                test_case.expected_no_leader
            );
            let logical_keys: BTreeSet<_> = observations
                .iter()
                .map(|row| {
                    (
                        row.occurred_at,
                        serde_json::to_string(&row.payload.platform).expect("event identity"),
                    )
                })
                .collect();
            assert_eq!(
                logical_keys.len(),
                observations.len(),
                "replicas do not duplicate an observation within its window"
            );
            assert!(
                observations
                    .iter()
                    .all(|row| row.occurred_at.timestamp().rem_euclid(30) == 0)
            );
            stop.send(true).expect("shutdown both replicas");
            for task in tasks {
                tokio::time::timeout(Duration::from_secs(2), task)
                    .await
                    .expect("prompt shutdown")
                    .expect("join watchdog")
                    .expect("watchdog stopped");
            }
            close_store(second).await;
            close_test_store(fixture).await;
        });
    }
}
