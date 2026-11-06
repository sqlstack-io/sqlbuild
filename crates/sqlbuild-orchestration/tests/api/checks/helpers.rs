//! Password sessions, fixture loading and audit reads for the check API tests.

use reqwest::{Client, StatusCode, header};
use serde_json::{Value, json};

use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};

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
