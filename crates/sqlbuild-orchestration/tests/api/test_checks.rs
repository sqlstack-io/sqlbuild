//! Check reads through the public API: keyed by binding key, view-gated, with audited SQL.

use reqwest::StatusCode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::tests::helpers::stop_api;
use crate::api::tests::test_types::WarehouseMetadataTestCase;
use crate::api::tests::warehouse_history_helpers::{
    batch, fixture, get, latest_audit, login, seed_target, start_password_api,
};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest_audit_results::ingest_audit_results;

#[test]
fn given_checks_sharing_an_audit_name_when_people_read_them_then_binding_keys_and_sql_permissions_hold()
 {
    let test_cases = [
        WarehouseMetadataTestCase {
            description: "SQLite check reads",
            backend: Backend::Sqlite,
            expected_observations: 4,
        },
        WarehouseMetadataTestCase {
            description: "PostgreSQL check reads",
            backend: Backend::Postgres,
            expected_observations: 4,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture_store = open_test_store(test_case.backend).await.expect("store");
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            let data = fixture("check_identity.json");
            let base = fixture("audit_result_history.json")["record"].clone();
            let records: Vec<Value> = data["results"]
                .as_array()
                .expect("results")
                .iter()
                .map(|result| {
                    let check = &data["checks"][result["check"].as_u64().expect("check") as usize];
                    let mut record = base.clone();
                    for (key, value) in check.as_object().expect("check").iter().chain(
                        result
                            .as_object()
                            .expect("result")
                            .iter()
                            .filter(|(key, _)| *key != "check"),
                    ) {
                        record[key] = value.clone();
                    }
                    record["audit_name"] = data["audit_name"].clone();
                    record
                })
                .collect();
            assert_eq!(records.len(), test_case.expected_observations);
            assert!(
                ingest_audit_results(
                    store,
                    (location, "development"),
                    &batch(session_id, "audit_results", records)
                )
                .await
                .expect("real audit ingestion")
                .applied
            );

            let (api, client) = start_password_api(store).await;
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let developer = login(&client, &api.url, "orders-developer").await;
            let administrator = login(&client, &api.url, "orders-admin").await;
            let base_path = format!("/api/control/locations/{location}/targets/development/checks");
            let orders_key = data["checks"][0]["binding_key"].as_str().expect("key");
            let customers_key = data["checks"][1]["binding_key"].as_str().expect("key");
            let archived_key = data["checks"][2]["binding_key"].as_str().expect("key");

            let (status, list) = get(&client, (&api.url, &viewer), &base_path).await;
            assert_eq!(status, StatusCode::OK, "{}", test_case.description);
            let keys: Vec<(&str, &str, &str)> = list
                .as_array()
                .expect("checks")
                .iter()
                .map(|check| {
                    (
                        check["binding_key"].as_str().expect("key"),
                        check["outcome"].as_str().expect("outcome"),
                        check["audit_name"].as_str().expect("name"),
                    )
                })
                .collect();
            assert_eq!(
                keys,
                [
                    (customers_key, "error", "not_null"),
                    (orders_key, "pass", "not_null"),
                    (archived_key, "pass", "not_null")
                ],
                "one entry per binding key, each its newest result"
            );
            for disclosed in ["SELECT", "private@example.com"] {
                assert!(
                    !list.to_string().contains(disclosed),
                    "listing never discloses SQL, errors quoting it, or evidence"
                );
            }

            let (status, history) = get(
                &client,
                (&api.url, &viewer),
                &format!("{base_path}/{orders_key}"),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            let outcomes: Vec<&str> = history
                .as_array()
                .expect("history")
                .iter()
                .map(|result| result["outcome"].as_str().expect("outcome"))
                .collect();
            assert_eq!(outcomes, ["pass", "fail"], "newest first, own key only");
            assert_eq!(history[1]["violation_count"], 3);
            let (_, limited) = get(
                &client,
                (&api.url, &viewer),
                &format!("{base_path}/{orders_key}?limit=1"),
            )
            .await;
            assert_eq!(limited.as_array().expect("limited").len(), 1);
            let (status, unknown) = get(
                &client,
                (&api.url, &viewer),
                &format!("{base_path}/unknown-check"),
            )
            .await;
            assert_eq!((status, unknown), (StatusCode::OK, json!([])));

            let sql_path = format!("{base_path}/{orders_key}/sql");
            let (status, _) = get(&client, (&api.url, &viewer), &sql_path).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "viewers lack view.sql");
            let denial = latest_audit(store).await;
            assert_eq!(denial["action"], "authorization.denied");
            assert_eq!(denial["scope_json"]["action"], "view.sql");
            assert_eq!(denial["scope_json"]["nodes"], json!(["orders"]));

            let (status, sql) = get(&client, (&api.url, &developer), &sql_path).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(sql["binding_key"], orders_key);
            assert_eq!(sql["executed_sql"], data["results"][1]["executed_sql"]);
            assert_eq!(sql["event_id"], history[0]["event_id"]);
            let read = latest_audit(store).await;
            assert_eq!(read["action"], "sql.read");
            assert_eq!(read["granting_role"], "developer");
            assert_eq!(read["scope_json"]["nodes"], json!(["orders"]));
            assert_eq!(
                read["after_json"],
                json!({"binding_key": orders_key, "event_id": history[0]["event_id"], "found": true})
            );
            assert!(
                !read.to_string().contains("IS NULL"),
                "the audit never holds SQL"
            );

            let (status, failed) = get(
                &client,
                (&api.url, &developer),
                &format!("{base_path}/{customers_key}/sql"),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                failed["execution_error"],
                data["results"][2]["execution_error"]
            );
            let (status, _) = get(
                &client,
                (&api.url, &viewer),
                &format!("{base_path}/{archived_key}/sql"),
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN);
            assert_eq!(
                latest_audit(store).await["scope_json"]["nodes"],
                json!(["customers", "inventory", "orders", "products"]),
                "a check on a node outside the snapshot needs the whole target"
            );

            let (status, missing) = get(
                &client,
                (&api.url, &developer),
                &format!("{base_path}/unknown-check/sql"),
            )
            .await;
            assert_eq!((status, missing), (StatusCode::OK, Value::Null));
            let read = latest_audit(store).await;
            assert_eq!(read["action"], "sql.read");
            assert_eq!(read["after_json"]["found"], false);

            for path in [
                base_path.replace("development", "preview"),
                base_path.replace(&location.to_string(), &Uuid::now_v7().to_string()),
            ] {
                assert_eq!(
                    get(&client, (&api.url, &viewer), &path).await.0,
                    StatusCode::FORBIDDEN
                );
            }
            let failed_id = history[1]["event_id"].as_str().expect("event");
            let evidence_path = format!("{base_path}/{orders_key}/results/{failed_id}/evidence");
            let (status, _) = get(&client, (&api.url, &developer), &evidence_path).await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "developers lack view.evidence"
            );
            let denial = latest_audit(store).await;
            assert_eq!(denial["scope_json"]["action"], "view.evidence");
            assert_eq!(denial["scope_json"]["nodes"], json!(["orders"]));
            let (status, evidence) = get(&client, (&api.url, &administrator), &evidence_path).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                evidence,
                json!({"event_id": failed_id, "binding_key": orders_key,
                    "columns": ["amount", "is_late", "order_id"],
                    "rows": [[1.5, null, "o-1"], [null, true, "o-2"]], "truncated": "false"}),
                "sorted keys become columns; missing keys become null cells"
            );
            let read = latest_audit(store).await;
            assert_eq!(read["action"], "evidence.read");
            assert_eq!(
                read["after_json"],
                json!({"binding_key": orders_key, "event_id": failed_id, "found": true})
            );
            assert!(
                !read.to_string().contains("o-1"),
                "the audit never holds evidence"
            );
            let (status, mismatched) = get(
                &client,
                (&api.url, &administrator),
                &evidence_path.replace(orders_key, customers_key),
            )
            .await;
            assert_eq!(
                (status, mismatched),
                (StatusCode::OK, Value::Null),
                "a result is read only under its own check"
            );
            let (_, archived) = get(
                &client,
                (&api.url, &administrator),
                &format!("{base_path}/{archived_key}"),
            )
            .await;
            let malformed_path = format!(
                "{base_path}/{archived_key}/results/{}/evidence",
                archived[0]["event_id"].as_str().expect("event")
            );
            let (status, refusal) = get(&client, (&api.url, &administrator), &malformed_path).await;
            assert_eq!(
                (status, refusal),
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error": "control-store operation failed"})
                ),
                "malformed stored evidence is never echoed"
            );
            let fault = TransactionFault {
                description: "evidence read requires durable audit",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "WHEN NEW.action='evidence.read'",
                sqlite_body: "SELECT RAISE(ABORT, 'synthetic evidence audit failure')",
                postgres_body: "RAISE EXCEPTION 'synthetic evidence audit failure'",
            };
            install_fault(store, &fault).await;
            let (status, refusal) = get(&client, (&api.url, &administrator), &evidence_path).await;
            assert_eq!(
                status,
                StatusCode::INTERNAL_SERVER_ERROR,
                "{}",
                fault.description
            );
            assert!(
                !refusal.to_string().contains("o-1"),
                "no evidence without its audit"
            );
            remove_fault(store, &fault).await;

            stop_api(api).await;
            close_test_store(fixture_store).await;
        });
    }
}
