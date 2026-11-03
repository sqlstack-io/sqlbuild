//! Replay and normal publication share the writer fence while readers keep a consistent cut.

use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::rebuild_projections::rebuild_projections;
use crate::snapshots::main::current_snapshot::current_snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::tests::helpers::snapshot;
use crate::snapshots::tests::test_types::SnapshotTestCase;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::logical_state::logical_state;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_concurrent_publication_when_replaying_then_readers_and_writers_keep_one_generation() {
    let test_cases = [
        SnapshotTestCase {
            description: "SQLite concurrent replay",
            backend: Backend::Sqlite,
            expected_generation: 2,
        },
        SnapshotTestCase {
            description: "PostgreSQL concurrent replay",
            backend: Backend::Postgres,
            expected_generation: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            input.documents.insert(
                "jobs".into(),
                serde_json::json!({
                    "orders":{"description":"Initial","selection":["orders"]}
                }),
            );
            let initial = publish_snapshot(store, &input).await.expect("initial");
            let replica = second_store(store).await;
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("held writer");
            for sql in [
                "DELETE FROM sqb_job_nodes",
                "DELETE FROM sqb_jobs",
                "DELETE FROM sqb_current_snapshots",
            ] {
                (transaction, _) = query(transaction, sql, vec![])
                    .await
                    .expect("uncommitted damage");
            }
            let (ready, started) = tokio::sync::oneshot::channel();
            let repair_store = replica.clone();
            let mut repair = tokio::spawn(async move {
                ready.send(()).expect("repair ready");
                rebuild_projections(&repair_store).await
            });
            started.await.expect("repair started");
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(100), &mut repair)
                    .await
                    .is_err()
            );
            input.generation = 2;
            input.documents.get_mut("jobs").expect("jobs")["orders"]["description"] =
                serde_json::json!("Current");
            input.documents.get_mut("jobs").expect("jobs")["orders"]["selection"] =
                serde_json::json!(["customers", "orders"]);
            let (ready, started) = tokio::sync::oneshot::channel();
            let publisher_store = replica.clone();
            let publisher = tokio::spawn(async move {
                ready.send(()).expect("publisher ready");
                publish_snapshot(&publisher_store, &input).await
            });
            started.await.expect("publisher started");
            assert!(!repair.is_finished());
            assert!(!publisher.is_finished());
            assert_eq!(
                current_snapshot(&replica, location, "development")
                    .await
                    .expect("concurrent reader")
                    .expect("previous generation")
                    .record,
                initial
            );
            commit(transaction).await.expect("release writer");
            tokio::time::timeout(std::time::Duration::from_secs(20), repair)
                .await
                .expect("repair deadline")
                .expect("join repair")
                .expect("repair");
            let published = tokio::time::timeout(std::time::Duration::from_secs(20), publisher)
                .await
                .expect("publisher deadline")
                .expect("join publisher")
                .expect("publish");
            let current = current_snapshot(&replica, location, "development")
                .await
                .expect("final reader")
                .expect("current generation");
            assert_eq!(current.record, published);
            assert_eq!(
                current.record.generation, test_case.expected_generation,
                "{}",
                test_case.description
            );
            assert_eq!(
                current.documents["jobs"]["orders"]["description"],
                "Current"
            );
            let final_state = logical_state(&replica).await;
            rebuild_projections(store).await.expect("replay newest cut");
            assert_eq!(logical_state(&replica).await, final_state);
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
