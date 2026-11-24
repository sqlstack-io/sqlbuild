//! A seeded target with scoped people, password sessions, fixtures and audit reads for warehouse history API tests.

use chrono::{Duration, Utc};
use futures_util::future::BoxFuture;
use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::models::{ApiAuthentication, HumanLogin};
use crate::api::tests::helpers::start_api_with_authentication;
use crate::api::tests::test_types::TestApi;
use crate::authorization::main::{
    people::bootstrap_user::bootstrap_user, roles::assign_builtin_role::assign_builtin_role,
};
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::set_password::set_password,
};
use crate::credentials::models::{
    AuditContext, BuiltinRole, PasswordAuthority, PasswordLogin, RoleAssignmentRequest,
};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_metadata::main::ingest::ingest_audit_results::ingest_audit_results;
use crate::warehouse_metadata::main::ingest::ingest_resource_costs::ingest_resource_costs;
use crate::warehouse_metadata::models::{MetadataBatch, WarehouseSyncResponse};

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

/// The fixture's observations, each started `hours_ago` before now so the read windows apply.
pub(crate) fn cost_observations(data: &Value) -> Vec<Value> {
    let now = Utc::now();
    data["observations"]
        .as_array()
        .expect("observations")
        .iter()
        .map(|values| {
            let mut record = data["record"].clone();
            for (key, value) in values.as_object().expect("override") {
                record[key] = value.clone();
            }
            let started = now - Duration::hours(values["hours_ago"].as_i64().expect("hours"));
            let attempt = record["attempt"].as_i64().expect("attempt");
            record["run_started_at"] = json!(started);
            record["run_completed_at"] = json!(started + Duration::minutes(10));
            record["observed_at"] = json!(started + Duration::minutes(10 + 5 * attempt));
            record.as_object_mut().expect("record").remove("hours_ago");
            record
        })
        .collect()
}

/// `(key, credits, usd, runs)` of each cost group, amounts rounded to hundredths.
pub(crate) fn cost_groups(summary: &Value, field: &str) -> Vec<(Value, f64, Value, i64)> {
    summary[field]
        .as_array()
        .expect("groups")
        .iter()
        .map(|group| {
            (
                group["key"].clone(),
                (group["credits"].as_f64().expect("credits") * 100.0).round() / 100.0,
                group["usd"]
                    .as_f64()
                    .map_or(Value::Null, |usd| json!((usd * 100.0).round() / 100.0)),
                group["runs"].as_i64().expect("runs"),
            )
        })
        .collect()
}

/// Mirrors one audit results page.
pub(crate) fn audit_page<'a>(
    store: &'a ControlStore,
    scope: (Uuid, &'a str),
    page: &'a MetadataBatch,
) -> BoxFuture<'a, Result<WarehouseSyncResponse, ControlError>> {
    Box::pin(ingest_audit_results(store, scope, page))
}

/// Mirrors one resource costs page.
pub(crate) fn cost_page<'a>(
    store: &'a ControlStore,
    scope: (Uuid, &'a str),
    page: &'a MetadataBatch,
) -> BoxFuture<'a, Result<WarehouseSyncResponse, ControlError>> {
    Box::pin(ingest_resource_costs(store, scope, page))
}

/// Builds of the `daily` job 2 hours and 9 days ago, and one from the command line 3 hours ago at the list price.
pub(crate) fn cost_selection_observations(daily: Uuid, old_daily: Uuid) -> Value {
    let row =
        |id: &str, hours: i64, run: Option<Uuid>, (kind, name): (&str, &str), credits: &str| {
            json!({"observation_id": id, "hours_ago": hours, "run_id": id.split('/').next(),
            "orchestration_run_id": run.map(|run| run.to_string()), "resource_type": kind,
            "resource_name": name, "estimated_credits": credits,
            "estimated_usd": format!("{:.4}", credits.parse::<f64>().expect("credits") * 3.0)})
        };
    let mut data = fixture("resource_cost_history.json");
    data["observations"] = json!([
        row(
            "build-a/total",
            2,
            Some(daily),
            ("_total", "build-a"),
            "0.15"
        ),
        row(
            "build-a/orders",
            2,
            Some(daily),
            ("model", "orders"),
            "0.10"
        ),
        row(
            "build-a/customers",
            2,
            Some(daily),
            ("model", "customers"),
            "0.05"
        ),
        row("build-b/total", 3, None, ("_total", "build-b"), "0.04"),
        row("build-b/orders", 3, None, ("model", "orders"), "0.04"),
        row(
            "build-c/total",
            216,
            Some(old_daily),
            ("_total", "build-c"),
            "0.20"
        ),
        row(
            "build-c/orders",
            216,
            Some(old_daily),
            ("model", "orders"),
            "0.20"
        ),
    ]);
    // The build from the command line was priced from the list price; the job's from the project.
    for observation in data["observations"]
        .as_array_mut()
        .expect("observations")
        .iter_mut()
        .filter(|observation| observation["run_id"] == json!("build-b"))
    {
        observation["usd_per_credit"] = json!("3.00");
        observation["usd_per_credit_source"] = json!("list_price");
    }
    data
}

/// `(job, node, folder, credits, previous)` of each fact, amounts rounded to hundredths.
pub(crate) fn cost_facts(read: &Value) -> Vec<(Value, Value, Value, f64, f64)> {
    let round = |value: &Value| (value.as_f64().expect("amount") * 100.0).round() / 100.0;
    read["facts"]
        .as_array()
        .expect("facts")
        .iter()
        .map(|fact| {
            (
                fact["job"].clone(),
                fact["node_key"].clone(),
                fact["folder"].clone(),
                round(&fact["credits"]),
                round(&fact["previous_credits"]),
            )
        })
        .collect()
}

/// Each series' total, rounded to hundredths.
pub(crate) fn series_totals(series: &Value) -> Vec<f64> {
    series
        .as_array()
        .expect("series")
        .iter()
        .map(|values| {
            let values = values.get("credits").unwrap_or(values);
            (values
                .as_array()
                .expect("days")
                .iter()
                .filter_map(Value::as_f64)
                .sum::<f64>()
                * 100.0)
                .round()
                / 100.0
        })
        .collect()
}

/// `text` percent-encoded for a query value.
pub(crate) fn percent_encoded(text: &str) -> String {
    let mut url = reqwest::Url::parse("http://query.invalid/").expect("url");
    url.query_pairs_mut().append_pair("v", text);
    url.query().expect("query")["v=".len()..].to_owned()
}
