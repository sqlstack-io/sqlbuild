use serde_json::{Value, json};
use tokio::time::Instant;
use uuid::Uuid;

use crate::api::tests::test_types::IngestionTurnTestCase;
use crate::api::tests::warehouse_history_helpers::seed_target;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest::ingest_catalog::ingest_catalog;
use crate::warehouse_metadata::models::MetadataBatch;

#[test]
fn given_a_sync_ingesting_pages_when_another_writer_waits_then_it_writes_before_the_sync_ends() {
    let test_cases = [IngestionTurnTestCase {
        description: "a scheduler lease during three 2,000-relation catalog pages",
        pages: 3,
        relations_per_page: 2000,
        expected_writer_first: true,
    }];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            let (location, session_id) = seed_target(store).await;
            let columns: Vec<Value> = (0..12)
                .map(|index| json!({"name": format!("column_{index}"), "data_type": "INTEGER"}))
                .collect();
            let batches: Vec<MetadataBatch> = (0..test_case.pages)
                .map(|page| {
                    let records: Vec<Value> = (0..test_case.relations_per_page)
                        .map(|index| {
                            json!({"state_database": null, "state_schema": "main",
                                "name": format!("orders_{page}_{index:05}"),
                                "relation_type": "BASE TABLE", "columns": columns})
                        })
                        .collect();
                    serde_json::from_value(json!({"kind": "catalog", "session_id": session_id,
                        "generation": 1, "records": records, "scan": {"scan_id": Uuid::now_v7(),
                        "location": {"database": null, "schema": "main"}, "complete": false}}))
                    .expect("batch")
                })
                .collect();
            let sync = async {
                for batch in &batches {
                    ingest_catalog(store, (location, "development"), batch)
                        .await
                        .expect("ingest");
                }
                Instant::now()
            };
            let lease = async {
                tokio::task::yield_now().await;
                let transaction = begin(store, TransactionMode::Write).await.expect("begin");
                commit(transaction).await.expect("commit");
                Instant::now()
            };
            let (synced, leased) = tokio::join!(sync, lease);
            assert_eq!(
                leased < synced,
                test_case.expected_writer_first,
                "{}: the waiting writer takes its turn between the sync's short transactions",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
