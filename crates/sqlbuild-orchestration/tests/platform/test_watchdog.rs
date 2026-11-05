use std::collections::BTreeSet;
use std::time::Duration;

use crate::platform::_helpers::retention::retain;
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::main::run_maintenance::run_maintenance;
use crate::platform::models::{LocationHealth, LocationHealthReport, PlatformEvent};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::query::query;
use crate::store::models::{Parameter, TransactionMode};
use chrono::TimeDelta;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::platform::tests::helpers::{retained_platform_facts, snapshot, wait_for_scans};
use crate::platform::tests::test_types::{LocationWatchdogTestCase, WatchdogTestCase};
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

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
                    tokio::spawn(async move {
                        run_maintenance((&store, id), (Default::default(), &[]), (|_| {}, shutdown))
                            .await
                    })
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
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("age health facts");
            let aged = transaction.now - TimeDelta::days(3);
            let (transaction, _) = query(
                transaction,
                "UPDATE recall_events SET occurred_at=$1 WHERE producer='platform'",
                vec![Parameter::Time(aged)],
            )
            .await
            .expect("aged health facts");
            commit(transaction).await.expect("aged facts committed");
            retain(store, Default::default(), &[])
                .await
                .expect("retention pass");
            let alerts = observations
                .iter()
                .filter(|row| !matches!(row.payload.platform, PlatformEvent::Heartbeat { .. }))
                .count();
            let remaining = retained_platform_facts(store).await;
            assert_eq!(
                remaining
                    .iter()
                    .filter(|row| matches!(row.payload.platform, PlatformEvent::Heartbeat { .. }))
                    .count(),
                0,
                "{}: expired replica heartbeats are pruned",
                test_case.description
            );
            assert!(
                remaining.len() >= alerts,
                "{}: alert facts outlive the heartbeat horizon for webhooks and sensors",
                test_case.description
            );
            close_store(second).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_stale_location_sessions_when_watchdogs_scan_then_latest_target_liveness_controls_paged_outage_events()
 {
    let test_cases = [
        LocationWatchdogTestCase {
            description: "SQLite latest target receipts",
            backend: Backend::Sqlite,
            expected_broken: 104,
        },
        LocationWatchdogTestCase {
            description: "PostgreSQL latest target receipts",
            backend: Backend::Postgres,
            expected_broken: 104,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate store");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let archived = create_location(store, "inventory", "test-admin")
                .await
                .expect("archived location");
            create_location(store, "products", "test-admin")
                .await
                .expect("never observed location");
            for (location_id, count) in [(location, 105), (archived, 1)] {
                for index in 0..count {
                    for _ in 0..2 {
                        report_location_health(
                            store,
                            location_id,
                            &LocationHealthReport {
                                event_id: Uuid::now_v7(),
                                session_id: Uuid::now_v7(),
                                target: format!("orders_{index:03}"),
                                health: LocationHealth::Heartbeat {},
                            },
                        )
                        .await
                        .expect("register old session");
                    }
                }
            }
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("age sessions");
            let stale_at = transaction.now - TimeDelta::seconds(105);
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_location_instances SET last_seen_at=$1",
                vec![Parameter::Time(stale_at)],
            )
            .await
            .expect("age all old sessions");
            let now = transaction.now;
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_locations SET archived_at=$1 WHERE location_id=$2",
                vec![Parameter::Time(now), Parameter::Id(archived)],
            )
            .await
            .expect("archive location");
            commit(transaction).await.expect("commit fixture");
            // A fresh successor protects only its target, not another target of the same location.
            let healthy_session = Uuid::now_v7();
            report_location_health(
                store,
                location,
                &LocationHealthReport {
                    event_id: Uuid::now_v7(),
                    session_id: healthy_session,
                    target: "orders_104".into(),
                    health: LocationHealth::Heartbeat {},
                },
            )
            .await
            .expect("healthy successor");
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("age successor");
            let recent_at = transaction.now - TimeDelta::seconds(75);
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_location_instances SET last_seen_at=$1 WHERE instance_id=$2",
                vec![Parameter::Time(recent_at), Parameter::Id(healthy_session)],
            )
            .await
            .expect("two missed heartbeats");
            commit(transaction).await.expect("commit successor time");
            let second = second_store(store).await;
            let replicas = BTreeSet::from([Uuid::now_v7(), Uuid::now_v7()]);
            let (stop, shutdown) = tokio::sync::watch::channel(false);
            let tasks: Vec<_> = [store.clone(), second.clone()]
                .into_iter()
                .zip(replicas.iter())
                .map(|(store, id)| {
                    let id = *id;
                    let shutdown = shutdown.clone();
                    tokio::spawn(async move {
                        run_maintenance((&store, id), (Default::default(), &[]), (|_| {}, shutdown))
                            .await
                    })
                })
                .collect();
            let observations = wait_for_scans(store, &replicas, &tasks).await;
            let broken: BTreeSet<_> = observations
                .iter()
                .filter_map(|row| {
                    let PlatformEvent::LocationBroken {
                        location_id,
                        ref target,
                        last_seen_at,
                    } = row.payload.platform
                    else {
                        return None;
                    };
                    assert_eq!(
                        location_id, location,
                        "archived and never-observed locations are excluded"
                    );
                    assert_eq!(
                        last_seen_at, stale_at,
                        "payload retains the latest server receipt time"
                    );
                    Some(target.clone())
                })
                .collect();
            assert_eq!(
                broken,
                (0..test_case.expected_broken)
                    .map(|index| format!("orders_{index:03}"))
                    .collect(),
                "{}: both pages are read, old replicas deduplicate, and the healthy target is excluded",
                test_case.description
            );
            let keys: BTreeSet<_> = observations
                .iter()
                .map(|row| {
                    (
                        row.occurred_at,
                        serde_json::to_string(&row.payload.platform).expect("identity"),
                    )
                })
                .collect();
            assert_eq!(
                keys.len(),
                observations.len(),
                "replicas share observation keys"
            );
            stop.send(true).expect("stop replicas");
            for task in tasks {
                tokio::time::timeout(Duration::from_secs(2), task)
                    .await
                    .expect("prompt shutdown")
                    .expect("join")
                    .expect("watchdog stopped");
            }
            close_store(second).await;
            close_test_store(fixture).await;
        });
    }
}
