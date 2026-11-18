//! Builds, check results and store page counts for state history retention tests.

use chrono::{Duration, TimeZone, Utc};
use serde_json::{Value, json};

use crate::api::tests::warehouse_history_helpers::fixture;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// `builds` builds of `node`, an hour apart.
pub(crate) fn fingerprints(node: &str, builds: i64) -> Vec<Value> {
    let base = fixture("fingerprint_lookup.json")["record"].clone();
    (0..builds)
        .map(|build| {
            let mut record = base.clone();
            record["event_id"] = json!(format!("{node}-build-{build:03}"));
            record["state_schema"] = json!("main");
            record["node_name"] = json!(node);
            record["target_name"] = json!(node);
            record["run_id"] = json!(format!("{node}-run-{build:03}"));
            record["definition"] = json!(format!("SELECT {build} AS {node}_version"));
            record["ts"] = json!(at(build));
            record
        })
        .collect()
}

/// `results` results of check `binding`, an hour apart.
pub(crate) fn audit_results(binding: &str, results: i64) -> Vec<Value> {
    let base = fixture("audit_result_history.json")["record"].clone();
    (0..results)
        .map(|result| {
            let mut record = base.clone();
            record["result_id"] = json!(format!("{binding}-{result:03}"));
            record["binding_key"] = json!(binding);
            record["occurred_at"] = json!(at(result));
            record
        })
        .collect()
}

/// The time of the `index`th version.
pub(crate) fn at(index: i64) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0)
        .single()
        .expect("time")
        + Duration::hours(index)
}

/// The newest of `count` versions, as the store renders it.
pub(crate) fn newest(count: i64) -> chrono::DateTime<Utc> {
    at(count - 1)
}

/// Rows kept per name and the newest time, by a `name, kept, newest` query.
pub(crate) async fn counts(
    store: &ControlStore,
    sql: &str,
) -> Vec<(String, usize, chrono::DateTime<Utc>)> {
    let transaction = begin(store, TransactionMode::Read).await.expect("read");
    let (transaction, rows) = query(transaction, sql, Vec::<Parameter>::new())
        .await
        .expect("count");
    commit(transaction).await.expect("commit");
    rows.into_iter()
        .map(|row| {
            (
                row["name"].as_str().expect("name").to_owned(),
                usize::try_from(row["kept"].as_i64().expect("count")).expect("count"),
                serde_json::from_value(row["newest"].clone()).expect("newest"),
            )
        })
        .collect()
}

/// Page count, free pages and auto-vacuum mode of a SQLite store.
pub(crate) async fn pages(store: &ControlStore) -> (i64, i64, i64) {
    let ControlStore::Sqlite(path) = store else {
        panic!("SQLite store");
    };
    let connection = rusqlite::Connection::open(&**path).expect("open");
    let pragma = |name: &str| -> i64 {
        connection
            .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
            .expect("pragma")
    };
    (
        pragma("page_count"),
        pragma("freelist_count"),
        pragma("auto_vacuum"),
    )
}

/// Builds of `node` whose metadata is about 128 KiB each, so pruning frees most of a store.
pub(crate) fn large_fingerprints(node: &str, builds: i64) -> Vec<Value> {
    let large = "x".repeat(128 * 1024);
    fingerprints(node, builds)
        .into_iter()
        .map(|mut record| {
            record["metadata_json"] =
                json!(format!("{{\"note\":\"{large}{}\"}}", record["run_id"]));
            record
        })
        .collect()
}

/// Rebuild a SQLite store in `mode` (`NONE` or `INCREMENTAL`) auto-vacuum.
pub(crate) fn set_auto_vacuum(store: &ControlStore, mode: &str) {
    let ControlStore::Sqlite(path) = store else {
        panic!("SQLite store");
    };
    rusqlite::Connection::open(&**path)
        .expect("open")
        .execute_batch(&format!("PRAGMA auto_vacuum={mode}; VACUUM"))
        .expect("auto-vacuum mode");
}
