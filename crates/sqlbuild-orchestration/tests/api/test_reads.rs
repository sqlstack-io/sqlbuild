//! Real HTTP reads keep both location and target scope, including guessed nested resource IDs.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::Utc;
use reqwest::StatusCode;
use serde_json::json;
use sqlbuild_orchestration::credentials::main::create_principal::create_principal;
use sqlbuild_orchestration::credentials::main::issue_token::issue_token;
use sqlbuild_orchestration::credentials::main::revoke_token::revoke_token;
use sqlbuild_orchestration::credentials::models::{AuditContext, CredentialScope, PrincipalKind};
use sqlbuild_orchestration::declarations::main::change_trigger::change_trigger;
use sqlbuild_orchestration::declarations::main::list_triggers::list_triggers;
use sqlbuild_orchestration::declarations::models::{TriggerChange, TriggerChangeRequest};
use sqlbuild_orchestration::runs::main::claim_next_run::claim_next_run;
use sqlbuild_orchestration::runs::main::request_run::request_run;
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::scheduler::main::evaluate_trigger::evaluate_trigger;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::helpers::{read_json, start_api, stop_api};
use crate::location_helpers::create_location;
use crate::run_helpers::{request, snapshot};
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::ApiReadTestCase;

#[test]
fn given_published_locations_and_scheduled_work_when_read_through_http_then_resources_and_histories_remain_scoped()
 {
    let test_cases = [
        ApiReadTestCase {
            description: "SQLite scoped HTTP reads",
            backend: Backend::Sqlite,
            expected_runs: 3,
            expected_ticks: 3,
        },
        ApiReadTestCase {
            description: "PostgreSQL scoped HTTP reads",
            backend: Backend::Postgres,
            expected_runs: 3,
            expected_ticks: 3,
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
            let mut input = snapshot(location_id);
            input.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            );
            input.documents.insert("triggers".into(), json!({"daily_orders": {"description": "Refresh orders", "job_key": "orders", "starts_at": "2020-01-01T00:00:00Z",
                "schedule": {"kind": "calendar", "times": ["2020-01-01T00:00:00Z", "2020-01-02T00:00:00Z", "2020-01-03T00:00:00Z", "2030-01-01T00:00:00Z"], "refresh_at": "2029-01-01T00:00:00Z"}}}));
            let published = publish_snapshot(store, &input)
                .await
                .expect("publish declarations");
            let trigger_id = list_triggers(store, location_id, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let mut foreign = input.clone();
            foreign.location_id = other;
            let foreign_snapshot = publish_snapshot(store, &foreign)
                .await
                .expect("publish other location");
            let foreign_trigger = list_triggers(store, other, "development")
                .await
                .expect("foreign trigger")[0]
                .trigger_id;
            let foreign_run = request(
                (other, foreign_snapshot.snapshot_id),
                1001,
                0,
                &["inventory"],
            );
            request_run(store, &foreign_run)
                .await
                .expect("queue foreign run");
            let mut another_target = input.clone();
            another_target.target = "validation".into();
            publish_snapshot(store, &another_target)
                .await
                .expect("publish second target");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            evaluate_trigger(store, &lease, trigger_id)
                .await
                .expect("evaluate due schedule");
            let mut manual_ids = Vec::new();
            for (id, node) in [(100, "customers"), (101, "products")] {
                let run = request((location_id, published.snapshot_id), id, 0, &[node]);
                manual_ids.push(run.run_id);
                request_run(store, &run).await.expect("queue manual run");
            }
            let change = TriggerChangeRequest {
                event_id: Uuid::now_v7(),
                trigger_id,
                actor: "test-admin".into(),
                reason: "Pause orders refresh".into(),
                change: TriggerChange::Pause,
            };
            change_trigger(store, &change).await.expect("pause trigger");
            let audit = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let expiry = Utc::now() + chrono::Duration::hours(1);
            let token = issue_token(
                store,
                &audit,
                CredentialScope::Location { location_id },
                expiry,
            )
            .await
            .expect("location token");
            let principal_id =
                create_principal(store, &audit, ("orders operator", PrincipalKind::Person))
                    .await
                    .expect("unprivileged person");
            let principal = issue_token(
                store,
                &audit,
                CredentialScope::Principal { principal_id },
                expiry,
            )
            .await
            .expect("principal token");
            let claim = claim_next_run(store, &lease, &BTreeMap::new())
                .await
                .expect("claim")
                .expect("work");
            let worker = issue_token(
                store,
                &audit,
                CredentialScope::Run {
                    run_id: claim.request.run_id,
                    claim_id: claim.claim_id,
                },
                expiry,
            )
            .await
            .expect("worker token");
            let api = start_api(store).await;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("HTTP client");
            let prefix = format!(
                "{}/api/locations/{location_id}/targets/development",
                api.url
            );
            let run_path = format!("{prefix}/runs");
            let node_path = format!("{prefix}/runs/{}/nodes", manual_ids[0]);
            let trigger_path = format!("{prefix}/triggers");
            let event_path = format!("{trigger_path}/{trigger_id}/events");
            let tick_path = format!("{trigger_path}/{trigger_id}/ticks");
            let paths = [
                &run_path,
                &node_path,
                &trigger_path,
                &event_path,
                &tick_path,
            ];
            let runs = read_json(&client, (&run_path, &token.bearer)).await;
            assert_eq!(
                runs.as_array().expect("runs").len(),
                test_case.expected_runs,
                "{}",
                test_case.description
            );
            assert!(
                runs.as_array()
                    .expect("runs")
                    .iter()
                    .all(|run| run["location_id"] == location_id.to_string()
                        && run["target"] == "development")
            );
            let nodes = read_json(&client, (&node_path, &token.bearer)).await;
            assert_eq!(nodes.as_array().expect("nodes").len(), 1);
            assert_eq!(nodes[0]["node_key"], "customers");
            assert_eq!(nodes[0]["status"], "pending");
            let triggers = read_json(&client, (&trigger_path, &token.bearer)).await;
            assert_eq!(triggers.as_array().expect("triggers").len(), 1);
            assert_eq!(triggers[0]["trigger_id"], trigger_id.to_string());
            assert_eq!(triggers[0]["status"], "paused");
            let events = read_json(&client, (&event_path, &token.bearer)).await;
            assert_eq!(events.as_array().expect("events").len(), 1);
            assert_eq!(events[0]["request_json"], json!(change));
            let ticks = read_json(&client, (&tick_path, &token.bearer)).await;
            assert_eq!(
                ticks.as_array().expect("ticks").len(),
                test_case.expected_ticks
            );
            assert_eq!(ticks[0]["outcome"], "requested");
            assert_eq!(ticks[0]["scheduled_for"], "2020-01-03T00:00:00Z");
            assert_eq!(ticks[1]["outcome"], "covered");
            for path in [&run_path, &event_path, &tick_path] {
                assert_eq!(
                    read_json(&client, (&format!("{path}?limit=1"), &token.bearer))
                        .await
                        .as_array()
                        .expect("bounded history")
                        .len(),
                    1
                );
                assert_eq!(
                    read_json(&client, (&format!("{path}?limit=0"), &token.bearer)).await,
                    json!([])
                );
                assert_eq!(
                    client
                        .get(format!("{path}?limit=invalid"))
                        .bearer_auth(&token.bearer)
                        .send()
                        .await
                        .expect("invalid limit")
                        .status(),
                    StatusCode::BAD_REQUEST
                );
            }
            for path in paths {
                assert_eq!(
                    client
                        .get(path)
                        .send()
                        .await
                        .expect("unauthenticated read")
                        .status(),
                    StatusCode::UNAUTHORIZED
                );
                for bearer in [&principal.bearer, &worker.bearer] {
                    assert_eq!(
                        client
                            .get(path)
                            .bearer_auth(bearer)
                            .send()
                            .await
                            .expect("wrong credential scope")
                            .status(),
                        StatusCode::FORBIDDEN
                    );
                }
                assert_eq!(
                    client
                        .get(path.replace(&location_id.to_string(), &other.to_string()))
                        .bearer_auth(&token.bearer)
                        .send()
                        .await
                        .expect("cross-location read")
                        .status(),
                    StatusCode::FORBIDDEN
                );
            }
            for path in [
                node_path.replace(&manual_ids[0].to_string(), &foreign_run.run_id.to_string()),
                event_path.replace(&trigger_id.to_string(), &foreign_trigger.to_string()),
                tick_path.replace(&trigger_id.to_string(), &foreign_trigger.to_string()),
                node_path.replace("/development/", "/validation/"),
                event_path.replace("/development/", "/validation/"),
                tick_path.replace("/development/", "/validation/"),
            ] {
                assert_eq!(
                    client
                        .get(path)
                        .bearer_auth(&token.bearer)
                        .send()
                        .await
                        .expect("nested ID cannot bypass scope")
                        .status(),
                    StatusCode::NOT_FOUND
                );
            }
            assert_eq!(
                read_json(
                    &client,
                    (
                        &run_path.replace("/development/", "/validation/"),
                        &token.bearer
                    )
                )
                .await,
                json!([])
            );
            let validation = read_json(
                &client,
                (
                    &trigger_path.replace("/development/", "/validation/"),
                    &token.bearer,
                ),
            )
            .await;
            assert_eq!(validation.as_array().expect("other target").len(), 1);
            assert_ne!(validation[0]["trigger_id"], trigger_id.to_string());
            let openapi: serde_json::Value = client
                .get(format!("{}/api/openapi.json", api.url))
                .send()
                .await
                .expect("OpenAPI")
                .json()
                .await
                .expect("OpenAPI JSON");
            for path in [
                "/api/locations/{location_id}/targets/{target}/runs",
                "/api/locations/{location_id}/targets/{target}/runs/{run_id}/nodes",
                "/api/locations/{location_id}/targets/{target}/triggers",
                "/api/locations/{location_id}/targets/{target}/triggers/{trigger_id}/events",
                "/api/locations/{location_id}/targets/{target}/triggers/{trigger_id}/ticks",
            ] {
                assert_eq!(
                    openapi["paths"][path]["get"]["security"],
                    json!([{"bearer": []}])
                );
            }
            input.generation += 1;
            input.documents.insert("triggers".into(), json!({}));
            publish_snapshot(store, &input)
                .await
                .expect("remove declaration");
            assert_eq!(
                read_json(&client, (&trigger_path, &token.bearer)).await,
                json!([])
            );
            assert_eq!(
                read_json(&client, (&event_path, &token.bearer)).await,
                events
            );
            assert_eq!(read_json(&client, (&tick_path, &token.bearer)).await, ticks);
            revoke_token(store, &audit, token.identity.token_id)
                .await
                .expect("revoke location");
            for path in paths {
                assert_eq!(
                    client
                        .get(path)
                        .bearer_auth(&token.bearer)
                        .send()
                        .await
                        .expect("revoked read")
                        .status(),
                    StatusCode::UNAUTHORIZED
                );
            }
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
