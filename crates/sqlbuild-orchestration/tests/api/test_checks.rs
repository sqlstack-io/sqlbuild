//! Check reads through the public API: keyed by binding key, view-gated, with audited SQL.

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::checks_helpers::{ORIGIN, PASSWORD, fixture, get, latest_audit, login};
use crate::api::tests::helpers::{start_api_with_authentication, stop_api};
use crate::api::tests::test_types::WarehouseMetadataTestCase;
use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, bootstrap_user::bootstrap_user,
};
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::set_password::set_password,
};
use crate::credentials::models::{
    AuditContext, BuiltinRole, PasswordAuthority, PasswordLogin, RoleAssignmentRequest,
};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest_audit_results::ingest_audit_results;
use crate::warehouse_metadata::models::MetadataBatch;

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
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some("127.0.0.1".parse().expect("local peer")),
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("bootstrap");
            let PasswordLogin::Authenticated(admin_session) = ({
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(admin),
                    PASSWORD,
                )
                .await
                .expect("admin password");
                login_password(store, &context, ("orders-admin", PASSWORD))
                    .await
                    .expect("admin login")
            }) else {
                panic!("authenticated administrator");
            };
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            for (name, role) in [
                ("orders-viewer", BuiltinRole::Viewer),
                ("orders-developer", BuiltinRole::Developer),
            ] {
                let person = create_principal(store, &context, name)
                    .await
                    .expect("person");
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(person),
                    PASSWORD,
                )
                .await
                .expect("password");
                assign_builtin_role(
                    store,
                    (&context, admin_session.identity.principal_id),
                    &RoleAssignmentRequest {
                        principal_id: person,
                        role: role.into(),
                        location_id: Some(location),
                        target: Some("development".into()),
                        revoke: false,
                    },
                )
                .await
                .expect("scoped role");
            }
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("snapshot");
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
            .expect("reader");
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
            let batch: MetadataBatch = serde_json::from_value(json!({"kind":"audit_results","session_id":session_id,"generation":1,"records":records,
                "scan":{"scan_id":Uuid::now_v7(),"location":{"database":null,"schema":"main"},"complete":false}})).expect("batch");
            assert!(
                ingest_audit_results(store, (location, "development"), &batch)
                    .await
                    .expect("real audit ingestion")
                    .applied
            );

            let api = start_api_with_authentication(
                store,
                ApiAuthentication {
                    login: HumanLogin::Password(ORIGIN.into()),
                    ..Default::default()
                },
            )
            .await;
            let client = Client::new();
            let viewer = login(&client, &api.url, "orders-viewer").await;
            let developer = login(&client, &api.url, "orders-developer").await;
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
            stop_api(api).await;
            close_test_store(fixture_store).await;
        });
    }
}
