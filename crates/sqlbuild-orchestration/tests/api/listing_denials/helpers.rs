//! Masked denial responses and the audit detail behind them.

use reqwest::header;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};

/// Status, the headers that could tell cases apart, and the body with its decision ID masked.
pub(crate) async fn masked(response: reqwest::Response) -> (Value, Uuid) {
    let status = response.status().as_u16();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .filter(|(name, _)| *name != header::DATE)
        .map(|(name, value)| (name.to_string(), value.to_str().unwrap_or("").to_owned()))
        .collect();
    let text = response.text().await.expect("denial body");
    let body: Value = serde_json::from_str(&text).expect("JSON denial");
    let decision: Uuid = serde_json::from_value(body["decision_id"].clone()).expect("decision ID");
    (
        json!({"status": status, "headers": headers,
            "body": text.replace(&decision.to_string(), "<decision>")}),
        decision,
    )
}

pub(crate) async fn audited_scope(store: &ControlStore, decision: Uuid) -> Value {
    let transaction = begin(store, TransactionMode::Read).await.expect("audit");
    let (transaction, rows) = query(
        transaction,
        "SELECT scope_json, after_json FROM sqb_security_audit WHERE action='authorization.denied'",
        vec![],
    )
    .await
    .expect("denials");
    commit(transaction).await.expect("close audit");
    rows.into_iter()
        .find(|row| row["after_json"]["decision_id"] == json!(decision))
        .map(|row| json!([row["scope_json"]["nodes"], row["after_json"]["reason"]]))
        .unwrap_or(Value::Null)
}
