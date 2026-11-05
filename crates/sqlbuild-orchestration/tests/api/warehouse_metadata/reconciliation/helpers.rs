//! HTTP acceptance for full scans: only a complete scan removes, and only in its own namespace.

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};

pub(crate) async fn reconcile_history(
    store: &ControlStore,
    (client, url, bearer): (&Client, &str, &str),
    (session_id, preview_session_id): (Uuid, Uuid),
) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/warehouse_reconciliation.json");
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop reconciliation"))
            .expect("fixture");
    for (kind, table, timestamp) in [
        ("source_freshness", "sqb_warehouse_freshness", "observed_at"),
        ("fingerprints", "sqb_warehouse_fingerprints", "ts"),
    ] {
        let record = fixture[kind].clone();
        let mut legacy = record.clone();
        legacy["event_id"] = Value::Null;
        legacy["run_id"] = json!("orders-legacy-build");
        legacy[timestamp] = json!("2025-12-31T20:00:00-04:00");
        let mut originals = vec![record.clone(), legacy.clone()];
        for index in 0..fixture["obsolete_count"].as_u64().expect("obsolete count") {
            let mut obsolete = record.clone();
            obsolete["event_id"] = json!(format!("reconciliation-obsolete-{index}"));
            originals.push(obsolete);
        }
        for (id, field, value) in [
            (
                "reconciliation-other-database",
                "state_database",
                "analytics",
            ),
            (
                "reconciliation-other-schema",
                "state_schema",
                "reconcile_inventory",
            ),
        ] {
            let mut control = record.clone();
            control["event_id"] = json!(id);
            control[field] = json!(value);
            originals.push(control);
        }
        for records in originals.chunks(100) {
            post(
                (client, url, bearer),
                &json!({"kind":kind,"session_id":session_id,"generation":1,"records":records}),
                StatusCode::OK,
            )
            .await;
        }
        let mut preview = record.clone();
        preview["event_id"] = json!("reconciliation-preview");
        post(
            (client, &url.replace("/development/", "/preview/"), bearer),
            &json!({"kind":kind,"session_id":preview_session_id,"generation":1,"records":[preview]}),
            StatusCode::OK,
        )
        .await;
        let before = observations(store, table).await;
        let mut page = json!({"kind":kind,"session_id":session_id,"generation":1,"records":[record.clone(),legacy],
            "scan":{"scan_id":Uuid::now_v7(),"location":{"database":null,"schema":"reconcile_orders"},"complete":false}});
        assert_eq!(
            post((client, url, bearer), &page, StatusCode::OK).await,
            json!({"applied":false})
        );
        assert_eq!(
            observations(store, table).await,
            before,
            "an incomplete scan cannot delete"
        );
        let mut foreign = page.clone();
        foreign["scan"]["location"]["database"] = json!("analytics");
        post((client, url, bearer), &foreign, StatusCode::BAD_REQUEST).await;
        page["records"] = json!([]);
        page["scan"]["complete"] = json!(true);
        assert_eq!(
            post((client, url, bearer), &page, StatusCode::OK).await,
            json!({"applied":true})
        );
        assert_eq!(
            source_ids(store, table).await,
            [
                "",
                "reconciliation-keep",
                "reconciliation-other-database",
                "reconciliation-other-schema",
                "reconciliation-preview"
            ],
            "the scanned rows and every other namespace and target survive"
        );
        assert_eq!(
            post((client, url, bearer), &page, StatusCode::OK).await,
            json!({"applied":false}),
            "a lost final response is retry-safe"
        );
        let empty = json!({"kind":kind,"session_id":session_id,"generation":1,"records":[],"scan":{
            "scan_id":Uuid::now_v7(),"location":{"database":null,"schema":"reconcile_orders"},"complete":true}});
        assert_eq!(
            post((client, url, bearer), &empty, StatusCode::OK).await,
            json!({"applied":true})
        );
        assert_eq!(
            source_ids(store, table).await,
            [
                "reconciliation-other-database",
                "reconciliation-other-schema",
                "reconciliation-preview"
            ],
            "a complete empty scan clears only its namespace"
        );
        for records in originals.chunks(100) {
            post(
                (client, url, bearer),
                &json!({"kind":kind,"session_id":session_id,"generation":1,"records":records}),
                StatusCode::OK,
            )
            .await;
        }
        assert_eq!(
            observations(store, table).await,
            before,
            "a resync restores exact identities and observations"
        );
        retained_history(
            (store, table),
            (client, url, bearer),
            (session_id, &fixture, &record, (kind, timestamp)),
        )
        .await;
    }
}

async fn retained_history(
    (store, table): (&ControlStore, &str),
    (client, url, bearer): (&Client, &str, &str),
    (session_id, fixture, record, (kind, timestamp)): (Uuid, &Value, &Value, (&str, &str)),
) {
    let count = fixture["retained_count"].as_u64().expect("retained count");
    let records: Vec<_> = (0..count)
        .map(|index| {
            let mut record = record.clone();
            record["event_id"] = json!(format!("retained-{index}"));
            record["state_database"] = json!("analytics");
            record["state_schema"] = json!("reconcile_retained");
            record[timestamp] = json!(format!(
                "2026-01-01T10:{:02}:{:02}Z",
                index / 120,
                (index / 2) % 60
            ));
            record
        })
        .collect();
    for records in records.chunks(100) {
        post(
            (client, url, bearer),
            &json!({"kind":kind,"session_id":session_id,"generation":1,"records":records}),
            StatusCode::OK,
        )
        .await;
    }
    let mut page = json!({"kind":kind,"session_id":session_id,"generation":1,"records":[],"scan":{
        "scan_id":Uuid::now_v7(),"location":{"database":"analytics","schema":"reconcile_retained"},"complete":false}});
    let pages = records.len().div_ceil(100);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        for (index, records) in records.chunks(100).enumerate() {
            page["records"] = json!(records);
            page["scan"]["complete"] = json!(index + 1 == pages);
            let reply = post((client, url, bearer), &page, StatusCode::OK).await;
            assert_eq!(
                reply["applied"], false,
                "retained observations never change"
            );
        }
    })
    .await
    .expect("a retained-heavy scan has a strict total deadline");
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read retained"),
        &format!("SELECT record_json ->> 'event_id' AS source_event_id FROM {table} WHERE state_database='analytics' AND state_schema='reconcile_retained' ORDER BY source_event_id"),
        vec![],
    ).await.expect("retained identities");
    commit(transaction).await.expect("close retained read");
    let mut expected: Vec<_> = (0..count)
        .map(|index| format!("retained-{index}"))
        .collect();
    expected.sort();
    assert_eq!(
        rows.iter()
            .map(|row| row["source_event_id"].as_str().expect("identity"))
            .collect::<Vec<_>>(),
        expected
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
        .expect("metadata request");
    let status = response.status();
    let body: Value = response.json().await.expect("metadata response");
    assert_eq!(status, expected, "{body}");
    body
}

async fn observations(store: &ControlStore, table: &str) -> Vec<crate::store::types::Row> {
    let (transaction, rows) = query(begin(store, TransactionMode::Read).await.expect("read"),
        &format!("SELECT event_id,target,record_json FROM {table} WHERE state_schema IN ('reconcile_orders','reconcile_inventory') ORDER BY event_id"), vec![]).await.expect("mirror facts");
    commit(transaction).await.expect("close read");
    rows
}

async fn source_ids(store: &ControlStore, table: &str) -> Vec<String> {
    let mut ids: Vec<String> = observations(store, table)
        .await
        .iter()
        .map(|row| {
            row["record_json"]["event_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    ids.sort();
    ids
}
