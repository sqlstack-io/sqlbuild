//! Service accounts made by the control-store owner, and keyed requests from any address.

use reqwest::header::COOKIE;
use reqwest::{Client, RequestBuilder, StatusCode, header};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::authorization::main::service_accounts::create_service_account::create_service_account;
use crate::credentials::models::{
    AuditContext, BuiltinRole, IssuedServiceKey, NewServiceAccount, ServiceAccountAuthority,
};
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::types::Row;

/// A Host that names neither the browser origin nor a loopback name, as a caller on the network sends.
pub(crate) const NETWORK_HOST: &str = "10.20.30.40:8000";

pub(crate) fn owner() -> AuditContext {
    AuditContext {
        actor: "local-control-store-owner".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    }
}

/// A service account `name` with `role`, made by the owner, and its key.
pub(crate) async fn service_account(
    store: &ControlStore,
    name: &str,
    role: BuiltinRole,
) -> IssuedServiceKey {
    let request = NewServiceAccount {
        name: name.into(),
        description: None,
        role,
    };
    create_service_account(
        store,
        (&owner(), ServiceAccountAuthority::Owner),
        (&request, None),
    )
    .await
    .expect("service account")
}

/// A client with no browser headers, as a deploy script is.
pub(crate) fn script() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("script transport")
}

/// The status of reading the bootstrap with `key`, and whom it names.
pub(crate) async fn keyed_bootstrap(client: &Client, url: &str, key: &str) -> (StatusCode, String) {
    let response = client
        .get(format!("{url}/api/control/bootstrap"))
        .header(header::HOST, NETWORK_HOST)
        .bearer_auth(key)
        .send()
        .await
        .expect("keyed bootstrap");
    let status = response.status();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    (
        status,
        body["access"]["name"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
    )
}

/// Every row of `table`.
pub(crate) async fn rows(store: &ControlStore, table: &str) -> Vec<Row> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        &format!("SELECT * FROM {table}"),
        Vec::new(),
    )
    .await
    .expect("rows");
    commit(transaction).await.expect("finish read");
    rows
}

/// Ends every rotation overlap of `principal_id` now, as if its time had passed.
pub(crate) async fn end_overlaps(store: &ControlStore, principal_id: Uuid) {
    let (transaction, _) = query(
        begin(store, TransactionMode::Write).await.expect("write"),
        "UPDATE sqb_service_account_keys SET expires_at=$1 WHERE principal_id=$2 AND expires_at IS NOT NULL",
        vec![
            Parameter::Time(chrono::Utc::now() - chrono::Duration::seconds(1)),
            Parameter::Id(principal_id),
        ],
    )
    .await
    .expect("end overlaps");
    commit(transaction).await.expect("commit");
}

/// A signed-in person's write: their session cookie and its CSRF token.
pub(crate) async fn person_post(
    client: &Client,
    (url, path): (&str, &str),
    person: &(String, String),
    body: &Value,
) -> (StatusCode, Value) {
    let response = client
        .post(format!("{url}{path}"))
        .header(COOKIE, &person.0)
        .header("x-sqb-csrf", &person.1)
        .json(body)
        .send()
        .await
        .expect("write");
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// A signed-in person's read.
pub(crate) async fn person_get(
    client: &Client,
    (url, path): (&str, &str),
    person: &(String, String),
) -> (StatusCode, Value) {
    let response = client
        .get(format!("{url}{path}"))
        .header(COOKIE, &person.0)
        .send()
        .await
        .expect("read");
    let status = response.status();
    (status, response.json().await.unwrap_or(Value::Null))
}

/// Whether the audit page has an entry by service account `name` with `action`.
pub(crate) fn audited(audit: &Value, (name, action): (&str, &str)) -> bool {
    audit.as_array().into_iter().flatten().any(|event| {
        event["actor_name"] == json!(name)
            && event["actor_kind"] == json!("service")
            && event["action"] == json!(action)
    })
}

pub(crate) fn with_key(request: RequestBuilder, key: &str) -> RequestBuilder {
    request.bearer_auth(key)
}

pub(crate) fn without_key(request: RequestBuilder, _key: &str) -> RequestBuilder {
    request
}
