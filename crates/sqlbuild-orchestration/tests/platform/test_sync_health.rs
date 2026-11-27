//! Health reads carry the server's verdict, sync and stall, and work for a silent location is refused at once.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::api::tests::helpers::check_in;
use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::models::ServerPoolCaps;
use crate::platform::main::health::read_person_location_health::read_person_location_health;
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::main::person_reads::read_person_problems::read_person_problems;
use crate::platform::main::work::request_location_work::request_location_work;
use crate::platform::main::work::request_republish::request_republish;
use crate::platform::models::{
    LocationFact, LocationHealth, LocationHealthReport, LocationVerdict, LocationWork,
    LocationWorkRequest, PersonWorkRequest, ProblemTier,
};
use crate::platform::tests::helpers::{one_schedule_snapshot, sync};
use crate::platform::tests::test_types::{ProblemVerdictTestCase, SyncHealthTestCase};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_location_when_it_is_silent_syncing_stalled_or_disconnected_then_the_server_says_so_once()
{
    let test_cases = [
        SyncHealthTestCase {
            description: "SQLite sync health",
            backend: Backend::Sqlite,
            expected_silent_refusal: "The location isn't checking in (last check-in 6m ago), so it can't test its connection. Start it, then try again.",
            expected_stall_tier: ProblemTier::NeedsAttention,
            expected_connection_tier: ProblemTier::StopsRuns,
        },
        SyncHealthTestCase {
            description: "PostgreSQL sync health",
            backend: Backend::Postgres,
            expected_silent_refusal: "The location isn't checking in (last check-in 6m ago), so it can't test its connection. Start it, then try again.",
            expected_stall_tier: ProblemTier::NeedsAttention,
            expected_connection_tier: ProblemTier::StopsRuns,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("admin");
            let location = create_location(store, "orders", "test-owner")
                .await
                .expect("location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let ask = |work: LocationWork| LocationWorkRequest {
                request_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                reason: "Catch up run history".into(),
                work,
            };
            let refused = request_location_work(
                store,
                (&context, Some(admin)),
                &ask(LocationWork::SyncWarehouse {}),
            )
            .await;
            assert_eq!(
                refused.map_err(|error| error.to_string()),
                Err("The location hasn't checked in yet, so it can't sync. Start it, then try again.".to_owned()),
                "{}: a sync for a location that never checked in is refused at once",
                test_case.description
            );
            check_in(store, location, "development").await;
            let transaction = begin(store, TransactionMode::Write).await.expect("age");
            let silent_since = transaction.now - TimeDelta::seconds(370);
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_location_instances SET last_seen_at=$1",
                vec![Parameter::Time(silent_since)],
            )
            .await
            .expect("silent instance");
            commit(transaction).await.expect("aged check-in");
            let refused = request_location_work(
                store,
                (&context, Some(admin)),
                &ask(LocationWork::CheckConnection {}),
            )
            .await;
            assert_eq!(
                refused.map_err(|error| error.to_string()),
                Err(test_case.expected_silent_refusal.to_owned()),
                "{}: a connection test names how long the location has been silent",
                test_case.description
            );
            let republish = request_republish(
                store,
                (&context, Some(admin)),
                (location, "development"),
                &PersonWorkRequest {
                    request_id: Uuid::now_v7(),
                    reason: "Retry the deploy".into(),
                },
            )
            .await;
            assert!(
                matches!(republish, Err(ControlError::NotCheckingIn(_))),
                "{}: a republish of a silent location is refused",
                test_case.description
            );
            let session = Uuid::now_v7();
            let report = |health: LocationHealth| LocationHealthReport {
                event_id: Uuid::now_v7(),
                session_id: session,
                target: "development".into(),
                health,
            };
            for health in [
                LocationHealth::Heartbeat {},
                LocationHealth::Connection {
                    generation: published.generation,
                    healthy: true,
                    trigger: None,
                    elapsed_millis: None,
                },
            ] {
                report_location_health(store, location, &report(health))
                    .await
                    .expect("live location");
            }
            assert!(
                request_location_work(
                    store,
                    (&context, Some(admin)),
                    &ask(LocationWork::SyncWarehouse {})
                )
                .await
                .expect("sync asked of a live location"),
                "{}: a live location takes the request",
                test_case.description
            );
            report_location_health(
                store,
                location,
                &report(sync(0, None, (None, false)).report.health),
            )
            .await
            .expect("sync progress");
            let caps = ServerPoolCaps::default();
            let health = || {
                read_person_location_health(
                    store,
                    (&context, admin),
                    (location, "development"),
                    (DefaultRole::None, &caps),
                )
            };
            let read = health().await.expect("running sync health");
            assert_eq!(
                (
                    read.warehouse_sync
                        .requested
                        .map(|requested| requested.requested_by),
                    read.warehouse_sync.running.map(|running| running.stalled),
                    read.verdict,
                ),
                (
                    Some("orders-admin".to_owned()),
                    Some(false),
                    LocationVerdict {
                        fact: LocationFact::Healthy,
                        tier: None
                    },
                ),
                "{}: a running sync and the request waiting for it are no problem",
                test_case.description
            );
            let transaction = begin(store, TransactionMode::Write).await.expect("stall");
            let progressed_at = transaction.now - TimeDelta::minutes(11);
            let (transaction, _) = query(
                transaction,
                "UPDATE recall_events SET occurred_at=$1 WHERE event_type='location_health' \
                 AND payload->'report'->'health'->>'kind'='warehouse_sync'",
                vec![Parameter::Time(progressed_at)],
            )
            .await
            .expect("stalled progress");
            commit(transaction).await.expect("aged progress");
            let read = health().await.expect("stalled sync health");
            let problems = read_person_problems(store, admin, DefaultRole::None)
                .await
                .expect("stalled problems");
            assert_eq!(
                (
                    read.warehouse_sync.running.map(|running| running.stalled),
                    read.verdict.fact,
                    problems
                        .stalled_syncs
                        .iter()
                        .map(|stalled| stalled.tier)
                        .collect::<Vec<_>>(),
                ),
                (
                    Some(true),
                    LocationFact::SyncStalled,
                    vec![test_case.expected_stall_tier]
                ),
                "{}: eleven minutes without progress is a stall that needs attention",
                test_case.description
            );
            report_location_health(
                store,
                location,
                &report(LocationHealth::Connection {
                    generation: published.generation,
                    healthy: false,
                    trigger: None,
                    elapsed_millis: None,
                }),
            )
            .await
            .expect("failed connection check");
            let read = health().await.expect("disconnected health");
            let problems = read_person_problems(store, admin, DefaultRole::None)
                .await
                .expect("disconnected problems");
            assert_eq!(
                (
                    read.verdict,
                    problems
                        .failing_locations
                        .iter()
                        .map(|failing| failing.tier)
                        .collect::<Vec<_>>(),
                ),
                (
                    LocationVerdict {
                        fact: LocationFact::CantConnect,
                        tier: Some(test_case.expected_connection_tier),
                    },
                    vec![test_case.expected_connection_tier]
                ),
                "{}: a failed connection check stops runs, ahead of the stalled sync",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_location_whose_only_open_problem_is_an_overdue_schedule_when_read_then_it_needs_attention()
 {
    let overdue = LocationVerdict {
        fact: LocationFact::ScheduleOverdue,
        tier: Some(ProblemTier::NeedsAttention),
    };
    let calm = LocationVerdict {
        fact: LocationFact::Healthy,
        tier: None,
    };
    let test_cases = [
        ProblemVerdictTestCase {
            description: "SQLite overdue schedule",
            backend: Backend::Sqlite,
            expected_overdue: overdue,
            expected_tick_tier: ProblemTier::NeedsAttention,
            expected_calm: calm,
        },
        ProblemVerdictTestCase {
            description: "PostgreSQL overdue schedule",
            backend: Backend::Postgres,
            expected_overdue: overdue,
            expected_tick_tier: ProblemTier::NeedsAttention,
            expected_calm: calm,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("admin");
            let late = create_location(store, "orders", "test-owner")
                .await
                .expect("late location");
            let quiet = create_location(store, "inventory", "test-owner")
                .await
                .expect("quiet location");
            for (location, input) in [
                (late, one_schedule_snapshot(late)),
                (quiet, snapshot(quiet)),
            ] {
                let published = publish_snapshot(store, &input).await.expect("publish");
                let session = Uuid::now_v7();
                for health in [
                    LocationHealth::Heartbeat {},
                    LocationHealth::Connection {
                        generation: published.generation,
                        healthy: true,
                        trigger: None,
                        elapsed_millis: None,
                    },
                ] {
                    report_location_health(
                        store,
                        location,
                        &LocationHealthReport {
                            event_id: Uuid::now_v7(),
                            session_id: session,
                            target: "development".into(),
                            health,
                        },
                    )
                    .await
                    .expect("live, connected location");
                }
            }
            let transaction = begin(store, TransactionMode::Write).await.expect("late");
            let due = transaction.now - TimeDelta::minutes(5);
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_trigger_state SET next_due_at=$1 \
                 WHERE trigger_id IN (SELECT trigger_id FROM sqb_triggers WHERE location_id=$2)",
                vec![Parameter::Time(due), Parameter::Id(late)],
            )
            .await
            .expect("overdue schedule");
            commit(transaction).await.expect("late schedule");
            let caps = ServerPoolCaps::default();
            let mut verdicts = Vec::new();
            for location in [late, quiet] {
                verdicts.push(
                    read_person_location_health(
                        store,
                        (&context, admin),
                        (location, "development"),
                        (DefaultRole::None, &caps),
                    )
                    .await
                    .expect("health")
                    .verdict,
                );
            }
            let problems = read_person_problems(store, admin, DefaultRole::None)
                .await
                .expect("problems");
            assert_eq!(
                (
                    verdicts,
                    problems
                        .overdue_ticks
                        .iter()
                        .map(|tick| (tick.location_id, tick.tier))
                        .collect::<Vec<_>>(),
                ),
                (
                    vec![test_case.expected_overdue, test_case.expected_calm],
                    vec![(late, test_case.expected_tick_tier)],
                ),
                "{}: only the location with an open problem needs attention, and its tick says so too",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
