//! A seeded target with scoped people, password sessions, fixtures and audit reads for warehouse history API tests.

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::helpers::start_api_with_authentication;
use crate::api::tests::test_types::TestApi;
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
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::models::MetadataBatch;

pub(crate) const ORIGIN: &str = "https://control.example.com";
pub(crate) const PASSWORD: &str = "orders-password-2026";

pub(crate) fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("fixture JSON")
}

pub(crate) async fn get(
    client: &Client,
    (url, cookie): (&str, &str),
    path: &str,
) -> (StatusCode, Value) {
    let response = client
        .get(format!("{url}{path}"))
        .header(header::HOST, "control.example.com")
        .header(header::ORIGIN, ORIGIN)
        .header("cookie", cookie)
        .send()
        .await
        .expect("check read");
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

pub(crate) async fn login(client: &Client, url: &str, username: &str) -> String {
    let response = client
        .post(format!("{url}/api/auth/password/login"))
        .header(header::HOST, "control.example.com")
        .header(header::ORIGIN, ORIGIN)
        .json(&json!({"username": username, "password": PASSWORD}))
        .send()
        .await
        .expect("password login");
    assert_eq!(response.status(), StatusCode::OK);
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().expect("cookie"))
        .find(|value| value.starts_with("__Host-sqb-session="))
        .expect("browser session")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_owned()
}

pub(crate) async fn latest_audit(store: &ControlStore) -> Value {
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("audit cut");
    let (transaction, rows) = query(transaction,
        "SELECT action,granting_role,scope_json,after_json FROM sqb_security_audit ORDER BY storage_id DESC LIMIT 1",
        vec![]).await.expect("audit");
    commit(transaction).await.expect("close audit cut");
    json!(rows[0])
}

/// A migrated store with scoped people, the orders snapshot and a live metadata reader session.
pub(crate) async fn seed_target(store: &ControlStore) -> (Uuid, Uuid) {
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
    (location, session_id)
}

/// One metadata batch of `kind` from the reader session, scanning the `main` state schema.
pub(crate) fn batch(session_id: Uuid, kind: &str, records: Vec<Value>) -> MetadataBatch {
    serde_json::from_value(json!({"kind":kind,"session_id":session_id,"generation":1,"records":records,
        "scan":{"scan_id":Uuid::now_v7(),"location":{"database":null,"schema":"main"},"complete":false}})).expect("batch")
}

/// An API with password sign-in and a client for it.
pub(crate) async fn start_password_api(store: &ControlStore) -> (TestApi, Client) {
    let api = start_api_with_authentication(
        store,
        ApiAuthentication {
            login: HumanLogin::Password(ORIGIN.into()),
            ..Default::default()
        },
    )
    .await;
    (api, Client::new())
}

/// Fixture records of `base` with each override applied.
pub(crate) fn records(base: &str, overrides: &Value) -> Vec<Value> {
    let base = fixture(base)["record"].clone();
    overrides
        .as_array()
        .expect("overrides")
        .iter()
        .map(|values| {
            let mut record = base.clone();
            for (key, value) in values.as_object().expect("override") {
                record[key] = value.clone();
            }
            record
        })
        .collect()
}

/// The event IDs of one kind of record in a history response.
pub(crate) fn ids(history: &Value, kind: &str) -> Vec<String> {
    history[kind]
        .as_array()
        .expect("records")
        .iter()
        .map(|record| record["event_id"].as_str().expect("event").to_owned())
        .collect()
}

/// One microbatch coverage batch as the API returns it.
pub(crate) fn coverage_batch(start: &str, end: &str, state: &str, run_id: Value) -> Value {
    json!({"start": start, "end": end, "state": state, "run_id": run_id})
}
