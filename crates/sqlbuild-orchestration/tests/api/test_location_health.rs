use chrono::Utc;
use reqwest::StatusCode;
use serde_json::json;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::platform::main::report_remote_location_health::report_remote_location_health;
use sqlbuild_orchestration::platform::models::{
    LocationHealth, LocationHealthObservation, LocationHealthReport,
};
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::read_recall_events::read_recall_events;
use uuid::Uuid;

use crate::helpers::{start_api, stop_api};
use crate::location_helpers::create_location;
use crate::run_helpers::snapshot;
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::LocationHealthTestCase;

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
            let client = reqwest::Client::new();
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
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
