//! A scoped HTTP reader mirrors immutable warehouse observations, including external runs.

use chrono::{DateTime, Duration, Utc};
use reqwest::StatusCode;
use serde_json::json;
use sqlbuild_core::constants::{DYNAMIC_PAUSES_TABLE_NAME, RUNS_TABLE_NAME, STATE_TABLE_NAMES};
use uuid::Uuid;

use crate::api::tests::changes_helpers::{connect, expect_topics};
use crate::api::tests::helpers::{start_api, stop_api};
use crate::api::tests::test_types::{
    KeptMirrorRow, MirrorConflictTestCase, StateTableClassificationTestCase,
    WarehouseMetadataTestCase,
};
use crate::api::tests::warehouse_history_helpers::{
    audit_page, batch, cost_page, fixture, seed_target,
};
use crate::credentials::main::tokens::issue_token::issue_token;
use crate::credentials::models::{AuditContext, CredentialScope};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::main::work::connect_location_work::connect_location_work;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::tls::main::client_builder::client_builder;
use crate::warehouse_metadata::constants::MAX_HISTORY_PAGE_RECORDS;
use crate::warehouse_metadata::models::{MetadataBatch, WarehouseHistoryKind};

#[test]
fn given_state_tables_when_classified_then_each_is_mirrored_by_exactly_one_kind() {
    // The runs table has no mirror until the control store decides where observed runs live;
    // dynamic pauses are a lease that builds read from the warehouse, never a mirror.
    let mut written: Vec<&str> = STATE_TABLE_NAMES
        .into_iter()
        .filter(|table| ![RUNS_TABLE_NAME, DYNAMIC_PAUSES_TABLE_NAME].contains(table))
        .collect();
    written.sort_unstable();
    let test_cases = [StateTableClassificationTestCase {
        description: "every state table SQLBuild writes, with statistics rewinding to catalog",
        expected_tables: written,
        expected_last_kinds: [
            WarehouseHistoryKind::Catalog,
            WarehouseHistoryKind::Statistics,
        ],
    }];
    for test_case in test_cases {
        let mut mirrored: Vec<&str> = WarehouseHistoryKind::ALL
            .iter()
            .filter_map(|kind| kind.source_table())
            .collect();
        mirrored.sort_unstable();
        assert_eq!(
            mirrored, test_case.expected_tables,
            "{}",
            test_case.description
        );
        assert_eq!(
            WarehouseHistoryKind::ALL[WarehouseHistoryKind::ALL.len() - 2..],
            test_case.expected_last_kinds,
            "{}",
            test_case.description
        );
    }
}

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
            let client = client_builder().build().expect("HTTP client");
            let url = format!(
                "{}/api/locations/{location}/targets/development/metadata",
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
            let batch = json!({"kind": "source_freshness", "session_id": session_id, "generation": 1, "records": page});
            let outbound = connect_location_work((&api.url, &token.bearer), location)
                .expect("pooled native location client");
            let typed: MetadataBatch = serde_json::from_value(batch.clone()).expect("page");
            let mut changes =
                connect(&client, &api, (location, "development", &token.bearer)).await;
            assert!(
                outbound
                    .sync_metadata("development", &typed)
                    .await
                    .expect("native client sync")
                    .applied
            );
            expect_topics(&mut changes, &["node_status:development".into()]).await;
            assert!(
                !outbound
                    .sync_metadata("development", &typed)
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
            oversized["records"] = json!(vec![record.clone(); MAX_HISTORY_PAGE_RECORDS + 1]);
            let mut unknown = batch.clone();
            unknown["records"][0]["status"] = json!("success");
            let foreign = url.replace(&location.to_string(), &other.to_string());
            let other_target = url.replace("/development/", "/production/");
            for (endpoint, body, expected) in [
                (&url, stale, StatusCode::CONFLICT),
                (&url, invalid, StatusCode::BAD_REQUEST),
                (&url, invalid_id, StatusCode::BAD_REQUEST),
                (&url, oversized, StatusCode::BAD_REQUEST),
                (&url, unknown, StatusCode::BAD_REQUEST),
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
            // A plan preview's copy counts as synced only once every schema the snapshot builds into has.
            for (name, node) in &mut preview.nodes {
                node.definition["destination"] =
                    json!({"database": null, "schema": "main", "name": name});
            }
            preview
                .nodes
                .get_mut("products")
                .expect("products node")
                .definition["destination"]["schema"] = json!("reporting");
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
            crate::api::tests::warehouse_metadata_helpers::sync_due_scope(
                store,
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
                (
                    WarehouseHistoryKind::ColumnMigrations,
                    "column_migration_history",
                    "event_id",
                ),
                (
                    WarehouseHistoryKind::NodeResults,
                    "node_result_history",
                    "node_name",
                ),
                (
                    WarehouseHistoryKind::Consumption,
                    "consumption_history",
                    "execution_id",
                ),
                (
                    WarehouseHistoryKind::Observations,
                    "observation_history",
                    "observation_id",
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
            crate::api::tests::warehouse_preview_copy_helpers::preview_copy_scope(
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

#[test]
fn given_one_source_id_in_two_pages_when_mirrored_in_either_order_then_the_same_row_is_kept() {
    let test_cases = [
        MirrorConflictTestCase {
            description: "SQLite audit results: page two's earlier row arrives second",
            backend: Backend::Sqlite,
            kind: WarehouseHistoryKind::AuditResults,
            fixture: "audit_result_history.json",
            fields: ["result_id", "occurred_at", "outcome"],
            ingest: audit_page,
            rows: [
                ("2026-02-03T04:05:06.000001Z", "pass"),
                ("2026-02-03T04:05:06Z", "pass"),
            ],
            expected_row: ("2026-02-03T04:05:06Z", "pass"),
            expected_column: Some("pass"),
        },
        MirrorConflictTestCase {
            description: "SQLite audit results: two rows at the same time",
            backend: Backend::Sqlite,
            kind: WarehouseHistoryKind::AuditResults,
            fixture: "audit_result_history.json",
            fields: ["result_id", "occurred_at", "outcome"],
            ingest: audit_page,
            rows: [
                ("2026-02-03T04:05:06Z", "pass"),
                ("2026-02-03T04:05:06Z", "error"),
            ],
            expected_row: ("2026-02-03T04:05:06Z", "error"),
            expected_column: Some("error"),
        },
        MirrorConflictTestCase {
            description: "SQLite resource costs: a refreshed observation keeps the later row",
            backend: Backend::Sqlite,
            kind: WarehouseHistoryKind::ResourceCosts,
            fixture: "resource_cost_history.json",
            fields: ["observation_id", "observed_at", "status"],
            ingest: cost_page,
            rows: [
                ("2026-02-03T04:05:06Z", "partial"),
                ("2026-02-03T04:05:06.000001Z", "complete"),
            ],
            expected_row: ("2026-02-03T04:05:06.000001Z", "complete"),
            expected_column: None,
        },
        MirrorConflictTestCase {
            description: "SQLite resource costs: two rows at the same time",
            backend: Backend::Sqlite,
            kind: WarehouseHistoryKind::ResourceCosts,
            fixture: "resource_cost_history.json",
            fields: ["observation_id", "observed_at", "status"],
            ingest: cost_page,
            rows: [
                ("2026-02-03T04:05:06Z", "partial"),
                ("2026-02-03T04:05:06Z", "complete"),
            ],
            expected_row: ("2026-02-03T04:05:06Z", "complete"),
            expected_column: None,
        },
        MirrorConflictTestCase {
            description: "PostgreSQL audit results: page two's earlier row arrives second",
            backend: Backend::Postgres,
            kind: WarehouseHistoryKind::AuditResults,
            fixture: "audit_result_history.json",
            fields: ["result_id", "occurred_at", "outcome"],
            ingest: audit_page,
            rows: [
                ("2026-02-03T04:05:06.000001Z", "pass"),
                ("2026-02-03T04:05:06Z", "pass"),
            ],
            expected_row: ("2026-02-03T04:05:06Z", "pass"),
            expected_column: Some("pass"),
        },
        MirrorConflictTestCase {
            description: "PostgreSQL audit results: two rows at the same time",
            backend: Backend::Postgres,
            kind: WarehouseHistoryKind::AuditResults,
            fixture: "audit_result_history.json",
            fields: ["result_id", "occurred_at", "outcome"],
            ingest: audit_page,
            rows: [
                ("2026-02-03T04:05:06Z", "pass"),
                ("2026-02-03T04:05:06Z", "error"),
            ],
            expected_row: ("2026-02-03T04:05:06Z", "error"),
            expected_column: Some("error"),
        },
        MirrorConflictTestCase {
            description: "PostgreSQL resource costs: a refreshed observation keeps the later row",
            backend: Backend::Postgres,
            kind: WarehouseHistoryKind::ResourceCosts,
            fixture: "resource_cost_history.json",
            fields: ["observation_id", "observed_at", "status"],
            ingest: cost_page,
            rows: [
                ("2026-02-03T04:05:06Z", "partial"),
                ("2026-02-03T04:05:06.000001Z", "complete"),
            ],
            expected_row: ("2026-02-03T04:05:06.000001Z", "complete"),
            expected_column: None,
        },
        MirrorConflictTestCase {
            description: "PostgreSQL resource costs: two rows at the same time",
            backend: Backend::Postgres,
            kind: WarehouseHistoryKind::ResourceCosts,
            fixture: "resource_cost_history.json",
            fields: ["observation_id", "observed_at", "status"],
            ingest: cost_page,
            rows: [
                ("2026-02-03T04:05:06Z", "partial"),
                ("2026-02-03T04:05:06Z", "complete"),
            ],
            expected_row: ("2026-02-03T04:05:06Z", "complete"),
            expected_column: None,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture_store) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            let [id_field, time_field, value_field] = test_case.fields;
            let base = fixture(test_case.fixture)["record"].clone();
            let record = |id: &str, (time, value): (&str, &str)| {
                let mut record = base.clone();
                record[id_field] = json!(id);
                record[time_field] = json!(time);
                record[value_field] = json!(value);
                record
            };
            let page = |record| {
                let page = batch(session_id, test_case.kind.as_str(), vec![record]);
                async move {
                    (test_case.ingest)(store, (location, "development"), &page)
                        .await
                        .expect("one page")
                        .applied
                }
            };
            for (order, rows) in [
                ("arrival order", test_case.rows),
                ("reverse order", [test_case.rows[1], test_case.rows[0]]),
            ] {
                for row in rows {
                    page(record(order, row)).await;
                }
            }
            assert!(
                !page(record("arrival order", test_case.expected_row)).await,
                "{}: re-sending the kept row changes nothing",
                test_case.description
            );
            let (transaction, rows) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                &format!("SELECT * FROM {}", test_case.kind.mirror_table()),
                vec![],
            )
            .await
            .expect("mirrored rows");
            commit(transaction).await.expect("close read");
            let mut kept: Vec<KeptMirrorRow> = rows
                .iter()
                .map(|row| {
                    let record = &row["record_json"];
                    (
                        record[id_field].as_str().expect("id").to_owned(),
                        record[time_field]
                            .as_str()
                            .expect("time")
                            .parse()
                            .expect("RFC 3339"),
                        record[value_field].as_str().expect("value").to_owned(),
                        serde_json::from_value(row["observed_at"].clone()).expect("column time"),
                        row.get(value_field)
                            .and_then(|value| value.as_str())
                            .map(str::to_owned),
                    )
                })
                .collect();
            kept.sort();
            let expected_at: DateTime<Utc> = test_case.expected_row.0.parse().expect("RFC 3339");
            assert_eq!(
                kept,
                ["arrival order", "reverse order"].map(|order| (
                    order.to_owned(),
                    expected_at,
                    test_case.expected_row.1.to_owned(),
                    expected_at,
                    test_case.expected_column.map(str::to_owned)
                )),
                "{}: the kind's winning time, then the smallest record, wins whichever page lands first",
                test_case.description
            );
            close_test_store(fixture_store).await;
        });
    }
}
