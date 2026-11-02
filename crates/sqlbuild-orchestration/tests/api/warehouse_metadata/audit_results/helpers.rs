//! External audit facts retain their identities, physical scopes and atomic replay semantics.

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::platform::models::LocationWorkClient;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{ControlStore, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::warehouse_metadata::models::{AuditResultsBatch, WarehouseWatermarkRead};

pub(crate) async fn audit_history(
    store: &ControlStore,
    outbound: &LocationWorkClient,
    (session_id, preview_session): (Uuid, Uuid),
    (client, freshness_url, bearer): (&Client, &str, &str),
) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/audit_result_history.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture"))
        .expect("audit history");
    let record = fixture["record"].clone();
    let batch = json!({"session_id": session_id, "generation": 1, "records": [record, record]});
    let typed: AuditResultsBatch = serde_json::from_value(batch.clone()).expect("audit batch");
    for expected in [true, false] {
        assert_eq!(
            outbound
                .sync_audit_results("development", &typed)
                .await
                .expect("sync")
                .applied,
            expected
        );
    }
    let before = mirrored_audits(store).await;
    assert_eq!(
        before.len(),
        1,
        "duplicate deterministic result IDs deduplicate"
    );
    assert_eq!(
        before[0]["record_json"], record,
        "empty external run ID and warehouse text survive"
    );
    let url = freshness_url.replace("source-freshness", "audit-results");
    for (field, value, status) in [
        ("generation", json!(2), StatusCode::CONFLICT),
        (
            "session_id",
            json!(preview_session),
            StatusCode::BAD_REQUEST,
        ),
        (
            "records",
            json!(vec![record.clone(); 101]),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let mut invalid = batch.clone();
        invalid[field] = value;
        assert_eq!(
            client
                .post(&url)
                .bearer_auth(bearer)
                .json(&invalid)
                .send()
                .await
                .expect("fenced batch")
                .status(),
            status
        );
    }
    assert_eq!(
        client
            .post(url.replace(
                &outbound.location_id.to_string(),
                &Uuid::now_v7().to_string()
            ))
            .bearer_auth(bearer)
            .json(&batch)
            .send()
            .await
            .expect("other location")
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        client
            .post(&url)
            .json(&batch)
            .send()
            .await
            .expect("anonymous")
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let mut scoped = record.clone();
    scoped["state_schema"] = json!("reporting");
    let mut database = record.clone();
    database["state_database"] = json!("analytics");
    let scopes: AuditResultsBatch = serde_json::from_value(json!({"session_id":session_id,
        "generation":1, "records":[scoped,database]}))
    .expect("physical scopes");
    assert!(
        outbound
            .sync_audit_results("development", &scopes)
            .await
            .expect("scope")
            .applied
    );
    let preview: AuditResultsBatch = serde_json::from_value(json!({"session_id":preview_session,
        "generation":1, "records":[record]}))
    .expect("preview");
    assert!(
        outbound
            .sync_audit_results("preview", &preview)
            .await
            .expect("target scope")
            .applied
    );
    assert_eq!(mirrored_audits(store).await.len(), 4);
    let watermark: WarehouseWatermarkRead = serde_json::from_value(json!({"session_id":session_id,
        "generation":1,"kind":"audit_results","location":{"database":null,"schema":"main"}}))
    .expect("watermark");
    assert_eq!(
        json!(
            outbound
                .metadata_watermark("development", &watermark)
                .await
                .expect("watermark")
        ),
        record["occurred_at"]
    );
    let fault = TransactionFault {
        description: "audit page rolls back atomically",
        table: "sqb_warehouse_audit_results",
        operation: "INSERT",
        condition: "WHEN NEW.source_event_id='rejected-result'",
        sqlite_body: "SELECT RAISE(ABORT, 'injected audit failure')",
        postgres_body: "RAISE EXCEPTION 'injected audit failure'",
    };
    let before = mirrored_audits(store).await;
    let mut first = record.clone();
    first["result_id"] = json!("accepted-result");
    let mut second = record.clone();
    second["result_id"] = json!("rejected-result");
    let additional = json!({"session_id":session_id,"generation":1,"records":[first,second]});
    install_fault(store, &fault).await;
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(bearer)
            .json(&additional)
            .send()
            .await
            .expect("fault")
            .status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    remove_fault(store, &fault).await;
    assert_eq!(
        mirrored_audits(store).await,
        before,
        "no partial page survives"
    );
    let (transaction, _) = query(
        begin(store, TransactionMode::Write).await.expect("write"),
        "DELETE FROM sqb_warehouse_audit_results",
        vec![],
    )
    .await
    .expect("empty mirror");
    commit(transaction).await.expect("wipe");
    assert!(
        outbound
            .sync_audit_results("development", &typed)
            .await
            .expect("rebuild")
            .applied
    );
    assert!(
        outbound
            .sync_audit_results("development", &scopes)
            .await
            .expect("rebuild scopes")
            .applied
    );
    assert!(
        outbound
            .sync_audit_results("preview", &preview)
            .await
            .expect("rebuild target")
            .applied
    );
    assert_eq!(
        mirrored_audits(store).await,
        before,
        "identical facts after rebuild"
    );
    migrate_store(store).await.expect("warm migration");
    assert_eq!(mirrored_audits(store).await, before);
    assert_eq!(
        client
            .post(&url)
            .bearer_auth(bearer)
            .json(&additional)
            .send()
            .await
            .expect("retry")
            .json::<Value>()
            .await
            .expect("response"),
        json!({"applied":true})
    );
}

async fn mirrored_audits(store: &ControlStore) -> Vec<Value> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        "SELECT event_id,target,record_json FROM sqb_warehouse_audit_results ORDER BY event_id",
        vec![],
    )
    .await
    .expect("mirror");
    commit(transaction).await.expect("close read");
    rows.into_iter().map(|row| json!(row)).collect()
}
