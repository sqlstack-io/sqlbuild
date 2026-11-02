//! Warehouse imports, atomic requests and durable receipt cursors run against both real stores.

use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::runs::main::list_runs::list_runs;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::{
    begin_sensor_evaluation::begin_sensor_evaluation,
    complete_builtin_sensor::complete_builtin_sensor,
};
use crate::sensors::tests::helpers::snapshot;
use crate::sensors::tests::test_types::SensorTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotNode;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::helpers::read_recall_events;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_metadata::main::ingest_source_freshness::ingest_source_freshness;
use crate::warehouse_metadata::models::{SourceFreshnessBatch, WarehouseFreshnessRecord};

#[test]
fn given_imported_source_versions_when_consuming_then_late_facts_queue_once_and_rebuilds_do_not_repeat()
 {
    let test_cases = [
        SensorTestCase {
            description: "SQLite source versions",
            backend: Backend::Sqlite,
            expected_runs: 2,
        },
        SensorTestCase {
            description: "PostgreSQL source versions",
            backend: Backend::Postgres,
            expected_runs: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required store");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            migrate_store(store).await.expect("warm migration");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.nodes.insert(
                "source:raw_orders".into(),
                SnapshotNode {
                    kind: "source".into(),
                    definition: json!({"name": "raw_orders"}),
                    raw_sql: None,
                    compiled_sql: None,
                    python_source: None,
                },
            );
            input.documents.insert(
                "triggers".into(),
                json!({"orders_changed": {
                    "description": "Refresh orders after source data changes.", "job_key": "orders",
                    "when": {"kind": "source_new_data", "selection": ["source:raw_orders"]},
                    "minimum_interval_seconds": 1
                }}),
            );
            publish_snapshot(store, &input)
                .await
                .expect("publish source sensor");
            let session_id = Uuid::now_v7();
            report_location_health(
                store,
                location,
                &LocationHealthReport {
                    event_id: Uuid::now_v7(),
                    session_id,
                    target: "development".into(),
                    health: LocationHealth::Heartbeat {},
                },
            )
            .await
            .expect("registered reader");
            let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/source_data_sensor.json");
            let data: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(fixture_path).expect("DuckShop source sensor"),
            )
            .expect("synthetic source versions");
            let mut records: Vec<WarehouseFreshnessRecord> = Vec::new();
            for index in 0..data["source_rows"].as_u64().expect("row count") {
                let mut record = data["record"].clone();
                record["event_id"] = json!(format!("orders-source-{index:03}"));
                records.push(
                    serde_json::from_value::<WarehouseFreshnessRecord>(record).expect("record"),
                );
            }
            for index in data["null_indices"].as_array().expect("null observations") {
                let record = &mut records[index.as_u64().expect("index") as usize];
                record.data_version = None;
                record.data_version_hash = "orders-null".into();
            }
            records[data["unselected_index"].as_u64().expect("index") as usize].source_name =
                "raw_products".into();
            let receipt_fault = TransactionFault {
                description: "source receipt rollback",
                table: "recall_events",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT, 'injected source receipt failure')",
                postgres_body: "RAISE EXCEPTION 'injected source receipt failure'",
            };
            install_fault(store, &receipt_fault).await;
            assert!(
                ingest_source_freshness(
                    store,
                    (location, "development"),
                    &SourceFreshnessBatch {
                        session_id,
                        generation: 1,
                        records: records[..1].to_vec(),
                        reconciliation: None,
                    }
                )
                .await
                .is_err()
            );
            remove_fault(store, &receipt_fault).await;
            let (transaction, mirrored) = query(
                begin(store, TransactionMode::Read)
                    .await
                    .expect("mirror read"),
                "SELECT COUNT(*) AS count FROM sqb_warehouse_freshness",
                vec![],
            )
            .await
            .expect("rolled back mirror");
            commit(transaction).await.expect("finish read");
            assert_eq!(mirrored[0]["count"], 0);
            for page in records.chunks(100) {
                let batch = SourceFreshnessBatch {
                    session_id,
                    generation: 1,
                    records: page.to_vec(),
                    reconciliation: None,
                };
                assert!(
                    ingest_source_freshness(store, (location, "development"), &batch)
                        .await
                        .expect("import")
                        .applied
                );
                assert!(
                    !ingest_source_freshness(store, (location, "development"), &batch)
                        .await
                        .expect("exact retry")
                        .applied
                );
            }
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("pre-sensor predecessor");
            for sql in [
                "DELETE FROM recall_events WHERE producer='warehouse' AND event_type='warehouse_source_freshness'",
                "DROP INDEX recall_warehouse_sources",
                "DELETE FROM sqb_schema_migrations WHERE name='source_sensors'",
            ] {
                (transaction, _) = query(transaction, sql, vec![])
                    .await
                    .expect("preserve mirror without source sensor receipts");
            }
            commit(transaction).await.expect("predecessor committed");
            install_fault(store, &receipt_fault).await;
            assert!(migrate_store(store).await.is_err());
            remove_fault(store, &receipt_fault).await;
            migrate_store(store)
                .await
                .expect("bootstrap preexisting source observations");
            migrate_store(store)
                .await
                .expect("warm bootstrap is idempotent");
            for page in records.chunks(100) {
                assert!(
                    !ingest_source_freshness(
                        store,
                        (location, "development"),
                        &SourceFreshnessBatch {
                            session_id,
                            generation: 1,
                            records: page.to_vec(),
                            reconciliation: None,
                        }
                    )
                    .await
                    .expect("already mirrored observation retry")
                    .applied
                );
            }
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let pending = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("evaluation")
                .expect("due");
            let fault = TransactionFault {
                description: "source cursor rollback",
                table: "sqb_sensor_cursor_commits",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT, 'injected source cursor failure')",
                postgres_body: "RAISE EXCEPTION 'injected source cursor failure'",
            };
            install_fault(store, &fault).await;
            assert!(
                complete_builtin_sensor(store, &lease, &pending)
                    .await
                    .is_err()
            );
            remove_fault(store, &fault).await;
            assert!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("rollback")
                    .is_empty()
            );
            assert_eq!(
                begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                    .await
                    .expect("retry"),
                Some(pending.clone())
            );
            assert_eq!(
                tokio::time::timeout(
                    Duration::from_secs(5),
                    complete_builtin_sensor(store, &lease, &pending)
                )
                .await
                .expect("bounded source page")
                .expect("complete"),
                1
            );
            assert_eq!(
                complete_builtin_sensor(store, &lease, &pending)
                    .await
                    .expect("lost acknowledgement retry"),
                0
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
            let second = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("second page")
                .expect("due");
            assert!(second.cursor.event_storage_id > 0);
            assert_eq!(
                complete_builtin_sensor(store, &lease, &second)
                    .await
                    .expect("unchanged source versions"),
                0
            );
            let mut late = data["record"].as_object().expect("record").clone();
            late.extend(
                data["late_record"]
                    .as_object()
                    .expect("late source observation")
                    .clone(),
            );
            let batch = SourceFreshnessBatch {
                session_id,
                generation: 1,
                records: vec![serde_json::from_value(json!(late)).expect("late record")],
                reconciliation: None,
            };
            ingest_source_freshness(store, (location, "development"), &batch)
                .await
                .expect("late commit");
            tokio::time::sleep(Duration::from_secs(1)).await;
            let later = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("late evaluation")
                .expect("due");
            assert_eq!(
                complete_builtin_sensor(store, &lease, &later)
                    .await
                    .expect("late warehouse time must not skip new receipt"),
                1
            );
            let events = read_recall_events(store, 0, 1000)
                .await
                .expect("durable observations");
            let source_events = events
                .iter()
                .filter(|event| event["event_type"] == "warehouse_source_freshness")
                .count();
            assert_eq!(
                source_events,
                records.len() + 1
                    - data["null_indices"]
                        .as_array()
                        .expect("null observations")
                        .len()
            );
            let (transaction, _) = query(
                begin(store, TransactionMode::Write)
                    .await
                    .expect("disposable mirror"),
                "DELETE FROM sqb_warehouse_freshness",
                vec![],
            )
            .await
            .expect("clear mirror");
            commit(transaction).await.expect("clear mirror committed");
            for page in records.chunks(100) {
                ingest_source_freshness(
                    store,
                    (location, "development"),
                    &SourceFreshnessBatch {
                        session_id,
                        generation: 1,
                        records: page.to_vec(),
                        reconciliation: None,
                    },
                )
                .await
                .expect("rebuild mirror");
            }
            ingest_source_freshness(store, (location, "development"), &batch)
                .await
                .expect("rebuild late fact");
            tokio::time::sleep(Duration::from_secs(1)).await;
            let rebuilt = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("rebuilt evaluation")
                .expect("due");
            assert_eq!(
                complete_builtin_sensor(store, &lease, &rebuilt)
                    .await
                    .expect("rebuild has no new source facts"),
                0
            );
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("queued requests")
                    .len(),
                test_case.expected_runs,
                "{}",
                test_case.description
            );
            let rebuilt = read_recall_events(store, 0, 1000)
                .await
                .expect("rebuilt observations");
            assert_eq!(
                rebuilt
                    .iter()
                    .filter(|event| event["event_type"] == "warehouse_source_freshness")
                    .count(),
                source_events
            );
            close_test_store(fixture).await;
        });
    }
}
