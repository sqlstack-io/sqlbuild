//! Snapshot GC keeps the newest generations and every referenced snapshot, and collects content.

use serde_json::json;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::runs::tests::helpers::{request, request_run};
use crate::scheduler::tests::contracts::ticks::helpers::snapshot;
use crate::snapshots::_helpers::retention::prune_snapshots;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::SnapshotRetention;
use crate::snapshots::tests::helpers::{retained_snapshot_state, retention_counts};
use crate::snapshots::tests::test_types::SnapshotRetentionTestCase;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_twelve_snapshots_when_collected_then_ten_newest_and_run_referenced_snapshots_survive() {
    let test_cases = [
        SnapshotRetentionTestCase {
            description: "SQLite keeps ten newest plus the run-pinned first generation",
            backend: Backend::Sqlite,
            expected_generations: &[1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        },
        SnapshotRetentionTestCase {
            description: "PostgreSQL keeps ten newest plus the run-pinned first generation",
            backend: Backend::Postgres,
            expected_generations: &[1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        },
    ];
    for test_case in test_cases {
        run(async {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/snapshot_retention.json");
            let case: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("snapshot fixture"))
                    .expect("snapshot case");
            let setting = |name: &str| {
                SnapshotRetention::count(
                    u16::try_from(case[name].as_u64().expect("count")).expect("u16"),
                )
                .expect("retention")
            };
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let generation_input = |generation: i64| {
                let mut input = snapshot(location);
                input.generation = generation;
                let node = input.nodes.get_mut("orders").expect("orders node");
                node.raw_sql = Some(format!("select {generation} as order_id"));
                node.compiled_sql = node.raw_sql.clone();
                node.definition = json!({"materialization": "table", "revision": generation});
                input
            };
            let first = publish_snapshot(store, &generation_input(1))
                .await
                .expect("publish first");
            assert!(
                request_run(
                    store,
                    &request((location, first.snapshot_id), 1, 0, &["orders"])
                )
                .await
                .expect("pinned run")
            );
            for generation in 2..=case["publications"].as_i64().expect("publications") {
                publish_snapshot(store, &generation_input(generation))
                    .await
                    .expect("publish");
            }
            let triggers = list_triggers(store, location, "development")
                .await
                .expect("triggers before collection");
            assert_eq!(retention_counts(store).await, (12, 0));
            assert_eq!(
                prune_snapshots(store, setting("keep"))
                    .await
                    .expect("collect") as u64,
                case["expected_collected"].as_u64().expect("collected")
            );
            assert_eq!(
                retained_snapshot_state(store, location).await,
                test_case.expected_generations,
                "{}",
                test_case.description
            );
            assert_eq!(
                retention_counts(store).await,
                (11, 0),
                "the expired snapshot's text goes with it"
            );
            assert_eq!(
                prune_snapshots(store, setting("keep"))
                    .await
                    .expect("idempotent"),
                0
            );
            assert!(SnapshotRetention::count(0).is_err());
            assert_eq!(
                prune_snapshots(store, setting("shorter_keep"))
                    .await
                    .expect("shorter retention"),
                9
            );
            assert_eq!(
                serde_json::json!(retained_snapshot_state(store, location).await),
                case["expected_after_shorter"]
            );
            assert_eq!(retention_counts(store).await.1, 0);
            assert_eq!(
                list_triggers(store, location, "development")
                    .await
                    .expect("triggers after collection"),
                triggers,
                "collection never changes live trigger state"
            );
            close_test_store(fixture).await;
        });
    }
}
