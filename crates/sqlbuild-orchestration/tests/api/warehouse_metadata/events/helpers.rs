//! Warehouse events retain their identities, physical scopes and atomic replay semantics.

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
use crate::warehouse_metadata::models::{
    WarehouseHistoryKind, WarehouseSyncResponse, WarehouseWatermarkRead,
};

pub(crate) async fn event_history(
    store: &ControlStore,
    outbound: &LocationWorkClient,
    (session_id, preview_session): (Uuid, Uuid),
    (client, metadata_url, bearer, (kind, fixture_name, source_key, key_fields)): (
        &Client,
        &str,
        &str,
        (WarehouseHistoryKind, &str, &str, &[&str]),
    ),
) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../benchmarks/duckshop/edge_cases/orchestration/{fixture_name}.json"
    ));
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture"))
        .expect("event history");
    let timestamp = fixture["timestamp_key"].as_str().unwrap_or("occurred_at");
    let record = fixture["record"].clone();
    let batch = json!({"kind": kind, "session_id": session_id, "generation": 1, "records": [record, record]});
    let url = metadata_url;
    for expected in [true, false] {
        assert_eq!(
            sync_events((client, url, bearer), &batch).await.applied,
            expected
        );
    }
    let before = mirrored_events(store, kind).await;
    assert_eq!(
        before.len(),
        1,
        "duplicate deterministic event identities deduplicate"
    );
    assert_eq!(
        before[0]["record_json"], record,
        "external run IDs, text and timestamps survive"
    );
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
                .post(url)
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
            .post(url)
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
    let scopes = json!({"kind":kind,"session_id":session_id,
        "generation":1, "records":[scoped,database]});
    assert!(sync_events((client, url, bearer), &scopes).await.applied);
    let preview = json!({"kind":kind,"session_id":preview_session,
        "generation":1, "records":[record]});
    assert!(
        sync_events(
            (client, &url.replace("/development/", "/preview/"), bearer),
            &preview
        )
        .await
        .applied
    );
    assert_eq!(mirrored_events(store, kind).await.len(), 4);
    let watermark: WarehouseWatermarkRead = serde_json::from_value(json!({"session_id":session_id,
        "generation":1,"kind":kind,"location":{"database":null,"schema":"main"}}))
    .expect("watermark");
    assert_eq!(
        json!(
            outbound
                .metadata_watermark("development", &watermark)
                .await
                .expect("watermark")
        ),
        record[timestamp]
    );
    let fault = TransactionFault {
        description: "event page rolls back atomically",
        table: kind.mirror_table(),
        operation: "INSERT",
        condition: "WHEN 'rejected-result' IN (NEW.record_json ->> 'event_id', NEW.record_json ->> 'result_id', \
            NEW.record_json ->> 'node_name', NEW.record_json ->> 'execution_id', \
            NEW.record_json ->> 'observation_id')",
        sqlite_body: "SELECT RAISE(ABORT, 'injected audit failure')",
        postgres_body: "RAISE EXCEPTION 'injected audit failure'",
    };
    let before = mirrored_events(store, kind).await;
    let mut first = record.clone();
    first[source_key] = json!("accepted-result");
    let mut second = record.clone();
    second[source_key] = json!("rejected-result");
    let additional =
        json!({"kind":kind,"session_id":session_id,"generation":1,"records":[first,second]});
    install_fault(store, &fault).await;
    assert_eq!(
        client
            .post(url)
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
        mirrored_events(store, kind).await,
        before,
        "no partial page survives"
    );
    let (transaction, _) = query(
        begin(store, TransactionMode::Write).await.expect("write"),
        &format!("DELETE FROM {}", kind.mirror_table()),
        vec![],
    )
    .await
    .expect("empty mirror");
    commit(transaction).await.expect("wipe");
    assert!(sync_events((client, url, bearer), &batch).await.applied);
    assert!(sync_events((client, url, bearer), &scopes).await.applied);
    assert!(
        sync_events(
            (client, &url.replace("/development/", "/preview/"), bearer),
            &preview
        )
        .await
        .applied
    );
    assert_eq!(
        mirrored_events(store, kind).await,
        before,
        "identical facts after rebuild"
    );
    migrate_store(store).await.expect("warm migration");
    assert_eq!(mirrored_events(store, kind).await, before);
    assert_eq!(
        client
            .post(url)
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
    key_page(
        store,
        (client, url, bearer),
        session_id,
        (kind, &record, source_key, key_fields),
    )
    .await;
}

/// A reconcile scan's key page keeps the mirrored row it names; the complete scan removes the rest.
async fn key_page(
    store: &ControlStore,
    (client, url, bearer): (&Client, &str, &str),
    session_id: Uuid,
    (kind, record, source_key, key_fields): (WarehouseHistoryKind, &Value, &str, &[&str]),
) {
    let projection: Value = key_fields
        .iter()
        .map(|field| ((*field).to_owned(), record[*field].clone()))
        .collect::<serde_json::Map<_, _>>()
        .into();
    let mut unknown = projection.clone();
    unknown[source_key] = json!("key-page-missing");
    let location = json!({"database": record["state_database"], "schema": record["state_schema"]});
    let scan = json!({"scan_id": Uuid::now_v7(), "location": location, "complete": false});
    let page = json!({"kind": kind, "session_id": session_id, "generation": 1,
        "keys": [projection, unknown], "scan": scan});
    let keys_url = format!("{url}/keys");
    assert_eq!(
        client
            .post(&keys_url)
            .bearer_auth(bearer)
            .json(&page)
            .send()
            .await
            .expect("key page")
            .json::<Value>()
            .await
            .expect("key page response"),
        json!({"missing": [1]}),
        "{kind:?}: the key projection matches the mirrored full record"
    );
    let mut complete = page.clone();
    complete["scan"]["complete"] = json!(true);
    assert_eq!(
        client
            .post(&keys_url)
            .bearer_auth(bearer)
            .json(&complete)
            .send()
            .await
            .expect("complete key page")
            .status(),
        StatusCode::BAD_REQUEST,
        "{kind:?}: only a records page completes a scan"
    );
    let mut finish = json!({"kind": kind, "session_id": session_id, "generation": 1, "records": [], "scan": scan});
    finish["scan"]["complete"] = json!(true);
    let _ = sync_events((client, url, bearer), &finish).await;
    let scanned: Vec<Value> = mirrored_events(store, kind)
        .await
        .into_iter()
        .filter(|row| {
            row["target"] == "development"
                && row["record_json"]["state_schema"] == record["state_schema"]
                && row["record_json"]["state_database"] == record["state_database"]
        })
        .map(|row| row["record_json"].clone())
        .collect();
    assert_eq!(
        scanned,
        [record.clone()],
        "{kind:?}: the complete scan keeps only the row its key page matched"
    );
}

async fn mirrored_events(store: &ControlStore, kind: WarehouseHistoryKind) -> Vec<Value> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        &format!(
            "SELECT event_id,target,record_json FROM {} ORDER BY event_id",
            kind.mirror_table()
        ),
        vec![],
    )
    .await
    .expect("mirror");
    commit(transaction).await.expect("close read");
    rows.into_iter().map(|row| json!(row)).collect()
}

async fn sync_events(
    (client, url, bearer): (&Client, &str, &str),
    batch: &Value,
) -> WarehouseSyncResponse {
    client
        .post(url)
        .bearer_auth(bearer)
        .json(batch)
        .send()
        .await
        .expect("event sync")
        .error_for_status()
        .expect("accepted page")
        .json()
        .await
        .expect("sync response")
}
