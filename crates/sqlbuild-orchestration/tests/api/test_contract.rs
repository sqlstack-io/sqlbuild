use std::collections::BTreeMap;
use std::time::Duration;

use chrono::Utc;
use reqwest::StatusCode;
use serde_json::json;
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope};
use sqlbuild_orchestration::runs::main::change_run::change_run;
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::runs::main::request_run::request_run;
use sqlbuild_orchestration::runs::models::{
    RunAuthority, RunChange, RunChangeRequest, RunExecution, RunStatus,
};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_remote_snapshot::publish_remote_snapshot;
use sqlbuild_orchestration::snapshots::main::verify_execution::verify_execution;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotRecord};
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::helpers::{start_api, stop_api};
use crate::location_helpers::create_location;
use crate::run_helpers::{queue, request, snapshot};
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::ApiContractTestCase;

#[test]
fn given_a_real_http_server_when_locations_and_workers_report_then_scopes_retries_and_terminal_expiry_are_enforced()
 {
    let test_cases = [
        ApiContractTestCase {
            description: "SQLite worker and location HTTP contract",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Cancelled,
        },
        ApiContractTestCase {
            description: "PostgreSQL worker and location HTTP contract",
            backend: Backend::Postgres,
            expected_status: RunStatus::Cancelled,
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
            assert!(openapi["components"]["schemas"]["RunExecution"].is_object());
            let input = snapshot(location_id);
            let published = publish_remote_snapshot(&api.url, &location_token.bearer, &input)
                .await
                .expect("publish retained native data");
            let response = client
                .get(&path)
                .bearer_auth(&location_token.bearer)
                .send()
                .await
                .expect("read published snapshot through API");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["cache-control"], "no-store");
            drop(response);
            assert!(matches!(
                publish_remote_snapshot(&api.url, &location_token.bearer, &input).await,
                Err(ClientError::Rejected(StatusCode::CONFLICT))
            ));
            assert!(matches!(
                publish_remote_snapshot(&api.url, &location_token.bearer, &snapshot(other)).await,
                Err(ClientError::Rejected(StatusCode::FORBIDDEN))
            ));
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
            for image_id in [None, Some("sha256:unpublished-orders-image".into())] {
                let mut mismatched =
                    request((location_id, published.snapshot_id), 1, 0, &["orders"]);
                mismatched.image_id = image_id;
                assert!(request_run(store, &mismatched).await.is_err());
            }
            assert!(
                list_runs(store, (location_id, "development"), 100)
                    .await
                    .expect("rejected requests do not queue")
                    .is_empty()
            );
            queue(
                store,
                &request((location_id, published.snapshot_id), 1, 0, &["orders"]),
            )
            .await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
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
            let execution_path = format!("{}/api/runs/{run_id}/execution", api.url);
            let response = client
                .get(&execution_path)
                .bearer_auth(&worker.bearer)
                .send()
                .await
                .expect("worker execution contract");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["cache-control"], "no-store");
            let execution: RunExecution = response.json().await.expect("pinned execution");
            assert_eq!(execution.claim, claim);
            assert_eq!(execution.environment_hash, input.environment_hash);
            assert_eq!(execution.sqb_version, input.sqb_version);
            assert_eq!(
                execution.node_hashes.keys().collect::<Vec<_>>(),
                input.nodes.keys().collect::<Vec<_>>(),
                "dependency verification includes nodes outside the run selection"
            );
            verify_execution(&input, &execution).expect("matching worker compilation");
            for field in ["target", "image_id", "environment_hash", "sqb_version"] {
                let mut changed = json!(input);
                changed[field] = json!("different");
                let changed: SnapshotInput =
                    serde_json::from_value(changed).expect("changed worker inputs");
                assert!(verify_execution(&changed, &execution).is_err(), "{field}");
            }
            for field in [
                "kind",
                "definition",
                "raw_sql",
                "compiled_sql",
                "python_source",
            ] {
                let mut changed = json!(input);
                changed["nodes"]["inventory"][field] = json!("different");
                let changed: SnapshotInput =
                    serde_json::from_value(changed).expect("changed dependency");
                assert!(verify_execution(&changed, &execution).is_err(), "{field}");
            }
            let mut renamed = input.clone();
            let node = renamed.nodes.remove("inventory").expect("dependency");
            renamed.nodes.insert("renamed_inventory".into(), node);
            assert!(verify_execution(&renamed, &execution).is_err());
            renamed.nodes.remove("renamed_inventory");
            assert!(verify_execution(&renamed, &execution).is_err());
            let mut display_only = input.clone();
            display_only.generation += 1;
            display_only.git_author = Some("Example Author".into());
            display_only.git_dirty = true;
            display_only
                .documents
                .insert("columns".into(), json!({"orders": ["id"]}));
            verify_execution(&display_only, &execution)
                .expect("display metadata does not change executable code");
            for (url, token) in [
                (execution_path.clone(), &location_token.bearer),
                (
                    format!("{}/api/runs/{}/execution", api.url, Uuid::now_v7()),
                    &worker.bearer,
                ),
            ] {
                assert_eq!(
                    client
                        .get(url)
                        .bearer_auth(token)
                        .send()
                        .await
                        .expect("execution scope denial")
                        .status(),
                    StatusCode::FORBIDDEN
                );
            }
            let mut redeploy = input.clone();
            redeploy.generation += 1;
            redeploy.environment_hash = "updated-orders-environment".into();
            redeploy.image_id = Some("sha256:updated-orders-image".into());
            redeploy.sqb_version = "99.0.0".into();
            redeploy
                .nodes
                .get_mut("orders")
                .expect("orders")
                .compiled_sql = Some("select 2 as id".into());
            assert_eq!(
                client
                    .post(&path)
                    .bearer_auth(&location_token.bearer)
                    .json(&redeploy)
                    .send()
                    .await
                    .expect("publish redeploy")
                    .status(),
                StatusCode::OK
            );
            let still_pinned: RunExecution = client
                .get(&execution_path)
                .bearer_auth(&worker.bearer)
                .send()
                .await
                .expect("execution after redeploy")
                .json()
                .await
                .expect("old execution contract");
            assert_eq!(still_pinned, execution);
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
            for (output_run, credential) in [
                (Uuid::now_v7(), &worker.bearer),
                (run_id, &location_token.bearer),
            ] {
                for (endpoint, body) in [
                    ("output", json!([])),
                    (
                        "recall",
                        json!({"invocation_id": "execution", "records": []}),
                    ),
                ] {
                    assert_eq!(
                        client
                            .post(format!("{}/api/runs/{output_run}/{endpoint}", api.url))
                            .bearer_auth(credential)
                            .json(&body)
                            .send()
                            .await
                            .expect("publication scope denial")
                            .status(),
                        StatusCode::FORBIDDEN
                    );
                }
            }
            let mut forged = json!(crate::output_helpers::chunk(
                0,
                sqlbuild_recall::records::models::OutputStream::Stdout,
                b"forged"
            ));
            forged["invocation_id"] = json!("scheduler/another-run");
            assert_eq!(
                client
                    .post(format!("{}/api/runs/{run_id}/output", api.url))
                    .bearer_auth(&worker.bearer)
                    .json(&json!([forged]))
                    .send()
                    .await
                    .expect("body cannot choose its output namespace")
                    .status(),
                StatusCode::UNPROCESSABLE_ENTITY
            );
            let cancellation = RunChangeRequest {
                event_id: Uuid::now_v7(),
                change: RunChange::CancelRequested {
                    actor: "test-admin".into(),
                    reason: "Stop the orders build".into(),
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
            change_run(store, RunAuthority::Operator(&audit), &cancellation)
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
                client
                    .get(&execution_path)
                    .bearer_auth(&worker.bearer)
                    .send()
                    .await
                    .expect("terminal execution credential rejected")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            assert_eq!(
                list_runs(store, (location_id, "development"), 100)
                    .await
                    .expect("run projection")[0]
                    .status,
                test_case.expected_status,
                "{}",
                test_case.description
            );
            redeploy.generation += 1;
            redeploy.image_id = None;
            let local: SnapshotRecord = client
                .post(&path)
                .bearer_auth(&location_token.bearer)
                .json(&redeploy)
                .send()
                .await
                .expect("publish local snapshot without a container image")
                .json()
                .await
                .expect("local snapshot response");
            let mut local_request = request((location_id, local.snapshot_id), 2, 0, &["orders"]);
            local_request.image_id = None;
            queue(store, &local_request).await;
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
