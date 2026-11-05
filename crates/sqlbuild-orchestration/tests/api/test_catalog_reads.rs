//! Real ingestion, scoped person grants and namespace keyset pages on both control stores.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::tests::test_types::WarehouseMetadataTestCase;
use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, bootstrap_user::bootstrap_user,
};
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::set_password::set_password,
};
use crate::credentials::models::{
    AuditContext, BuiltinRole, DefaultRole, PasswordAuthority, PasswordLogin, RoleAssignmentRequest,
};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::{
    ingest_catalog::ingest_catalog, read_person_catalog::read_person_catalog,
};
use crate::warehouse_metadata::models::{CatalogQuery, MetadataBatch};

#[test]
fn given_catalog_observations_when_people_page_then_live_grants_and_exact_namespaces_bound_every_result()
 {
    let test_cases = [
        WarehouseMetadataTestCase {
            description: "SQLite catalog reads",
            backend: Backend::Sqlite,
            expected_observations: 205,
        },
        WarehouseMetadataTestCase {
            description: "PostgreSQL catalog reads",
            backend: Backend::Postgres,
            expected_observations: 205,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("store");
            let store = &fixture.store;
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
            let person = create_principal(store, &context, "orders-reader")
                .await
                .expect("person");
            let mut sessions = Vec::new();
            for (id, name) in [(admin, "orders-admin"), (person, "orders-reader")] {
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(id),
                    "orders-password-2026",
                )
                .await
                .expect("password");
                let PasswordLogin::Authenticated(session) =
                    login_password(store, &context, (name, "orders-password-2026"))
                        .await
                        .expect("login")
                else {
                    panic!("authenticated");
                };
                sessions.push(session);
            }
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let other = create_location(store, "inventory", "test-admin")
                .await
                .expect("other location");
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/warehouse_catalog.json");
            let data: Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("fixture"))
                    .expect("catalog inputs");
            let expected: Vec<String> = (0..test_case.expected_observations)
                .map(|index| format!("orders_🧾_{index:03}"))
                .collect();
            for (scope, target) in [
                (location, "development"),
                (location, "preview"),
                (other, "development"),
            ] {
                let mut input = snapshot(scope);
                input.target = target.into();
                publish_snapshot(store, &input).await.expect("snapshot");
                let session_id = Uuid::now_v7();
                report_location_health(
                    store,
                    scope,
                    &LocationHealthReport {
                        event_id: Uuid::now_v7(),
                        session_id,
                        target: target.into(),
                        health: LocationHealth::Heartbeat {},
                    },
                )
                .await
                .expect("reader");
                for (namespace, count) in [
                    (&data["read_namespaces"][0], test_case.expected_observations),
                    (&data["read_namespaces"][1], 1),
                    (&data["read_namespaces"][2], 1),
                ] {
                    let records: Vec<Value> = (0..count)
                        .map(|index| {
                            let mut record = data["record"].clone();
                            record["name"] = json!(format!("orders_🧾_{index:03}"));
                            record["state_database"] = namespace["database"].clone();
                            record["state_schema"] = namespace["schema"].clone();
                            record
                        })
                        .collect();
                    for records in records.chunks(100) {
                        let batch: MetadataBatch = serde_json::from_value(json!({"kind":"catalog","session_id":session_id,"generation":1,"records":records,
                            "scan":{"scan_id":Uuid::now_v7(),"location":{"database":namespace["database"],"schema":namespace["schema"]},"complete":false}})).expect("batch");
                        assert!(
                            ingest_catalog(store, (scope, target), &batch)
                                .await
                                .expect("real catalog ingestion")
                                .applied
                        );
                    }
                }
            }
            let credentials = (&context, (sessions[1].bearer.as_str(), ""));
            let mut options = CatalogQuery {
                database: None,
                schema: "main".into(),
                after_observed_at: None,
                after_event_id: None,
                limit: Some(u32::MAX),
            };
            assert!(matches!(
                read_person_catalog(
                    store,
                    credentials,
                    (location, "development", &options),
                    DefaultRole::None
                )
                .await,
                Err(ControlError::Denied { .. })
            ));
            let mut assignment = RoleAssignmentRequest {
                principal_id: person,
                role: BuiltinRole::Viewer.into(),
                location_id: Some(location),
                target: Some("development".into()),
                revoke: false,
            };
            assign_builtin_role(
                store,
                (&context, (&sessions[0].bearer, &sessions[0].csrf)),
                &assignment,
            )
            .await
            .expect("scoped viewer");
            let reader_session = crate::api::tests::fingerprint_reads_helpers::fingerprint_reads(
                store,
                credentials,
                (location, other),
            )
            .await;
            crate::api::tests::statistics_helpers::statistics_reads(
                store,
                credentials,
                (location, other, reader_session),
            )
            .await;
            let mut actual = Vec::new();
            for _ in 0..2 {
                let page = read_person_catalog(
                    store,
                    credentials,
                    (location, "development", &options),
                    DefaultRole::None,
                )
                .await
                .expect("bounded page");
                assert_eq!(page.records.len(), 100, "{}", test_case.description);
                let after = page.next_after.expect("more rows remain");
                options.after_observed_at = Some(after.observed_at);
                options.after_event_id = Some(after.event_id);
                actual.extend(page.records);
            }
            let last = read_person_catalog(
                store,
                credentials,
                (location, "development", &options),
                DefaultRole::None,
            )
            .await
            .expect("last page");
            assert_eq!(last.records.len(), 5);
            assert!(last.next_after.is_none());
            actual.extend(last.records);
            assert_eq!(actual.len(), test_case.expected_observations);
            for row in &actual {
                assert_eq!(json!(row.record.columns), data["record"]["columns"]);
            }
            let mut names = actual
                .into_iter()
                .map(|row| row.record.name)
                .collect::<Vec<_>>();
            names.sort();
            assert_eq!(
                names, expected,
                "ties and namespace exclusions preserve each exact name once"
            );
            options.after_observed_at = None;
            options.after_event_id = None;
            options.limit = Some(0);
            assert_eq!(
                read_person_catalog(
                    store,
                    credentials,
                    (location, "development", &options),
                    DefaultRole::None
                )
                .await
                .expect("minimum page")
                .records
                .len(),
                1
            );
            options.database = Some("orders_data".into());
            let exact = read_person_catalog(
                store,
                credentials,
                (location, "development", &options),
                DefaultRole::None,
            )
            .await
            .expect("exact database");
            assert_eq!(exact.records.len(), 1);
            assert_eq!(
                exact.records[0].record.state_database.as_deref(),
                Some("orders_data")
            );
            let after = exact.next_after.expect("full page needs continuation");
            options.after_observed_at = Some(after.observed_at);
            options.after_event_id = Some(after.event_id);
            let empty = read_person_catalog(
                store,
                credentials,
                (location, "development", &options),
                DefaultRole::None,
            )
            .await
            .expect("exact multiple ends with empty page");
            assert!(empty.records.is_empty());
            assert!(empty.next_after.is_none());
            options.after_observed_at = None;
            options.after_event_id = None;
            for scope in [(other, "development"), (location, "preview")] {
                assert!(matches!(
                    read_person_catalog(
                        store,
                        credentials,
                        (scope.0, scope.1, &options),
                        DefaultRole::None
                    )
                    .await,
                    Err(ControlError::Denied { .. })
                ));
            }
            options.after_event_id = Some(Uuid::now_v7());
            assert!(matches!(
                read_person_catalog(
                    store,
                    credentials,
                    (location, "development", &options),
                    DefaultRole::None
                )
                .await,
                Err(ControlError::Invalid(_))
            ));
            crate::api::tests::node_status_helpers::node_status_reads(
                store,
                credentials,
                (location, reader_session),
            )
            .await;
            assignment.revoke = true;
            assign_builtin_role(
                store,
                (&context, (&sessions[0].bearer, &sessions[0].csrf)),
                &assignment,
            )
            .await
            .expect("revoke");
            assert!(matches!(
                crate::warehouse_metadata::main::read_person_statistics::read_person_statistics(
                    store,
                    credentials,
                    (
                        location,
                        "development",
                        &crate::warehouse_metadata::models::StatisticsQuery {
                            database: None,
                            schema: "main".into(),
                            name: "orders_🧾_000".into(),
                            after_observed_at: None,
                            after_event_id: None,
                            limit: None,
                        }
                    ),
                    (DefaultRole::None, Default::default()),
                )
                .await,
                Err(ControlError::Denied { .. })
            ));
            assert!(matches!(
                crate::warehouse_metadata::main::read_person_node_status::read_person_node_status(
                    store,
                    credentials,
                    (
                        location,
                        "development",
                        &crate::warehouse_metadata::models::NodeStatusQuery {
                            node_key: "model:orders".into()
                        }
                    ),
                    DefaultRole::None,
                )
                .await,
                Err(ControlError::Denied { .. })
            ));
            let evidence = crate::warehouse_metadata::models::FingerprintQuery {
                node_type: "model".into(),
                node_name: "orders".into(),
                database: None,
                schema: Some("main".into()),
                name: Some("orders".into()),
            };
            assert!(matches!(
                crate::warehouse_metadata::main::read_person_fingerprint::read_person_fingerprint(
                    store,
                    credentials,
                    (location, "development", &evidence),
                    DefaultRole::None,
                )
                .await,
                Err(ControlError::Denied { .. })
            ));
            assert!(
                matches!(
                    read_person_catalog(
                        store,
                        credentials,
                        (location, "development", &options),
                        DefaultRole::None
                    )
                    .await,
                    Err(ControlError::Denied { .. })
                ),
                "authorization precedes even invalid cursor handling"
            );
            options.after_event_id = None;
            assert_eq!(
                read_person_catalog(
                    store,
                    credentials,
                    (location, "development", &options),
                    DefaultRole::Viewer
                )
                .await
                .expect("configured viewer default")
                .records
                .len(),
                1
            );
            close_test_store(fixture).await;
        });
    }
}
