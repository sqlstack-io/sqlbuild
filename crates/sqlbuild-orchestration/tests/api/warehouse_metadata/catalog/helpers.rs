//! Catalog scans replace their namespace atomically through real HTTP.

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::platform::models::LocationWorkClient;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::close_store;
use crate::warehouse_metadata::models::MetadataBatch;

pub(crate) async fn catalog_history(
    store: &ControlStore,
    outbound: &LocationWorkClient,
    (session_id, preview_session): (Uuid, Uuid),
    (client, url, bearer): (&Client, &str, &str),
) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/warehouse_catalog.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture"))
        .expect("catalog fixture");
    let record = fixture["record"].clone();
    let mut batch = json!({"kind":"catalog","session_id":session_id,"generation":1,"records":[record],
        "scan":{"scan_id":Uuid::now_v7(),"location":{"database":null,"schema":"main"},"complete":true}});
    let typed: MetadataBatch = serde_json::from_value(batch.clone()).expect("catalog batch");
    assert!(
        outbound
            .sync_metadata("development", &typed)
            .await
            .expect("native catalog client")
            .applied
    );
    assert!(
        !outbound
            .sync_metadata("development", &typed)
            .await
            .expect("lost response retry")
            .applied
    );
    let initial = current(store).await;
    assert_eq!(initial.as_array().expect("current").len(), 1);
    let mut unscanned = batch.clone();
    unscanned["scan"] = Value::Null;
    for (field, value, status) in [
        ("generation", json!(2), StatusCode::CONFLICT),
        (
            "session_id",
            json!(preview_session),
            StatusCode::BAD_REQUEST,
        ),
        ("records", json!([record, record]), StatusCode::BAD_REQUEST),
    ] {
        let mut invalid = batch.clone();
        invalid[field] = value;
        post((client, url, bearer), &invalid, status).await;
    }
    post((client, url, bearer), &unscanned, StatusCode::BAD_REQUEST).await;
    post(
        (
            client,
            &url.replace(
                &outbound.location_id.to_string(),
                &Uuid::now_v7().to_string(),
            ),
            bearer,
        ),
        &batch,
        StatusCode::FORBIDDEN,
    )
    .await;
    batch["scan"]["scan_id"] = json!(Uuid::now_v7());
    assert_eq!(
        post((client, url, bearer), &batch, StatusCode::OK).await["applied"],
        false
    );
    assert_eq!(
        current(store).await,
        initial,
        "an unchanged scan changes nothing"
    );
    batch["scan"]["scan_id"] = json!(Uuid::now_v7());
    batch["records"][0]["columns"][1]["data_type"] = json!("INTEGER");
    let fault = TransactionFault {
        description: "catalog change rollback",
        table: "sqb_warehouse_catalog",
        operation: "UPDATE",
        condition: "",
        sqlite_body: "SELECT RAISE(ABORT, 'injected catalog failure')",
        postgres_body: "RAISE EXCEPTION 'injected catalog failure'",
    };
    install_fault(store, &fault).await;
    post(
        (client, url, bearer),
        &batch,
        StatusCode::INTERNAL_SERVER_ERROR,
    )
    .await;
    remove_fault(store, &fault).await;
    assert_eq!(current(store).await, initial, "a failed page rolls back");
    let replica = second_store(store).await;
    let transaction = begin(store, TransactionMode::Write)
        .await
        .expect("held catalog writer");
    let publisher = outbound.clone();
    let typed: MetadataBatch =
        serde_json::from_value(batch.clone()).expect("changed catalog batch");
    let mut publication =
        tokio::spawn(async move { publisher.sync_metadata("development", &typed).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut publication)
            .await
            .is_err()
    );
    assert_eq!(
        current(&replica).await,
        initial,
        "reader retains the previous committed cut"
    );
    commit(transaction).await.expect("release catalog writer");
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(20), publication)
            .await
            .expect("catalog publication deadline")
            .expect("join publication")
            .expect("serialized publication")
            .applied
    );
    close_store(replica).await;
    assert_eq!(
        current(store).await[0]["record_json"]["columns"][1]["data_type"],
        "INTEGER"
    );
    let kept = batch["records"][0].clone();
    let mut obsolete: Vec<Value> = Vec::new();
    for index in 0..205 {
        let mut item = record.clone();
        item["name"] = json!(format!("obsolete-{index:03}"));
        obsolete.push(item);
    }
    for records in obsolete.chunks(100) {
        batch["scan"]["scan_id"] = json!(Uuid::now_v7());
        batch["scan"]["complete"] = json!(false);
        batch["records"] = json!(records);
        post((client, url, bearer), &batch, StatusCode::OK).await;
    }
    let before_deletion = current(store).await;
    assert_eq!(before_deletion.as_array().expect("current").len(), 206);
    batch["scan"]["scan_id"] = json!(Uuid::now_v7());
    batch["records"] = json!([kept]);
    post((client, url, bearer), &batch, StatusCode::OK).await;
    assert_eq!(
        current(store).await,
        before_deletion,
        "an incomplete scan removes nothing"
    );
    batch["scan"]["complete"] = json!(true);
    batch["records"] = json!([]);
    assert_eq!(
        post((client, url, bearer), &batch, StatusCode::OK).await["applied"],
        true
    );
    let replaced = current(store).await;
    assert_eq!(
        replaced
            .as_array()
            .expect("current")
            .iter()
            .map(|row| row["record_json"]["name"].clone())
            .collect::<Vec<_>>(),
        vec![record["name"].clone()],
        "a complete scan keeps exactly the relations it delivered"
    );
    assert_eq!(
        post((client, url, bearer), &batch, StatusCode::OK).await["applied"],
        false
    );
    assert_eq!(
        current(store).await,
        replaced,
        "replaying the final page changes nothing"
    );
}

async fn post(
    (client, url, bearer): (&Client, &str, &str),
    body: &Value,
    expected: StatusCode,
) -> Value {
    let response = client
        .post(url)
        .bearer_auth(bearer)
        .json(body)
        .send()
        .await
        .expect("catalog request");
    let status = response.status();
    let body: Value = response.json().await.expect("catalog reply");
    assert_eq!(status, expected, "{body}");
    body
}

async fn current(store: &ControlStore) -> Value {
    let (transaction, current) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        "SELECT event_id,record_json FROM sqb_warehouse_catalog ORDER BY event_id",
        vec![],
    )
    .await
    .expect("projection");
    commit(transaction).await.expect("close read");
    json!(current)
}
