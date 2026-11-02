//! Real durable audit pages retain clock-independent order, live permissions and atomic writes.

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU16, Ordering},
};
use uuid::Uuid;

use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, bootstrap_user::bootstrap_user,
    read_security_audit::read_security_audit,
};
use crate::credentials::main::{
    append_audit::append_audit, create_principal::create_principal, login_password::login_password,
    set_password::set_password,
};
use crate::credentials::models::{
    AdministratorAuthentication, AuditContext, BuiltinRole, PasswordAuthority, PasswordLogin,
    PrincipalKind, RoleAssignmentRequest,
};
use crate::credentials::tests::test_types::{AuditTestCase, AuditWebhookReceiver};
use crate::store::errors::ControlError;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::webhooks::_helpers::delivery::deliver_webhook;
use crate::webhooks::main::connect_webhook::connect_webhook;
use crate::webhooks::models::WebhookSource;

#[test]
fn given_security_audit_pages_when_clock_and_authority_change_then_commit_order_and_atomicity_hold()
{
    let test_cases = [
        AuditTestCase {
            description: "SQLite durable audit pages",
            backend: Backend::Sqlite,
            expected_page_length: 2,
        },
        AuditTestCase {
            description: "PostgreSQL durable audit pages",
            backend: Backend::Postgres,
            expected_page_length: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("database");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let replica = second_store(store).await;
            let context = AuditContext {
                actor: "local-control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some("127.0.0.1".parse().expect("local peer")),
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("bootstrap");
            let person =
                create_principal(store, &context, ("orders-person", PrincipalKind::Person))
                    .await
                    .expect("person");
            let mut sessions = Vec::new();
            for (principal, name) in [(admin, "orders-admin"), (person, "orders-person")] {
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(principal),
                    "orders-password-2026",
                )
                .await
                .expect("password");
                let PasswordLogin::Authenticated(session) =
                    login_password(store, &context, (name, "orders-password-2026"))
                        .await
                        .expect("login")
                else {
                    panic!("valid login");
                };
                sessions.push(session);
            }
            let admin_credentials = (sessions[0].bearer.as_str(), "");
            let ordinary_credentials = (sessions[1].bearer.as_str(), "");
            for role in [
                BuiltinRole::Viewer,
                BuiltinRole::Operator,
                BuiltinRole::Developer,
            ] {
                assert!(
                    assign_builtin_role(
                        store,
                        (
                            &context,
                            (sessions[0].bearer.as_str(), sessions[0].csrf.as_str())
                        ),
                        &RoleAssignmentRequest {
                            principal_id: person,
                            role,
                            location_id: None,
                            target: None,
                            revoke: false,
                        },
                        &AdministratorAuthentication::Password
                    )
                    .await
                    .expect("role assignment")
                );
                assert!(matches!(
                    read_security_audit(&replica, (&context, ordinary_credentials), (0, 100)).await,
                    Err(ControlError::Denied { .. })
                ));
            }
            let existing = read_security_audit(store, (&context, admin_credentials), (0, 100))
                .await
                .expect("existing security facts");
            let mut expected = existing
                .iter()
                .map(|event| (event.occurred_at, event.event_id))
                .collect::<Vec<_>>();
            expected.sort();
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("predecessor");
            for sql in [
                "DROP TABLE sqb_security_audit_deliveries",
                "DELETE FROM sqb_schema_migrations WHERE name='security_audit_sink'",
                "DROP INDEX sqb_freshness_source_latest",
                "DELETE FROM sqb_schema_migrations WHERE name='source_staleness'",
                "DROP INDEX recall_warehouse_sources",
                "DELETE FROM sqb_schema_migrations WHERE name='source_sensors'",
                "DROP TABLE sqb_warehouse_microbatches",
                "DELETE FROM sqb_schema_migrations WHERE name='warehouse_microbatches'",
                "DROP TABLE sqb_warehouse_old_name_views",
                "DROP TABLE sqb_warehouse_migrations",
                "DELETE FROM sqb_schema_migrations WHERE name='warehouse_migrations'",
                "DROP TABLE sqb_warehouse_janitor_events",
                "DELETE FROM sqb_schema_migrations WHERE name='warehouse_janitor_events'",
                "DROP TABLE sqb_warehouse_audit_results",
                "DELETE FROM sqb_schema_migrations WHERE name='warehouse_audit_results'",
                "DROP INDEX sqb_security_audit_storage_id",
                "ALTER TABLE sqb_security_audit DROP COLUMN storage_id",
                "DELETE FROM sqb_schema_migrations WHERE name='audit_storage_order'",
            ] {
                (transaction, _) = query(transaction, sql, vec![])
                    .await
                    .expect("populated predecessor");
            }
            commit(transaction).await.expect("commit predecessor");
            migrate_store(&replica)
                .await
                .expect("additive audit upgrade");
            migrate_store(store)
                .await
                .expect("idempotent audit upgrade");
            let upgraded = read_security_audit(&replica, (&context, admin_credentials), (0, 100))
                .await
                .expect("backfilled audit");
            assert_eq!(
                upgraded
                    .iter()
                    .map(|event| event.event_id)
                    .collect::<Vec<_>>(),
                expected.into_iter().map(|(_, id)| id).collect::<Vec<_>>()
            );
            assert_eq!(
                upgraded
                    .iter()
                    .map(|event| event.storage_id)
                    .collect::<Vec<_>>(),
                (1..=upgraded.len() as i64).collect::<Vec<_>>()
            );
            let cursor = upgraded.last().expect("populated history").storage_id;
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                "../../benchmarks/duckshop/edge_cases/orchestration/security_audit_order.json",
            );
            let inputs: Value = serde_json::from_str(
                &std::fs::read_to_string(path).expect("DuckShop clock fixture"),
            )
            .expect("clock inputs");
            let mut ids = Vec::new();
            for (index, timestamp) in inputs["occurred_at"]
                .as_array()
                .expect("clock steps")
                .iter()
                .enumerate()
            {
                let mut transaction = begin(store, TransactionMode::Write)
                    .await
                    .expect("append clock fact");
                transaction.now = timestamp
                    .as_str()
                    .expect("timestamp")
                    .parse::<DateTime<Utc>>()
                    .expect("clock value");
                let id = Uuid::now_v7();
                ids.push(id);
                let transaction = append_audit(
                    transaction,
                    &context,
                    (id, "audit.cursor_fixture"),
                    (
                        json!({"observation":index}),
                        json!(null),
                        json!({"recorded":true}),
                    ),
                )
                .await
                .expect("durable clock fact");
                commit(transaction).await.expect("commit clock fact");
            }
            let first = read_security_audit(&replica, (&context, admin_credentials), (cursor, 2))
                .await
                .expect("first bounded page");
            assert_eq!(
                first.len(),
                test_case.expected_page_length,
                "{}",
                test_case.description
            );
            assert_eq!(
                first.iter().map(|event| event.event_id).collect::<Vec<_>>(),
                ids[..2]
            );
            assert!(first[0].occurred_at > first[1].occurred_at);
            assert_eq!(first[0].storage_id, cursor + 1);
            assert_eq!(first[1].storage_id, cursor + 2);
            assert_eq!(first[1].actor, context.actor);
            assert_eq!(first[1].granting_role, context.granting_role);
            assert_eq!(first[1].request_id, context.request_id);
            assert_eq!(first[1].ip_address.as_deref(), Some("127.0.0.1"));
            assert_eq!(first[1].scope_json, json!({"observation":1}));
            assert_eq!(first[1].before_json, json!(null));
            assert_eq!(first[1].after_json, json!({"recorded":true}));
            let last = read_security_audit(
                store,
                (&context, admin_credentials),
                (first[1].storage_id, 2),
            )
            .await
            .expect("strict continuation");
            assert_eq!(
                last.iter().map(|event| event.event_id).collect::<Vec<_>>(),
                ids[2..]
            );
            let cursor = last[0].storage_id;
            assert!(
                read_security_audit(store, (&context, admin_credentials), (cursor, 100))
                    .await
                    .expect("empty continuation")
                    .is_empty()
            );
            for bounds in [(-1, 1), (0, 0), (0, 101)] {
                assert!(matches!(
                    read_security_audit(store, (&context, admin_credentials), bounds).await,
                    Err(ControlError::Invalid(_))
                ));
            }
            let fault = TransactionFault {
                description: "audit append fails atomically",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT, 'audit unavailable')",
                postgres_body: "RAISE EXCEPTION 'audit unavailable'",
            };
            install_fault(store, &fault).await;
            assert!(
                create_principal(store, &context, ("orders-rollback", PrincipalKind::Person))
                    .await
                    .is_err()
            );
            remove_fault(store, &fault).await;
            assert!(
                read_security_audit(&replica, (&context, admin_credentials), (cursor, 100))
                    .await
                    .expect("failed audit did not publish")
                    .is_empty()
            );
            create_principal(store, &context, ("orders-rollback", PrincipalKind::Person))
                .await
                .expect("failed principal did not persist");
            let after = read_security_audit(store, (&context, admin_credentials), (cursor, 100))
                .await
                .expect("successful retry");
            assert_eq!(after.len(), 1);
            assert_eq!(after[0].storage_id, cursor + 1);
            let cursor = after[0].storage_id;
            let (left, right) = tokio::join!(
                create_principal(
                    store,
                    &context,
                    ("orders-concurrent", PrincipalKind::Person)
                ),
                create_principal(
                    &replica,
                    &context,
                    ("customers-concurrent", PrincipalKind::Person)
                ),
            );
            assert_ne!(
                left.expect("first concurrent append"),
                right.expect("second concurrent append")
            );
            let concurrent =
                read_security_audit(store, (&context, admin_credentials), (cursor, 100))
                    .await
                    .expect("serialized writers");
            assert_eq!(
                concurrent
                    .iter()
                    .map(|event| event.storage_id)
                    .collect::<Vec<_>>(),
                vec![cursor + 1, cursor + 2]
            );
            assert!(
                concurrent
                    .iter()
                    .all(|event| event.action == "principal.created")
            );
            assert!(
                assign_builtin_role(
                    store,
                    (
                        &context,
                        (sessions[0].bearer.as_str(), sessions[0].csrf.as_str())
                    ),
                    &RoleAssignmentRequest {
                        principal_id: person,
                        role: BuiltinRole::Admin,
                        location_id: None,
                        target: None,
                        revoke: false,
                    },
                    &AdministratorAuthentication::Password
                )
                .await
                .expect("second administrator")
            );
            assert!(
                assign_builtin_role(
                    store,
                    (
                        &context,
                        (sessions[1].bearer.as_str(), sessions[1].csrf.as_str())
                    ),
                    &RoleAssignmentRequest {
                        principal_id: admin,
                        role: BuiltinRole::Admin,
                        location_id: None,
                        target: None,
                        revoke: true,
                    },
                    &AdministratorAuthentication::Password
                )
                .await
                .expect("live role revocation")
            );
            assert!(matches!(
                read_security_audit(&replica, (&context, admin_credentials), (0, 100)).await,
                Err(ControlError::Denied { .. })
            ));
            assert!(
                !read_security_audit(&replica, (&context, ordinary_credentials), (0, 100))
                    .await
                    .expect("replacement administrator reads audit")
                    .is_empty()
            );
            set_password(
                store,
                &context,
                PasswordAuthority::Operator(person),
                "orders-replacement-2026",
            )
            .await
            .expect("revoke old sessions");
            assert!(matches!(
                read_security_audit(&replica, (&context, ordinary_credentials), (0, 100)).await,
                Err(ControlError::Denied { .. })
            ));
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_committed_security_facts_when_export_retries_then_only_successful_receipts_advance_commit_order()
 {
    let test_cases = [
        AuditTestCase {
            description: "SQLite durable audit export",
            backend: Backend::Sqlite,
            expected_page_length: 100,
        },
        AuditTestCase {
            description: "PostgreSQL durable audit export",
            backend: Backend::Postgres,
            expected_page_length: 100,
        },
    ];
    let scenario: Value = serde_json::from_str(
        &std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../benchmarks/duckshop/edge_cases/orchestration/security_audit_delivery.json",
        ))
        .expect("delivery fixture"),
    )
    .expect("delivery scenario");
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("database");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "principal:orders-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some("127.0.0.1".parse().expect("peer")),
            };
            let mut expected = Vec::new();
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("audit action");
            for index in 0..scenario["events"].as_i64().expect("count") {
                // Deliberately decreasing event clocks must not change delivery order.
                transaction.now = "2026-01-01T00:00:00Z"
                    .parse::<DateTime<Utc>>()
                    .expect("clock")
                    - chrono::TimeDelta::seconds(index);
                let id = Uuid::now_v7();
                expected.push(json!({"storage_id":index+1,"event_id":id,"occurred_at":transaction.now,
                    "actor":context.actor,"granting_role":context.granting_role,"request_id":context.request_id,
                    "ip_address":"127.0.0.1","action":scenario["action"],"scope_json":scenario["scope"],
                    "before_json":scenario["before"],"after_json":scenario["after"]}));
                transaction = append_audit(
                    transaction,
                    &context,
                    (id, scenario["action"].as_str().expect("action")),
                    (
                        scenario["scope"].clone(),
                        scenario["before"].clone(),
                        scenario["after"].clone(),
                    ),
                )
                .await
                .expect("append");
            }
            commit(transaction).await.expect("commit source actions");
            let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
            let status = Arc::new(AtomicU16::new(503));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("listener");
            let endpoint = format!(
                "http://audit:sample-password@{}/audit?token=sample-token",
                listener.local_addr().expect("address")
            );
            let app = Router::new()
                .route(
                    "/audit",
                    post(
                        |State(AuditWebhookReceiver { requests, status }): State<
                            AuditWebhookReceiver,
                        >,
                         headers: HeaderMap,
                         Json(event): Json<Value>| async move {
                            assert_eq!(
                                headers
                                    .get("Idempotency-Key")
                                    .expect("key")
                                    .to_str()
                                    .expect("text"),
                                event["event_id"].as_str().expect("event id")
                            );
                            requests.lock().expect("requests").push(event);
                            (
                                StatusCode::from_u16(status.load(Ordering::SeqCst))
                                    .expect("status"),
                                [(axum::http::header::LOCATION, "/audit")],
                            )
                        },
                    ),
                )
                .with_state(AuditWebhookReceiver {
                    requests: requests.clone(),
                    status: status.clone(),
                });
            let (stop, shutdown) = tokio::sync::oneshot::channel();
            let server = tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        drop(shutdown.await);
                    })
                    .await
            });
            let sink =
                connect_webhook(&endpoint, WebhookSource::SecurityAudit).expect("audit sink");
            assert!(!format!("{sink:?}").contains("sample-password"));
            assert!(deliver_webhook(store, &sink).await.is_err());
            assert_eq!(
                requests.lock().expect("requests").len(),
                1,
                "failure stops this page"
            );
            status.store(307, Ordering::SeqCst);
            assert!(
                deliver_webhook(store, &sink).await.is_err(),
                "never redirect audit facts"
            );
            assert_eq!(requests.lock().expect("requests").len(), 2);
            status.store(204, Ordering::SeqCst);
            let fault = TransactionFault {
                description: "lost security export acknowledgement",
                table: "sqb_security_audit_deliveries",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT, 'test acknowledgement failure')",
                postgres_body: "RAISE EXCEPTION 'test acknowledgement failure'",
            };
            install_fault(store, &fault).await;
            assert!(deliver_webhook(store, &sink).await.is_err());
            remove_fault(store, &fault).await;
            let replica = second_store(store).await;
            let restarted =
                connect_webhook(&endpoint, WebhookSource::SecurityAudit).expect("restart");
            assert_eq!(
                deliver_webhook(&replica, &restarted)
                    .await
                    .expect("first bounded page"),
                test_case.expected_page_length,
                "{}",
                test_case.description
            );
            assert_eq!(
                deliver_webhook(&replica, &restarted)
                    .await
                    .expect("remaining page"),
                5
            );
            assert_eq!(
                deliver_webhook(&replica, &restarted)
                    .await
                    .expect("durable cursor"),
                0
            );
            // Identical endpoint, independent platform stream: security cursor grants no authority.
            let platform =
                connect_webhook(&endpoint, WebhookSource::Platform).expect("platform sink");
            assert_eq!(
                deliver_webhook(&replica, &platform)
                    .await
                    .expect("independent cursor"),
                0
            );
            {
                let received = requests.lock().expect("requests");
                assert_eq!(received.len(), expected.len() + 3);
                assert_eq!(received[0], expected[0]);
                assert_eq!(received[1], expected[0]);
                assert_eq!(
                    received[2], expected[0],
                    "lost acknowledgement retains full identity"
                );
                assert_eq!(
                    received[3..],
                    expected,
                    "every field survives; storage order beats clocks"
                );
            }
            stop.send(()).expect("stop receiver");
            server
                .await
                .expect("receiver task")
                .expect("receiver stopped");
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
