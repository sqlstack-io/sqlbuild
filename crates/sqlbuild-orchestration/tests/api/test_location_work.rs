use chrono::{Duration, Utc};
use reqwest::StatusCode;
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::platform::main::connect_location_work::connect_location_work;
use sqlbuild_orchestration::platform::main::location_work::location_work;
use sqlbuild_orchestration::platform::main::report_location_health::report_location_health;
use sqlbuild_orchestration::platform::main::request_location_work::request_location_work;
use sqlbuild_orchestration::platform::models::{
    LocationHealth, LocationHealthReport, LocationWork, LocationWorkOutcome, LocationWorkPoll,
    LocationWorkRecord, LocationWorkRequest, LocationWorkResult,
};
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::helpers::{start_api, stop_api};
use crate::location_helpers::create_location;
use crate::run_helpers::snapshot;
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::LocationWorkTestCase;
use crate::work_helpers::execute;

#[test]
fn given_outbound_work_when_claimed_retried_or_replaced_then_scope_and_immutable_results_are_enforced()
 {
    let test_cases = [
        LocationWorkTestCase {
            description: "SQLite outbound work",
            backend: Backend::Sqlite,
            expected_generation: 1,
        },
        LocationWorkTestCase {
            description: "PostgreSQL outbound work",
            backend: Backend::Postgres,
            expected_generation: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("fresh migration");
            // Exercise both deployed predecessors, including checksums and repeated startup.
            execute(store, "DROP TABLE sqb_location_work_results; DROP TABLE sqb_location_work; DELETE FROM sqb_schema_migrations WHERE version=3").await;
            migrate_store(store).await.expect("schema 2 upgrade");
            execute(store, "DROP TABLE sqb_location_work_results; DROP TABLE sqb_location_work; DROP TABLE sqb_location_instances; DELETE FROM sqb_schema_migrations WHERE version>=2").await;
            migrate_store(store).await.expect("schema 1 upgrade");
            migrate_store(store).await.expect("repeat migration");
            let location_id = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let other = create_location(store, "inventory", "test-admin")
                .await
                .expect("other location");
            let audit = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let request = LocationWorkRequest {
                request_id: Uuid::now_v7(),
                location_id,
                target: "preview #1".into(),
                reason: "Check orders connectivity".into(),
                work: LocationWork::CheckConnection {},
            };
            assert!(
                request_location_work(store, &audit, &request)
                    .await
                    .is_err(),
                "unpublished target"
            );
            let mut input = snapshot(location_id);
            input.target.clone_from(&request.target);
            publish_snapshot(store, &input).await.expect("publish");
            let invalid_audit = AuditContext {
                granting_role: String::new(),
                ..audit.clone()
            };
            assert!(
                request_location_work(store, &invalid_audit, &request)
                    .await
                    .is_err()
            );
            assert!(
                location_work(store, request.request_id)
                    .await
                    .expect("rolled back read")
                    .is_none()
            );
            assert!(
                request_location_work(store, &audit, &request)
                    .await
                    .expect("enqueue")
            );
            let original = location_work(store, request.request_id)
                .await
                .expect("read")
                .expect("request");
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("republish");
            assert!(
                !request_location_work(store, &audit, &request)
                    .await
                    .expect("retry after redeploy")
            );
            assert_eq!(
                location_work(store, request.request_id)
                    .await
                    .expect("read"),
                Some(original.clone())
            );
            let changed = LocationWorkRequest {
                reason: "Different request".into(),
                ..request.clone()
            };
            assert!(
                request_location_work(store, &audit, &changed)
                    .await
                    .is_err()
            );
            let changed_actor = AuditContext {
                actor: "another-admin".into(),
                ..audit.clone()
            };
            assert!(
                request_location_work(store, &changed_actor, &request)
                    .await
                    .is_err()
            );
            let token = issue_token(
                store,
                &audit,
                CredentialScope::Location { location_id },
                Utc::now() + Duration::hours(1),
            )
            .await
            .expect("token");
            let api = start_api(store).await;
            let worker =
                connect_location_work((&api.url, &token.bearer), location_id).expect("work client");
            let poll = LocationWorkPoll {
                session_id: Uuid::now_v7(),
                target: request.target.clone(),
            };
            assert!(worker.poll(&poll).await.is_err(), "register before polling");
            for session_id in [poll.session_id, Uuid::nil()] {
                report_location_health(
                    store,
                    location_id,
                    &LocationHealthReport {
                        event_id: Uuid::now_v7(),
                        session_id,
                        target: poll.target.clone(),
                        health: LocationHealth::Heartbeat {},
                    },
                )
                .await
                .expect("register instance");
            }
            let wrong_target = LocationWorkPoll {
                target: "another target".into(),
                ..poll.clone()
            };
            assert!(worker.poll(&wrong_target).await.is_err());
            let foreign =
                connect_location_work((&api.url, &token.bearer), other).expect("foreign client");
            assert!(matches!(
                foreign.poll(&poll).await,
                Err(ClientError::Rejected(StatusCode::FORBIDDEN))
            ));
            let client = reqwest::Client::new();
            let url = format!("{}/api/locations/{location_id}/work", api.url);
            let response = client
                .post(format!("{url}/poll"))
                .header("Content-Type", "application/json")
                .body("invalid")
                .send()
                .await
                .expect("anonymous");
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            let claim = worker.poll(&poll).await.expect("poll").expect("claim");
            assert_eq!(
                claim.generation, test_case.expected_generation,
                "{}: request remains pinned despite redeployment",
                test_case.description
            );
            assert_eq!(claim.request, request);
            assert_eq!(
                worker.poll(&poll).await.expect("lost poll response retry"),
                Some(claim.clone())
            );
            let replacement = LocationWorkPoll {
                session_id: Uuid::nil(),
                ..poll.clone()
            };
            assert!(
                worker
                    .poll(&replacement)
                    .await
                    .expect("another instance")
                    .is_none()
            );
            let stale = LocationWorkResult {
                claim_id: claim.claim_id,
                session_id: poll.session_id,
                outcome: LocationWorkOutcome::Connection { healthy: false },
            };
            execute(
                store,
                "UPDATE sqb_location_work SET claimed_until='2000-01-01T00:00:00Z'",
            )
            .await;
            assert!(matches!(
                worker.complete(request.request_id, &stale).await,
                Err(ClientError::Rejected(StatusCode::CONFLICT))
            ));
            let reclaimed = worker
                .poll(&replacement)
                .await
                .expect("reclaim")
                .expect("new claim");
            assert_ne!(claim.claim_id, reclaimed.claim_id);
            assert!(matches!(
                worker.complete(request.request_id, &stale).await,
                Err(ClientError::Rejected(StatusCode::CONFLICT))
            ));
            let result = LocationWorkResult {
                claim_id: reclaimed.claim_id,
                session_id: replacement.session_id,
                outcome: LocationWorkOutcome::Superseded {},
            };
            assert!(
                worker
                    .complete(request.request_id, &result)
                    .await
                    .expect("complete")
            );
            execute(store, "UPDATE sqb_location_work SET expires_at='2000-01-01T00:00:00Z', claimed_until='2000-01-01T00:00:00Z'").await;
            assert!(
                !worker
                    .complete(request.request_id, &result)
                    .await
                    .expect("lost acknowledgement retry after expiry")
            );
            let changed_result = LocationWorkResult {
                outcome: LocationWorkOutcome::Connection { healthy: true },
                ..result.clone()
            };
            assert!(
                worker
                    .complete(request.request_id, &changed_result)
                    .await
                    .is_err()
            );
            assert!(
                worker
                    .poll(&poll)
                    .await
                    .expect("completed excluded")
                    .is_none()
            );
            let response = client
                .get(format!("{url}/{}", request.request_id))
                .bearer_auth(&token.bearer)
                .send()
                .await
                .expect("result read");
            assert_eq!(response.headers()["cache-control"], "no-store");
            let record: LocationWorkRecord = response.json().await.expect("typed result");
            assert_eq!(record.result_json, Some(result));
            assert!(record.completed_at.is_some());
            let next = LocationWorkRequest {
                request_id: Uuid::now_v7(),
                ..request
            };
            request_location_work(store, &audit, &next)
                .await
                .expect("another request");
            execute(
                store,
                "UPDATE sqb_location_work SET expires_at='2000-01-01T00:00:00Z'",
            )
            .await;
            assert!(
                worker
                    .poll(&poll)
                    .await
                    .expect("expired excluded")
                    .is_none()
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
