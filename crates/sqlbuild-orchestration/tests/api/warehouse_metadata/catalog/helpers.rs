//! Authoritative catalog changes, atomic facts and bounded removal through real HTTP.

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
use crate::warehouse_metadata::_helpers::catalog::rebuild_catalog_projection;
use crate::warehouse_metadata::models::CatalogBatch;

pub(crate) async fn catalog_history(
    store: &ControlStore,
    outbound: &LocationWorkClient,
    (session_id, preview_session): (Uuid, Uuid),
    (client, freshness_url, bearer): (&Client, &str, &str),
) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/warehouse_catalog.json");
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("fixture"))
        .expect("catalog fixture");
    let record = fixture["record"].clone();
    let url = freshness_url.replace("source-freshness", "catalog");
    let mut batch = json!({"session_id":session_id,"generation":1,"records":[record],
        "reconciliation":{"scan_id":Uuid::now_v7(),"page":0,"state_database":null,"state_schema":"main","day":null,"complete":true,"after":null}});
    let typed: CatalogBatch = serde_json::from_value(batch.clone()).expect("catalog batch");
    assert!(
        outbound
            .sync_catalog("development", &typed)
            .await
            .expect("native catalog client")
            .applied
    );
    assert!(
        !outbound
            .sync_catalog("development", &typed)
            .await
            .expect("lost response retry")
            .applied
    );
    let initial = evidence(store).await;
    assert_eq!(initial["facts"].as_array().expect("facts").len(), 1);
    crate::api::tests::warehouse_catalog_replay::catalog_replay_contract(store).await;
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
        post((client, &url, bearer), &invalid, status).await;
    }
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
    batch["reconciliation"]["scan_id"] = json!(Uuid::now_v7());
    assert_eq!(
        post((client, &url, bearer), &batch, StatusCode::OK).await["applied"],
        false
    );
    assert_eq!(
        evidence(store).await,
        initial,
        "unchanged scans do not append structural facts"
    );
    batch["reconciliation"]["scan_id"] = json!(Uuid::now_v7());
    batch["records"][0]["columns"][1]["data_type"] = json!("INTEGER");
    for (table, operation) in [
        ("recall_events", "INSERT"),
        ("sqb_warehouse_catalog", "UPDATE"),
        ("sqb_warehouse_scan_keys", "INSERT"),
    ] {
        let fault = TransactionFault {
            description: "catalog change rollback",
            table,
            operation,
            condition: "",
            sqlite_body: "SELECT RAISE(ABORT, 'injected catalog failure')",
            postgres_body: "RAISE EXCEPTION 'injected catalog failure'",
        };
        install_fault(store, &fault).await;
        post(
            (client, &url, bearer),
            &batch,
            StatusCode::INTERNAL_SERVER_ERROR,
        )
        .await;
        remove_fault(store, &fault).await;
        assert_eq!(
            evidence(store).await,
            initial,
            "fact, projection and receipt roll back together"
        );
    }
    let replica = second_store(store).await;
    let transaction = begin(store, TransactionMode::Write)
        .await
        .expect("held catalog writer");
    let repair_store = replica.clone();
    let mut repair = tokio::spawn(async move { rebuild_catalog_projection(&repair_store).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut repair)
            .await
            .is_err()
    );
    let publisher = outbound.clone();
    let typed: CatalogBatch = serde_json::from_value(batch.clone()).expect("changed catalog batch");
    let mut publication =
        tokio::spawn(async move { publisher.sync_catalog("development", &typed).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut publication)
            .await
            .is_err()
    );
    assert_eq!(
        evidence(&replica).await,
        initial,
        "reader retains the previous committed cut"
    );
    commit(transaction).await.expect("release catalog writer");
    tokio::time::timeout(std::time::Duration::from_secs(20), repair)
        .await
        .expect("catalog replay deadline")
        .expect("join replay")
        .expect("serialized replay");
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(20), publication)
            .await
            .expect("catalog publication deadline")
            .expect("join publication")
            .expect("serialized publication")
            .applied
    );
    close_store(replica).await;
    let changed = evidence(store).await;
    assert_eq!(changed["facts"].as_array().expect("facts").len(), 2);
    assert_eq!(
        changed["current"][0]["record_json"]["columns"][1]["data_type"],
        "INTEGER"
    );
    crate::api::tests::warehouse_catalog_replay::catalog_replay_contract(store).await;
    let mut obsolete: Vec<Value> = Vec::new();
    for index in 0..205 {
        let mut item = record.clone();
        item["name"] = json!(format!("obsolete-{index:03}"));
        obsolete.push(item);
    }
    for records in obsolete.chunks(100) {
        batch["reconciliation"]["scan_id"] = json!(Uuid::now_v7());
        batch["reconciliation"]["complete"] = json!(false);
        batch["records"] = json!(records);
        post((client, &url, bearer), &batch, StatusCode::OK).await;
    }
    let before_deletion = evidence(store).await;
    batch["reconciliation"]["scan_id"] = json!(Uuid::now_v7());
    batch["records"] = json!([]);
    post((client, &url, bearer), &batch, StatusCode::OK).await;
    assert_eq!(
        evidence(store).await,
        before_deletion,
        "partial catalog scans cannot remove objects"
    );
    batch["reconciliation"]["page"] = json!(1);
    batch["reconciliation"]["complete"] = json!(true);
    let fault = TransactionFault {
        description: "catalog removal fact rollback",
        table: "recall_events",
        operation: "INSERT",
        condition: "",
        sqlite_body: "SELECT RAISE(ABORT, 'injected catalog removal failure')",
        postgres_body: "RAISE EXCEPTION 'injected catalog removal failure'",
    };
    install_fault(store, &fault).await;
    post(
        (client, &url, bearer),
        &batch,
        StatusCode::INTERNAL_SERVER_ERROR,
    )
    .await;
    remove_fault(store, &fault).await;
    assert_eq!(
        evidence(store).await,
        before_deletion,
        "removal without its fact cannot commit"
    );
    for index in 0..3 {
        let before = evidence(store).await["current"]
            .as_array()
            .expect("current")
            .len();
        let reply = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            post((client, &url, bearer), &batch, StatusCode::OK),
        )
        .await
        .expect("bounded catalog reconciliation");
        let after = evidence(store).await["current"]
            .as_array()
            .expect("current")
            .len();
        assert!(before - after <= 100);
        batch["reconciliation"]["after"] = reply["next_reconciliation_after"].clone();
        assert_eq!(batch["reconciliation"]["after"].is_null(), index == 2);
    }
    let removed = evidence(store).await;
    assert_eq!(removed["current"], json!([]));
    assert_eq!(removed["facts"].as_array().expect("facts").len(), 413);
    post((client, &url, bearer), &batch, StatusCode::OK).await;
    assert_eq!(
        evidence(store).await,
        removed,
        "committed final-page replay appends no facts"
    );
    crate::api::tests::warehouse_catalog_replay::catalog_replay_contract(store).await;
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

async fn evidence(store: &ControlStore) -> Value {
    let (transaction, current) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        "SELECT event_id,record_json FROM sqb_warehouse_catalog ORDER BY event_id",
        vec![],
    )
    .await
    .expect("projection");
    let (transaction,facts) = query(transaction,
        "SELECT event_id,payload,invocation_sequence FROM recall_events WHERE event_type='warehouse_catalog_changed' ORDER BY storage_id",vec![]).await.expect("facts");
    commit(transaction).await.expect("close read");
    json!({"current":current,"facts":facts})
}
