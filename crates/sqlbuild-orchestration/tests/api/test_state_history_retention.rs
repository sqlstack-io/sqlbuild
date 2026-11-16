//! The control store keeps a bounded number of builds per node and results per check.

use chrono::{Duration, TimeZone, Utc};
use serde_json::{Value, json};

use crate::api::tests::test_types::StateHistoryRetentionTestCase;
use crate::api::tests::warehouse_history_helpers::{batch, fixture, seed_target};
use crate::platform::_helpers::retention::reclaim_sqlite_space;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest::prune_state_history::prune_state_history;
use crate::warehouse_metadata::main::ingest::{
    ingest_audit_results::ingest_audit_results, ingest_fingerprints::ingest_fingerprints,
};
use crate::warehouse_metadata::models::StateHistoryRetention;

#[test]
fn given_many_builds_and_check_results_when_pruning_then_each_node_and_check_keeps_its_newest() {
    let test_cases = [
        StateHistoryRetentionTestCase {
            description: "SQLite keeps five by default",
            backend: Backend::Sqlite,
            configured: None,
            expected_versions: 5,
        },
        StateHistoryRetentionTestCase {
            description: "PostgreSQL keeps twenty by default",
            backend: Backend::Postgres,
            configured: None,
            expected_versions: 20,
        },
        StateHistoryRetentionTestCase {
            description: "a configured limit applies on SQLite",
            backend: Backend::Sqlite,
            configured: Some(2),
            expected_versions: 2,
        },
        StateHistoryRetentionTestCase {
            description: "a configured limit applies on PostgreSQL",
            backend: Backend::Postgres,
            configured: Some(1),
            expected_versions: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture_store = open_test_store(test_case.backend).await.expect("store");
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            let scope = (location, "development");
            let retention = test_case
                .configured
                .map_or_else(StateHistoryRetention::default, |versions| {
                    StateHistoryRetention::versions(versions).expect("valid limit")
                });
            assert_eq!(
                retention.kept_versions(store),
                test_case.expected_versions,
                "{}",
                test_case.description
            );
            let builds = 25;
            for node in ["orders", "customers"] {
                for records in fingerprints(node, builds).chunks(50) {
                    ingest_fingerprints(
                        store,
                        scope,
                        &batch(session_id, "fingerprints", records.to_vec()),
                    )
                    .await
                    .expect("fingerprint ingestion");
                }
            }
            ingest_fingerprints(
                store,
                scope,
                &batch(session_id, "fingerprints", fingerprints("inventory", 1)),
            )
            .await
            .expect("single build");
            for binding in ["orders.valid_orders", "customers.unique_email"] {
                ingest_audit_results(
                    store,
                    scope,
                    &batch(session_id, "audit_results", audit_results(binding, builds)),
                )
                .await
                .expect("check result ingestion");
            }
            loop {
                let (_, more) = prune_state_history(store, retention).await.expect("prune");
                if !more {
                    break;
                }
            }
            let kept = usize::from(test_case.expected_versions);
            assert_eq!(
                counts(store, "SELECT node_name AS name, COUNT(*) AS kept, MAX(observed_at) AS newest FROM sqb_warehouse_fingerprints GROUP BY node_name ORDER BY node_name").await,
                vec![
                    ("customers".to_owned(), kept, newest(builds)),
                    ("inventory".to_owned(), 1, newest(1)),
                    ("orders".to_owned(), kept, newest(builds)),
                ],
                "{}: builds kept per node, the newest always among them",
                test_case.description
            );
            assert_eq!(
                counts(store, "SELECT binding_key AS name, COUNT(*) AS kept, MAX(observed_at) AS newest FROM sqb_warehouse_audit_results GROUP BY binding_key ORDER BY binding_key").await,
                vec![
                    ("customers.unique_email".to_owned(), kept, newest(builds)),
                    ("orders.valid_orders".to_owned(), kept, newest(builds)),
                ],
                "{}: results kept per check",
                test_case.description
            );
            assert_eq!(
                prune_state_history(store, retention).await.expect("repeat"),
                (0, false),
                "{}: a repeated pass finds nothing",
                test_case.description
            );
            close_test_store(fixture_store).await;
        });
    }
}

#[test]
fn given_a_sqlite_store_when_pruning_frees_most_of_it_then_the_file_shrinks() {
    for incremental in [true, false] {
        run(async {
            let fixture_store = open_test_store(Backend::Sqlite).await.expect("store");
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            if !incremental {
                let ControlStore::Sqlite(path) = store else {
                    panic!("SQLite store");
                };
                rusqlite::Connection::open(&**path)
                    .expect("open")
                    .execute_batch("PRAGMA auto_vacuum=NONE; VACUUM")
                    .expect("a store created before incremental vacuum");
            }
            let large = "x".repeat(128 * 1024);
            let records: Vec<Value> = fingerprints("orders", 48)
                .into_iter()
                .map(|mut record| {
                    record["metadata_json"] =
                        json!(format!("{{\"note\":\"{large}{}\"}}", record["run_id"]));
                    record
                })
                .collect();
            for records in records.chunks(10) {
                ingest_fingerprints(
                    store,
                    (location, "development"),
                    &batch(session_id, "fingerprints", records.to_vec()),
                )
                .await
                .expect("large builds");
            }
            let before = pages(store).await;
            prune_state_history(store, StateHistoryRetention::versions(1).expect("one"))
                .await
                .expect("prune");
            while reclaim_sqlite_space(store).await.expect("reclaim") {
                if pages(store).await.1 == 0 {
                    break;
                }
            }
            let after = pages(store).await;
            assert!(
                after.0 * 2 < before.0,
                "incremental={incremental}: the file shrinks from {} to {} pages",
                before.0,
                after.0
            );
            assert_eq!(
                after.2, 2,
                "incremental={incremental}: the store reclaims space incrementally from now on"
            );
            close_test_store(fixture_store).await;
        });
    }
}

fn fingerprints(node: &str, builds: i64) -> Vec<Value> {
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

fn audit_results(binding: &str, results: i64) -> Vec<Value> {
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

fn at(index: i64) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0)
        .single()
        .expect("time")
        + Duration::hours(index)
}

/// The newest of `count` versions, as the store renders it.
fn newest(count: i64) -> chrono::DateTime<Utc> {
    at(count - 1)
}

async fn counts(store: &ControlStore, sql: &str) -> Vec<(String, usize, chrono::DateTime<Utc>)> {
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
async fn pages(store: &ControlStore) -> (i64, i64, i64) {
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
