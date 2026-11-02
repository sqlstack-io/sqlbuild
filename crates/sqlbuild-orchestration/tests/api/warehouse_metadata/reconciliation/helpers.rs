//! HTTP acceptance for bounded reconciliation, retry receipts and partial-scan safety.

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;

pub(crate) async fn reconcile_history(
    store: &ControlStore,
    (client, freshness_url, bearer): (&Client, &str, &str),
    (session_id, preview_session_id): (Uuid, Uuid),
) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/duckshop/edge_cases/orchestration/warehouse_reconciliation.json");
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("DuckShop reconciliation"))
            .expect("fixture");
    for (kind, endpoint, table, timestamp) in [
        (
            "source_freshness",
            "source-freshness",
            "sqb_warehouse_freshness",
            "observed_at",
        ),
        (
            "fingerprints",
            "fingerprints",
            "sqb_warehouse_fingerprints",
            "ts",
        ),
    ] {
        let url = freshness_url.replace("source-freshness", endpoint);
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
            (
                "reconciliation-before-day",
                timestamp,
                "2025-12-31T23:59:59.999999Z",
            ),
            (
                "reconciliation-after-day",
                timestamp,
                "2026-01-02T00:00:00Z",
            ),
        ] {
            let mut control = record.clone();
            control["event_id"] = json!(id);
            control[field] = json!(value);
            originals.push(control);
        }
        for records in originals.chunks(100) {
            post(
                (client, &url, bearer),
                &json!({"session_id":session_id,"generation":1,"records":records}),
                StatusCode::OK,
            )
            .await;
        }
        let mut preview = record.clone();
        preview["event_id"] = json!("reconciliation-preview");
        post(
            (client, &url.replace("/development/", "/preview/"), bearer),
            &json!({"session_id":preview_session_id,"generation":1,"records":[preview]}),
            StatusCode::OK,
        )
        .await;
        let before = observations(store, table).await;
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let mut page = json!({"session_id":session_id,"generation":1,"records":[record.clone(),legacy],
            "reconciliation":{"scan_id":Uuid::now_v7(),"page":0,"state_database":null,"state_schema":"reconcile_orders",
                "day":fixture["day"],"complete":false,"after":null}});
        assert_eq!(
            post((client, &url, bearer), &page, StatusCode::OK).await,
            json!({"applied":false})
        );
        assert_eq!(
            observations(store, table).await,
            before,
            "partial scan cannot delete"
        );
        for (field, value) in [
            ("page", json!(2)),
            ("state_database", json!("analytics")),
            ("day", json!("2026-01-02")),
        ] {
            let mut invalid = page.clone();
            invalid["records"] = json!([]);
            invalid["reconciliation"][field] = value;
            post((client, &url, bearer), &invalid, StatusCode::BAD_REQUEST).await;
        }
        let mut changed = page.clone();
        changed["records"] = json!([record.clone()]);
        post((client, &url, bearer), &changed, StatusCode::BAD_REQUEST).await;
        let mut fenced = page.clone();
        fenced["generation"] = json!(2);
        post((client, &url, bearer), &fenced, StatusCode::CONFLICT).await;
        fenced = page.clone();
        fenced["session_id"] = json!(Uuid::now_v7());
        post((client, &url, bearer), &fenced, StatusCode::BAD_REQUEST).await;
        let mut late = record.clone();
        late["event_id"] = json!("reconciliation-late");
        post(
            (client, &url, bearer),
            &json!({"session_id":session_id,"generation":1,"records":[late]}),
            StatusCode::OK,
        )
        .await;
        let before_final = observations(store, table).await;
        let mut final_new = record.clone();
        final_new["event_id"] = json!("reconciliation-final-new");
        page["records"] = json!([final_new]);
        page["reconciliation"]["page"] = json!(1);
        page["reconciliation"]["complete"] = json!(true);
        for (fault_table, operation) in [
            ("sqb_warehouse_scan_pages", "INSERT"),
            ("sqb_warehouse_scan_keys", "INSERT"),
            (table, "DELETE"),
        ] {
            let fault = TransactionFault {
                description: "reconciliation rollback",
                table: fault_table,
                operation,
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT, 'injected reconciliation failure')",
                postgres_body: "RAISE EXCEPTION 'injected reconciliation failure'",
            };
            install_fault(store, &fault).await;
            post(
                (client, &url, bearer),
                &page,
                StatusCode::INTERNAL_SERVER_ERROR,
            )
            .await;
            remove_fault(store, &fault).await;
            assert_eq!(
                observations(store, table).await,
                before_final,
                "receipt, import, keys and deletion roll back together"
            );
        }
        let mut remaining = before_final.len() + 1;
        for index in 0..3 {
            let reply = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                post((client, &url, bearer), &page, StatusCode::OK),
            )
            .await
            .expect("bounded reconciliation page");
            let now = observations(store, table).await.len();
            assert!(
                remaining - now <= 100,
                "at most one bounded mirror page is deleted"
            );
            remaining = now;
            page["reconciliation"]["after"] = reply["next_reconciliation_after"].clone();
            assert_eq!(page["reconciliation"]["after"].is_null(), index == 2);
        }
        assert!(
            page["reconciliation"]["after"].is_null(),
            "all bounded pages completed"
        );
        let after = observations(store, table).await;
        let mut retained: Vec<_> = after
            .iter()
            .filter_map(|row| row["record_json"]["event_id"].as_str())
            .collect();
        retained.sort();
        let mut expected: Vec<_> = fixture["retained_ids"]
            .as_array()
            .expect("expected identities")
            .iter()
            .map(|id| id.as_str().expect("identity"))
            .collect();
        expected.sort();
        assert_eq!(
            retained, expected,
            "other namespaces/days/targets and concurrent delivery survive"
        );
        assert_eq!(
            after.len(),
            expected.len() + 1,
            "the fixed legacy observation also survives"
        );
        page["reconciliation"]["after"] = Value::Null;
        assert_eq!(
            post((client, &url, bearer), &page, StatusCode::OK).await,
            json!({"applied":false}),
            "lost final response is retry-safe"
        );
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let empty = json!({"session_id":session_id,"generation":1,"records":[],"reconciliation":{
            "scan_id":Uuid::now_v7(),"page":0,"state_database":null,"state_schema":"reconcile_orders","day":fixture["day"],"complete":true,"after":null}});
        assert_eq!(
            post((client, &url, bearer), &empty, StatusCode::OK).await,
            json!({"applied":true})
        );
        assert_eq!(
            observations(store, table).await.len(),
            5,
            "complete empty day deletes only its mirror scope"
        );
        for records in originals.chunks(100) {
            post(
                (client, &url, bearer),
                &json!({"session_id":session_id,"generation":1,"records":records}),
                StatusCode::OK,
            )
            .await;
        }
        assert_eq!(
            observations(store, table).await,
            before,
            "empty-copy resync preserves exact identities and observations"
        );
        retained_history(
            (store, table),
            (client, &url, bearer),
            (session_id, &fixture, &record, timestamp),
        )
        .await;
    }
}

async fn retained_history(
    (store, table): (&ControlStore, &str),
    (client, url, bearer): (&Client, &str, &str),
    (session_id, fixture, record, timestamp): (Uuid, &Value, &Value, &str),
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
            &json!({"session_id":session_id,"generation":1,"records":records}),
            StatusCode::OK,
        )
        .await;
    }
    let mut page = json!({"session_id":session_id,"generation":1,"records":[],"reconciliation":{
        "scan_id":Uuid::now_v7(),"page":0,"state_database":"analytics","state_schema":"reconcile_retained",
        "day":fixture["day"],"complete":false,"after":null}});
    let pages = records.len().div_ceil(100);
    for (index, records) in records.chunks(100).enumerate() {
        page["records"] = json!(records);
        page["reconciliation"]["page"] = json!(index);
        page["reconciliation"]["complete"] = json!(index + 1 == pages);
        let reply = post((client, url, bearer), &page, StatusCode::OK).await;
        assert_eq!(
            reply["applied"], false,
            "retained observations never change"
        );
        page["reconciliation"]["after"] = reply["next_reconciliation_after"].clone();
    }
    assert!(page["reconciliation"]["after"].is_object());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        for index in 1..=pages {
            let previous = page["reconciliation"]["after"].clone();
            let reply = post((client, url, bearer), &page, StatusCode::OK).await;
            assert_eq!(reply["applied"], false);
            page["reconciliation"]["after"] = reply["next_reconciliation_after"].clone();
            assert_ne!(page["reconciliation"]["after"], previous, "cursor advances");
            assert_eq!(page["reconciliation"]["after"].is_null(), index == pages);
        }
    })
    .await
    .expect("retained-heavy continuation has a strict total deadline");
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read retained"),
        &format!("SELECT source_event_id FROM {table} WHERE state_database='analytics' AND state_schema='reconcile_retained' ORDER BY source_event_id"),
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
        &format!("SELECT event_id,target,source_event_id,record_json FROM {table} WHERE state_schema IN ('reconcile_orders','reconcile_inventory') ORDER BY event_id"), vec![]).await.expect("mirror facts");
    commit(transaction).await.expect("close read");
    rows
}
