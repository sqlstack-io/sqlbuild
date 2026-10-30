use std::collections::BTreeMap;
use std::time::Duration;

use chrono::Utc;
use reqwest::StatusCode;
use serde_json::json;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::runs::models::{RunAuthority, RunChange, RunChangeRequest, RunStatus};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::models::SnapshotRecord;
use sqlbuild_orchestration::store::main::create_location::create_location;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::helpers::{start_api, stop_api};
use crate::run_helpers::{queue, request, snapshot};
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;

#[test]
fn given_a_real_http_server_when_locations_and_workers_report_then_scopes_retries_and_terminal_expiry_are_enforced()
 {
    for backend in [Backend::Sqlite, Backend::Postgres] {
        run(async {
            let Some(fixture) = open_test_store(backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location_id = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let other = create_location(store, "inventory", "test-admin")
                .await
                .expect("another location");
            let audit = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let expiry = Utc::now() + chrono::Duration::hours(1);
            let location_token = issue_token(
                store,
                &audit,
                CredentialScope::Location { location_id },
                expiry,
            )
            .await
            .expect("location credential");
            let api = start_api(store).await;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("HTTP client");
            let path = format!(
                "{}/api/locations/{location_id}/targets/development/snapshots",
                api.url
            );
            let response = client
                .post(&path)
                .header("Content-Type", "application/json")
                .body("invalid-json")
                .send()
                .await
                .expect("unauthenticated request");
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "authentication precedes JSON parsing"
            );
            let openapi: serde_json::Value = client
                .get(format!("{}/api/openapi.json", api.url))
                .send()
                .await
                .expect("OpenAPI response")
                .json()
                .await
                .expect("OpenAPI JSON");
            assert_eq!(
                openapi["components"]["securitySchemes"]["bearer"]["scheme"],
                "bearer"
            );
            assert!(openapi["components"]["schemas"]["SnapshotInput"].is_object());
            assert!(openapi["components"]["schemas"]["RunChangeRequest"].is_object());
            let input = snapshot(location_id);
            let response = client
                .post(&path)
                .bearer_auth(&location_token.bearer)
                .json(&input)
                .send()
                .await
                .expect("publish through API");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["cache-control"], "no-store");
            let published: SnapshotRecord = response.json().await.expect("snapshot response");
            let response = client
                .post(&path)
                .bearer_auth(&location_token.bearer)
                .json(&input)
                .send()
                .await
                .expect("stale publish");
            assert_eq!(response.status(), StatusCode::CONFLICT);
            let response = client
                .post(format!(
                    "{}/api/locations/{other}/targets/development/snapshots",
                    api.url
                ))
                .bearer_auth(&location_token.bearer)
                .json(&snapshot(other))
                .send()
                .await
                .expect("cross-location publish");
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let current: serde_json::Value = client
                .get(&path)
                .bearer_auth(&location_token.bearer)
                .send()
                .await
                .expect("current snapshot")
                .json()
                .await
                .expect("snapshot JSON");
            assert_eq!(
                current["record"]["snapshot_id"],
                published.snapshot_id.to_string()
            );
            assert_eq!(current["nodes"]["orders"]["compiled_sql"], "select 1 as id");
            queue(
                store,
                &request((location_id, published.snapshot_id), 1, 0, &["orders"]),
            )
            .await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let claim = claim_next_run(store, &lease, &BTreeMap::new())
                .await
                .expect("claim")
                .expect("work");
            let run_id = claim.request.run_id;
            let worker = issue_token(
                store,
                &audit,
                CredentialScope::Run {
                    run_id,
                    claim_id: claim.claim_id,
                },
                expiry,
            )
            .await
            .expect("worker credential");
            let events_path = format!("{}/api/runs/{run_id}/events", api.url);
            let heartbeat_path = format!("{}/api/runs/{run_id}/heartbeat", api.url);
            let started = RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id,
                claim_id: Some(claim.claim_id),
                change: RunChange::Started,
            };
            assert_eq!(
                client
                    .post(&events_path)
                    .bearer_auth(&location_token.bearer)
                    .json(&started)
                    .send()
                    .await
                    .expect("location cannot act as worker")
                    .status(),
                StatusCode::FORBIDDEN
            );
            assert_eq!(
                client
                    .get(&path)
                    .bearer_auth(&worker.bearer)
                    .send()
                    .await
                    .expect("worker cannot read location snapshot")
                    .status(),
                StatusCode::FORBIDDEN
            );
            for applied in [true, false] {
                let response = client
                    .post(&events_path)
                    .bearer_auth(&worker.bearer)
                    .json(&started)
                    .send()
                    .await
                    .expect("report started");
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(
                    response
                        .json::<serde_json::Value>()
                        .await
                        .expect("report response"),
                    json!({"applied": applied})
                );
            }
            let wrong_claim = RunChangeRequest {
                event_id: Uuid::now_v7(),
                claim_id: Some(Uuid::now_v7()),
                ..started.clone()
            };
            assert_eq!(
                client
                    .post(&events_path)
                    .bearer_auth(&worker.bearer)
                    .json(&wrong_claim)
                    .send()
                    .await
                    .expect("wrong claim")
                    .status(),
                StatusCode::FORBIDDEN
            );
            assert_eq!(
                client
                    .post(format!("{}/api/runs/{}/heartbeat", api.url, Uuid::now_v7()))
                    .bearer_auth(&worker.bearer)
                    .send()
                    .await
                    .expect("other run")
                    .status(),
                StatusCode::FORBIDDEN
            );
            let cancellation = RunChangeRequest {
                event_id: Uuid::now_v7(),
                change: RunChange::CancelRequested {
                    actor: "test-admin".into(),
                },
                ..started.clone()
            };
            assert_eq!(
                client
                    .post(&events_path)
                    .bearer_auth(&worker.bearer)
                    .json(&cancellation)
                    .send()
                    .await
                    .expect("worker cannot impersonate admin")
                    .status(),
                StatusCode::BAD_REQUEST
            );
            let response = client
                .post(&heartbeat_path)
                .bearer_auth(&worker.bearer)
                .send()
                .await
                .expect("worker heartbeat");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response
                    .json::<serde_json::Value>()
                    .await
                    .expect("heartbeat response"),
                json!({"cancel_requested": false})
            );
            change_run(store, RunAuthority::Operator("test-admin"), &cancellation)
                .await
                .expect("operator cancellation");
            let response: serde_json::Value = client
                .post(&heartbeat_path)
                .bearer_auth(&worker.bearer)
                .send()
                .await
                .expect("cancel heartbeat")
                .json()
                .await
                .expect("cancel response");
            assert_eq!(response, json!({"cancel_requested": true}));
            let terminal = RunChangeRequest {
                event_id: Uuid::now_v7(),
                change: RunChange::Finished {
                    outcome: sqlbuild_orchestration::runs::models::RunOutcome::Cancelled,
                    reason: "worker acknowledged cancellation".into(),
                    exit_code: Some(143),
                },
                ..started
            };
            assert_eq!(
                client
                    .post(&events_path)
                    .bearer_auth(&worker.bearer)
                    .json(&terminal)
                    .send()
                    .await
                    .expect("worker terminates")
                    .status(),
                StatusCode::OK
            );
            assert_eq!(
                client
                    .post(&heartbeat_path)
                    .bearer_auth(&worker.bearer)
                    .send()
                    .await
                    .expect("terminal credential rejected")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                list_runs(store, (location_id, "development"), 100)
                    .await
                    .expect("run projection")[0]
                    .status,
                RunStatus::Cancelled
            );
            revoke_token(store, &audit, location_token.identity.token_id)
                .await
                .expect("revoke location token");
            assert_eq!(
                client
                    .get(&path)
                    .bearer_auth(&location_token.bearer)
                    .send()
                    .await
                    .expect("revocation effective")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
