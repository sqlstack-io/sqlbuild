//! A scoped HTTP reader mirrors immutable warehouse observations, including external runs.

use chrono::{Duration, Utc};
use reqwest::StatusCode;
use serde_json::json;
use uuid::Uuid;

use crate::api::tests::changes_helpers::{connect, expect_topics};
use crate::api::tests::helpers::{start_api, stop_api};
use crate::api::tests::test_types::WarehouseMetadataTestCase;
use crate::credentials::main::tokens::issue_token::issue_token;
use crate::credentials::models::{AuditContext, CredentialScope};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::connect_location_work::connect_location_work;
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::models::{SourceFreshnessBatch, WarehouseHistoryKind};

#[test]
fn given_warehouse_observations_when_pages_retry_then_scope_history_and_atomicity_are_preserved() {
    let test_cases = [
        WarehouseMetadataTestCase {
            description: "SQLite warehouse history",
            backend: Backend::Sqlite,
            expected_observations: 4,
        },
        WarehouseMetadataTestCase {
            description: "PostgreSQL warehouse history",
            backend: Backend::Postgres,
            expected_observations: 4,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let other = create_location(store, "inventory", "test-admin")
                .await
                .expect("other location");
            let input = snapshot(location);
            publish_snapshot(store, &input).await.expect("snapshot");
            let session_id = Uuid::now_v7();
            report_location_health(
                store,
                location,
                &LocationHealthReport {
                    event_id: Uuid::now_v7(),
                    session_id,
                    target: "development".into(),
                    health: LocationHealth::Heartbeat {},
                },
            )
            .await
            .expect("registered reader");
            let token = issue_token(
                store,
                &AuditContext {
                    actor: "test-admin".into(),
                    granting_role: "admin".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                },
                CredentialScope::Location {
                    location_id: location,
                },
                Utc::now() + Duration::hours(1),
            )
            .await
            .expect("token");
            let api = start_api(store).await;
            let client = reqwest::Client::new();
            let url = format!(
                "{}/api/locations/{location}/targets/development/metadata/source-freshness",
                api.url
            );
            let record = json!({
                "event_id": "warehouse-observation-1",
                "state_database": null, "state_schema": "main", "source_name": "raw_orders",
                "target_database": null, "target_schema": "main", "target_name": "orders",
                "run_id": "external-command-1", "strategy": "max_column", "value_kind": "integer",
                "data_version": "18446744073709551617", "data_version_hash": "same-data-version",
                "observed_at": "2026-01-01T10:00:00Z",
            });
            let mut later = record.clone();
            later["event_id"] = json!("warehouse-observation-2");
            later["observed_at"] = json!("2026-01-01T11:00:00Z");
            let mut other_run = record.clone();
            other_run
                .as_object_mut()
                .expect("record")
                .remove("event_id");
            other_run["run_id"] = json!("external-command-2");
            let mut distinct = record.clone();
            distinct["event_id"] = json!("warehouse-observation-3");
            let mut page = vec![record.clone(); 100];
            page[1] = later;
            page[2] = other_run.clone();
            page[3] = distinct;
            // Both missing and explicitly NULL identities use the same fixed legacy fallback.
            other_run["event_id"] = json!(null);
            page[4] = other_run;
            let batch = json!({"session_id": session_id, "generation": 1, "records": page});
            let outbound = connect_location_work((&api.url, &token.bearer), location)
                .expect("pooled native location client");
            let typed: SourceFreshnessBatch = serde_json::from_value(batch.clone()).expect("page");
            let mut changes =
                connect(&client, &api, (location, "development", &token.bearer)).await;
            assert!(
                outbound
                    .sync_source_freshness("development", &typed)
                    .await
                    .expect("native client sync")
                    .applied
            );
            expect_topics(&mut changes, &["node_status:development".into()]).await;
            assert!(
                !outbound
                    .sync_source_freshness("development", &typed)
                    .await
                    .expect("native client replay")
                    .applied
            );
            drop(changes);
            let mut equivalent = batch.clone();
            equivalent["records"] = json!([record]);
            equivalent["records"][0]["observed_at"] = json!("2026-01-01T11:00:00+01:00");
            assert_eq!(
                client
                    .post(&url)
                    .bearer_auth(&token.bearer)
                    .json(&equivalent)
                    .send()
                    .await
                    .expect("equivalent timestamp")
                    .json::<serde_json::Value>()
                    .await
                    .expect("reply"),
                json!({"applied": false})
            );
            let mut stale = batch.clone();
            stale["generation"] = json!(2);
            let mut invalid = batch.clone();
            invalid["records"][1]["source_name"] = json!(" ");
            let mut invalid_id = batch.clone();
            invalid_id["records"][0]["event_id"] = json!(" ");
            let mut oversized = batch.clone();
            oversized["records"] = json!(vec![record.clone(); 101]);
            let mut unknown = batch.clone();
            unknown["records"][0]["status"] = json!("success");
            let foreign = url.replace(&location.to_string(), &other.to_string());
            let other_target = url.replace("/development/", "/production/");
            for (endpoint, body, expected) in [
                (&url, stale, StatusCode::CONFLICT),
                (&url, invalid, StatusCode::BAD_REQUEST),
                (&url, invalid_id, StatusCode::BAD_REQUEST),
                (&url, oversized, StatusCode::BAD_REQUEST),
                (&url, unknown, StatusCode::UNPROCESSABLE_ENTITY),
                (&foreign, batch.clone(), StatusCode::FORBIDDEN),
                (&other_target, batch.clone(), StatusCode::BAD_REQUEST),
            ] {
                assert_eq!(
                    client
                        .post(endpoint)
                        .bearer_auth(&token.bearer)
                        .json(&body)
                        .send()
                        .await
                        .expect("rejected page")
                        .status(),
                    expected
                );
            }
            assert_eq!(
                client
                    .post(&url)
                    .body("invalid")
                    .send()
                    .await
                    .expect("anonymous")
                    .status(),
                StatusCode::UNAUTHORIZED
            );
            let fault = TransactionFault {
                description: "warehouse page commits all records or none",
                table: "sqb_warehouse_freshness",
                operation: "INSERT",
                condition: "WHEN NEW.record_json->>'source_name'='inventory'",
                sqlite_body: "SELECT RAISE(ABORT, 'injected metadata failure')",
                postgres_body: "RAISE EXCEPTION 'injected metadata failure'",
            };
            install_fault(store, &fault).await;
            let mut new_record = record.clone();
            new_record["state_schema"] = json!("reporting");
            let mut inventory = new_record.clone();
            inventory["event_id"] = json!("inventory-observation-1");
            inventory["source_name"] = json!("inventory");
            let mut additional = batch.clone();
            additional["records"] = json!([new_record, inventory]);
            assert_eq!(
                client
                    .post(&url)
                    .bearer_auth(&token.bearer)
                    .json(&additional)
                    .send()
                    .await
                    .expect("faulted page")
                    .status(),
                StatusCode::INTERNAL_SERVER_ERROR
            );
            remove_fault(store, &fault).await;
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT record_json FROM sqb_warehouse_freshness ORDER BY observed_at,event_id",
                vec![],
            )
            .await
            .expect("mirrored history");
            commit(transaction).await.expect("close read");
            assert_eq!(
                rows.len(),
                test_case.expected_observations,
                "{}: no observation collapsed by data version and no partial page committed",
                test_case.description
            );
            for row in rows {
                assert_eq!(row["record_json"]["data_version"], "18446744073709551617");
                assert_eq!(row["record_json"]["state_schema"], "main");
            }
            assert_eq!(
                client
                    .post(&url)
                    .bearer_auth(&token.bearer)
                    .json(&additional)
                    .send()
                    .await
                    .expect("retry after rollback")
                    .json::<serde_json::Value>()
                    .await
                    .expect("reply"),
                json!({"applied": true})
            );
            migrate_store(store)
                .await
                .expect("warm migration preserves observations");
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT COUNT(*) AS n FROM sqb_warehouse_freshness",
                vec![],
            )
            .await
            .expect("retained history");
            commit(transaction).await.expect("close read");
            assert_eq!(rows[0]["n"], 6);
            // The same warehouse rows in another target are separate observations, not replays.
            let preview_session = Uuid::now_v7();
            let mut preview = input.clone();
            preview.target = "preview".into();
            publish_snapshot(store, &preview)
                .await
                .expect("preview snapshot");
            report_location_health(
                store,
                location,
                &LocationHealthReport {
                    event_id: Uuid::now_v7(),
                    session_id: preview_session,
                    target: "preview".into(),
                    health: LocationHealth::Heartbeat {},
                },
            )
            .await
            .expect("preview reader");
            let mut preview_batch = batch.clone();
            preview_batch["session_id"] = json!(preview_session);
            let preview_url = url.replace("/development/", "/preview/");
            assert_eq!(
                client
                    .post(&preview_url)
                    .bearer_auth(&token.bearer)
                    .json(&preview_batch)
                    .send()
                    .await
                    .expect("separate target")
                    .json::<serde_json::Value>()
                    .await
                    .expect("reply"),
                json!({"applied": true})
            );
            let (transaction, rows) = query(begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT target, COUNT(*) AS n FROM sqb_warehouse_freshness GROUP BY target ORDER BY target", vec![]).await.expect("isolated history");
            commit(transaction).await.expect("close read");
            assert_eq!(
                serde_json::to_value(rows).expect("rows"),
                json!([{"target": "development", "n": 6}, {"target": "preview", "n": 4}])
            );
            crate::api::tests::warehouse_metadata_helpers::fingerprint_history(
                store,
                &outbound,
                (session_id, preview_session),
                (&client, &url, &token.bearer),
            )
            .await;
            crate::api::tests::warehouse_reconciliation_helpers::reconcile_history(
                store,
                (&client, &url, &token.bearer),
                (session_id, preview_session),
            )
            .await;
            for event_case in [
                (
                    WarehouseHistoryKind::AuditResults,
                    "audit_result_history",
                    "result_id",
                ),
                (
                    WarehouseHistoryKind::JanitorEvents,
                    "janitor_event_history",
                    "event_id",
                ),
                (
                    WarehouseHistoryKind::Migrations,
                    "migration_history",
                    "event_id",
                ),
                (
                    WarehouseHistoryKind::OldNameViews,
                    "old_name_history",
                    "event_id",
                ),
                (
                    WarehouseHistoryKind::Microbatches,
                    "microbatch_history",
                    "event_id",
                ),
            ] {
                crate::api::tests::warehouse_events_helpers::event_history(
                    store,
                    &outbound,
                    (session_id, preview_session),
                    (&client, &url, &token.bearer, event_case),
                )
                .await;
            }
            crate::api::tests::warehouse_catalog_helpers::catalog_history(
                store,
                &outbound,
                (session_id, preview_session),
                (&client, &url, &token.bearer),
            )
            .await;
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
