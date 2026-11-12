//! Sensitive SQL reads disclose only the current exact node after durable audit publication.

use reqwest::{Client, Method, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::api::tests::person_sessions_helpers::request;
use crate::api::tests::person_sessions_test_types::BrowserSession;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotInput;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::logical_state::logical_state;

pub(crate) async fn node_sql_boundaries(
    store: &ControlStore,
    (client, session): (&Client, &BrowserSession),
    (location, snapshot_id): (Uuid, Uuid),
    (input, data): (&SnapshotInput, &Value),
) {
    let path = format!("/api/control/locations/{location}/targets/development/nodes/orders/sql");
    for other in [
        path.replace("development", "preview"),
        path.replace(&location.to_string(), &Uuid::now_v7().to_string()),
    ] {
        assert_eq!(
            request(client, session, (Method::GET, &other), (&Value::Null, ""))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    for (key, expected) in [
        (
            "udf:orders",
            json!({"snapshot_id":snapshot_id,"node_key":"udf:orders","raw_sql":null,"compiled_sql":data["function_sql"],"expression_ranges":{}}),
        ),
        ("missing-orders", Value::Null),
        ("orders+", Value::Null),
    ] {
        let response = request(
            client,
            session,
            (
                Method::GET,
                &path.replace("nodes/orders/", &format!("nodes/{key}/")),
            ),
            (&Value::Null, ""),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.json::<Value>().await.expect("exact node"),
            expected
        );
    }
    let mut before = logical_state(store).await;
    let read = request(client, session, (Method::GET, &path), (&Value::Null, "")).await;
    assert_eq!(read.status(), StatusCode::OK);
    assert_eq!(
        read.json::<Value>().await.expect("current SQL"),
        json!({"snapshot_id":snapshot_id,"node_key":"orders","raw_sql":data["raw_sql"],"compiled_sql":data["compiled_sql"],"expression_ranges":data["expression_ranges"]})
    );
    let mut after = logical_state(store).await;
    assert_ne!(
        before.remove("sqb_security_audit"),
        after.remove("sqb_security_audit")
    );
    assert_eq!(
        before, after,
        "SQL inspection changes only the security audit"
    );
    let transaction = begin(store, TransactionMode::Read)
        .await
        .expect("audit cut");
    let (transaction, audit) = query(transaction, "SELECT actor,action,granting_role,scope_json,before_json,after_json FROM sqb_security_audit WHERE action='sql.read' ORDER BY storage_id DESC LIMIT 1", vec![]).await.expect("audit");
    commit(transaction).await.expect("close audit cut");
    assert_eq!(audit[0]["granting_role"], "developer");
    assert_eq!(audit[0]["scope_json"]["action"], "view.sql");
    assert_eq!(audit[0]["scope_json"]["nodes"], json!(["orders"]));
    assert_eq!(
        audit[0]["after_json"],
        json!({"snapshot_id":snapshot_id,"found":true})
    );
    assert!(
        !json!(audit)
            .to_string()
            .contains(data["raw_sql"].as_str().expect("raw SQL"))
    );
    let fault = TransactionFault {
        description: "SQL read requires durable audit",
        table: "sqb_security_audit",
        operation: "INSERT",
        condition: "WHEN NEW.action='sql.read'",
        sqlite_body: "SELECT RAISE(ABORT, 'synthetic SQL audit failure')",
        postgres_body: "RAISE EXCEPTION 'synthetic SQL audit failure'",
    };
    install_fault(store, &fault).await;
    let before = logical_state(store).await;
    let response = request(client, session, (Method::GET, &path), (&Value::Null, "")).await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response.json::<Value>().await.expect("sanitized refusal"),
        json!({"error":"control-store operation failed"})
    );
    assert_eq!(
        logical_state(store).await,
        before,
        "audit failure discloses no SQL and changes no rows"
    );
    remove_fault(store, &fault).await;
    let mut updated = input.clone();
    updated.generation += 1;
    updated
        .nodes
        .get_mut("orders")
        .expect("orders")
        .compiled_sql = Some(data["updated_sql"].as_str().expect("updated SQL").into());
    let current = publish_snapshot(store, &updated)
        .await
        .expect("new publication");
    let response = request(client, session, (Method::GET, &path), (&Value::Null, "")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>().await.expect("latest SQL"),
        json!({"snapshot_id":current.snapshot_id,"node_key":"orders","raw_sql":data["raw_sql"],"compiled_sql":data["updated_sql"],"expression_ranges":data["expression_ranges"]})
    );
}
