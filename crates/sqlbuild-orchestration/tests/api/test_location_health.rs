use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::main::tokens::issue_token::issue_token;
use crate::credentials::models::{AuditContext, CredentialScope, DefaultRole};
use crate::platform::main::health::read_person_location_health::read_person_location_health;
use crate::platform::main::health::report_remote_location_health::report_remote_location_health;
use crate::platform::models::{
    ConnectionCheckTrigger, LocationHealth, LocationHealthObservation, LocationHealthReport,
    LocationState,
};
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::helpers::read_recall_events;
use chrono::{TimeDelta, Utc};
use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

use crate::api::tests::helpers::{pool_capped, start_api, stop_api};
use crate::api::tests::test_types::{LocationHealthTestCase, PoolCapHealthTestCase};
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::models::ServerPoolCaps;
use crate::platform::main::health::report_location_health::report_location_health;
use crate::runs::tests::helpers::snapshot;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::tls::main::client_builder::client_builder;

#[test]
fn given_scoped_location_observations_when_retried_or_read_then_history_and_authority_are_preserved()
 {
    let test_cases = [
        LocationHealthTestCase {
            description: "SQLite health history",
            backend: Backend::Sqlite,
            expected_observations: 2,
        },
        LocationHealthTestCase {
            description: "PostgreSQL health history",
            backend: Backend::Postgres,
            expected_observations: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location_id = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let other = create_location(store, "inventory", "test-admin")
                .await
                .expect("other location");
            let token = issue_token(
                store,
                &AuditContext {
                    actor: "test-admin".into(),
                    granting_role: "admin".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                },
                CredentialScope::Location { location_id },
                Utc::now() + chrono::Duration::hours(1),
            )
            .await
            .expect("location token");
            let api = start_api(store).await;
            let client = client_builder().build().expect("HTTP client");
            let url = format!("{}/api/locations/{location_id}/health", api.url);
            let other_url = format!("{}/api/locations/{other}/health", api.url);
            let heartbeat = LocationHealthReport {
                event_id: Uuid::now_v7(),
                session_id: Uuid::now_v7(),
                target: "preview #1".into(),
                health: LocationHealth::Heartbeat {},
            };
            let before = Utc::now();
            assert!(
                report_remote_location_health((&api.url, &token.bearer), location_id, &heartbeat)
                    .await
                    .expect("register")
            );
            assert!(
                !report_remote_location_health((&api.url, &token.bearer), location_id, &heartbeat)
                    .await
                    .expect("exact retry")
            );
            let changed = LocationHealthReport {
                target: "another target".into(),
                ..heartbeat.clone()
            };
            let response = client
                .post(&url)
                .bearer_auth(&token.bearer)
                .json(&changed)
                .send()
                .await
                .expect("changed retry");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let response = client
                .post(&other_url)
                .bearer_auth(&token.bearer)
                .json(&heartbeat)
                .send()
                .await
                .expect("foreign observation");
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let response = client
                .get(&other_url)
                .bearer_auth(&token.bearer)
                .send()
                .await
                .expect("foreign history");
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .body("invalid")
                .send()
                .await
                .expect("anonymous report");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            let response = client.post(&url).bearer_auth(&token.bearer).json(&json!({
                "event_id": Uuid::now_v7(), "session_id": heartbeat.session_id, "target": heartbeat.target,
                "health": { "kind": "heartbeat", "unexpected": "must not disappear" },
            })).send().await.expect("strict health schema");
            assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
            let heartbeat = LocationHealthReport {
                event_id: Uuid::now_v7(),
                ..heartbeat
            };
            assert!(
                report_remote_location_health((&api.url, &token.bearer), location_id, &heartbeat)
                    .await
                    .expect("refresh existing instance liveness")
            );
            let connection = LocationHealthReport {
                event_id: Uuid::now_v7(),
                health: LocationHealth::Connection {
                    generation: 1,
                    healthy: false,
                    trigger: Some(ConnectionCheckTrigger::Startup),
                    elapsed_millis: Some(410),
                },
                ..heartbeat.clone()
            };
            let response = client
                .post(&url)
                .bearer_auth(&token.bearer)
                .json(&connection)
                .send()
                .await
                .expect("unpublished health");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let mut input = snapshot(location_id);
            input.target.clone_from(&heartbeat.target);
            publish_snapshot(store, &input)
                .await
                .expect("published generation");
            assert!(
                report_remote_location_health((&api.url, &token.bearer), location_id, &connection)
                    .await
                    .expect("connection observation")
            );
            let response = client
                .get(&url)
                .bearer_auth(&token.bearer)
                .send()
                .await
                .expect("health history");
            assert_eq!(response.headers()["cache-control"], "no-store");
            let observations: Vec<LocationHealthObservation> =
                response.json().await.expect("typed health history");
            assert_eq!(
                observations.len(),
                test_case.expected_observations,
                "{}: rejected and duplicate reports append nothing",
                test_case.description
            );
            assert_eq!(observations[0].report, connection);
            assert_eq!(observations[1].report, heartbeat);
            let audit = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let administrator = bootstrap_user(store, &audit, "orders-admin")
                .await
                .expect("administrator");
            let no_caps = ServerPoolCaps::default();
            let person_read = |target: &'static str| {
                read_person_location_health(
                    store,
                    (&audit, administrator),
                    (location_id, target),
                    (DefaultRole::None, &no_caps),
                )
            };
            let read = person_read("preview #1").await.expect("person health");
            assert_eq!(
                (read.observations.clone(), read.state),
                (
                    observations.clone(),
                    LocationState::of(&observations, ("preview #1", Utc::now()))
                ),
                "a person sees the same observations for the reporting target, and the server's state of them"
            );
            assert!(
                person_read("another target")
                    .await
                    .expect("other target health")
                    .observations
                    .is_empty(),
                "another target's read leaves this target's observations out"
            );
            // A location syncs only after runs, so its newest report can be days old.
            for (age, kept) in [(7, true), (8, false), (0, true)] {
                let transaction = begin(store, TransactionMode::Write).await.expect("age");
                let day = (transaction.now - TimeDelta::days(age)).date_naive();
                let (transaction, _) = query(
                    transaction,
                    "UPDATE recall_events SET invocation_id=$1 WHERE event_type='location_health'",
                    vec![Parameter::Text(format!("location/{location_id}/{day}"))],
                )
                .await
                .expect("aged report");
                commit(transaction).await.expect("aged report committed");
                let reports = person_read("preview #1")
                    .await
                    .expect("aged health")
                    .observations;
                assert_eq!(
                    reports
                        .iter()
                        .any(|observation| observation.report == connection),
                    kept,
                    "{}: a report {age} days old is read within location health retention",
                    test_case.description
                );
            }
            let events = read_recall_events(store, 0, 100)
                .await
                .expect("read durable events");
            assert_eq!(
                events
                    .iter()
                    .filter(
                        |event| event.get("event_type").and_then(serde_json::Value::as_str)
                            == Some("location_health")
                    )
                    .count(),
                1,
                "heartbeats only update liveness; connection observations remain durable facts"
            );
            assert!(
                observations
                    .iter()
                    .all(|observation| observation.received_at >= before
                        && observation.received_at <= Utc::now())
            );
            let mut busy = snapshot(location_id);
            busy.target = "another target".into();
            publish_snapshot(store, &busy)
                .await
                .expect("busy target generation");
            let busy_heartbeat = LocationHealthReport {
                event_id: Uuid::now_v7(),
                session_id: Uuid::now_v7(),
                target: busy.target.clone(),
                health: LocationHealth::Heartbeat {},
            };
            report_remote_location_health((&api.url, &token.bearer), location_id, &busy_heartbeat)
                .await
                .expect("busy target instance");
            for _ in 0..101 {
                let busy_connection = LocationHealthReport {
                    event_id: Uuid::now_v7(),
                    health: LocationHealth::Connection {
                        generation: 1,
                        healthy: true,
                        trigger: None,
                        elapsed_millis: None,
                    },
                    ..busy_heartbeat.clone()
                };
                report_remote_location_health(
                    (&api.url, &token.bearer),
                    location_id,
                    &busy_connection,
                )
                .await
                .expect("busy target observation");
            }
            assert_eq!(
                person_read("preview #1")
                    .await
                    .expect("quiet target health")
                    .observations,
                observations,
                "a busier target's newer observations do not push out this target's"
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_pool_amounts_above_this_servers_caps_when_reading_health_then_a_pool_capped_warning_lists_them()
 {
    let test_cases = [
        PoolCapHealthTestCase {
            description: "a cap below a job's amount warns about that job only",
            caps: &[("warehouse_heavy", 1)],
            expected_warnings: &[(
                "orders",
                "warehouse_heavy",
                3,
                1,
                "job orders takes 3 of pool warehouse_heavy; this server caps it at 1, so it runs \
                 alone in that pool",
            )],
        },
        PoolCapHealthTestCase {
            description: "a cap below a group's amount names it a job group",
            caps: &[("supplier_api", 1)],
            expected_warnings: &[(
                "nightly_group",
                "supplier_api",
                2,
                1,
                "job group nightly_group takes 2 of pool supplier_api; this server caps it at 1, \
                 so it runs alone in that pool",
            )],
        },
        PoolCapHealthTestCase {
            description: "a cap at or above every amount warns about nothing",
            caps: &[("warehouse_heavy", 3), ("supplier_api", 2)],
            expected_warnings: &[],
        },
        PoolCapHealthTestCase {
            description: "an uncapped server warns about nothing",
            caps: &[],
            expected_warnings: &[],
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location_id = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location_id);
            input.documents.insert(
                "jobs".into(),
                json!({
                    "orders": {"description": "Build orders", "selection": ["orders"],
                        "pools": {"warehouse_heavy": {"amount": 3, "limit": 4}}},
                    "customers": {"description": "Build customers", "selection": ["customers"],
                        "pools": {"warehouse_heavy": {"amount": 1, "limit": 4}}},
                }),
            );
            input.documents.insert(
                "groups".into(),
                json!({"nightly_group": {"description": "Everything nightly",
                    "selection": ["customers", "orders"],
                    "pools": {"supplier_api": {"amount": 2, "limit": 2}},
                    "jobs": {"customers": [], "orders": []}}}),
            );
            let published = publish_snapshot(store, &input).await.expect("publish");
            let audit = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let administrator = bootstrap_user(store, &audit, "orders-admin")
                .await
                .expect("administrator");
            let caps = ServerPoolCaps {
                launchers: [(
                    (location_id, "development".to_owned()),
                    test_case
                        .caps
                        .iter()
                        .map(|(pool, cap)| {
                            (
                                (*pool).to_owned(),
                                std::num::NonZeroU32::new(*cap).expect("cap"),
                            )
                        })
                        .collect(),
                )]
                .into(),
            };

            let observations = read_person_location_health(
                store,
                (&audit, administrator),
                (location_id, "development"),
                (DefaultRole::None, &caps),
            )
            .await
            .expect("health")
            .observations;

            let capped = pool_capped(&observations);
            assert!(
                capped
                    .iter()
                    .all(|(generation, event_id, _)| (*generation, *event_id)
                        == (1, published.snapshot_id)),
                "{}: the warning belongs to the current snapshot",
                test_case.description
            );
            let warnings: Vec<(String, String, u32, u32, String)> = capped
                .into_iter()
                .map(|(_, _, warning)| {
                    (
                        warning.job_key,
                        warning.pool,
                        warning.amount,
                        warning.cap,
                        warning.message,
                    )
                })
                .collect();
            assert_eq!(
                warnings,
                test_case
                    .expected_warnings
                    .iter()
                    .map(|(job, pool, amount, cap, message)| (
                        (*job).to_owned(),
                        (*pool).to_owned(),
                        *amount,
                        *cap,
                        (*message).to_owned()
                    ))
                    .collect::<Vec<_>>(),
                "{}",
                test_case.description
            );
            assert!(
                report_location_health(
                    store,
                    location_id,
                    &LocationHealthReport {
                        event_id: Uuid::now_v7(),
                        session_id: Uuid::now_v7(),
                        target: "development".into(),
                        health: LocationHealth::PoolCapped {
                            generation: 1,
                            jobs: Vec::new(),
                        },
                    },
                )
                .await
                .is_err(),
                "{}: a location cannot report the server's own warning",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
