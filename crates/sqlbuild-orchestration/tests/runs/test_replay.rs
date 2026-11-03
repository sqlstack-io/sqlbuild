//! Causal replay crosses pages and restores a missing projection without rebuilding sources.

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::_helpers::lifecycle::change_run::change_run_in;
use crate::runs::_helpers::requests::publication::publish_request;
use crate::runs::main::list_runs::list_runs;
use crate::runs::models::{RunAuthority, RunChange, RunChangeRequest, RunStatus};
use crate::runs::tests::helpers::{queue, request, snapshot};
use crate::runs::tests::replay_helpers::{rebuild_run_projections, run_replay_contract};
use crate::runs::tests::test_types::RunReplayTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::logical_state::logical_state;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use uuid::Uuid;

#[test]
fn given_paged_run_facts_when_replaying_then_missing_rows_and_original_times_return() {
    let test_cases = [
        RunReplayTestCase {
            description: "SQLite paged replay",
            backend: Backend::Sqlite,
            expected_runs: 56,
        },
        RunReplayTestCase {
            description: "PostgreSQL paged replay",
            backend: Backend::Postgres,
            expected_runs: 56,
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
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("snapshot");
            for id in (1..=55).rev() {
                queue(
                    store,
                    &request(
                        (location, published.snapshot_id),
                        id,
                        id as i64,
                        &["customers", "orders"],
                    ),
                )
                .await;
            }
            let queued_at = list_runs(store, (location, "development"), 100)
                .await
                .expect("original times")
                .into_iter()
                .find(|run| run.run_id == Uuid::from_u128(1))
                .expect("original run")
                .queued_at;
            let replica = second_store(store).await;
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("held writer");
            transaction.now = published.published_at - chrono::Duration::days(1);
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "operator".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let (transaction, applied) = change_run_in(
                transaction,
                RunAuthority::Operator(&context),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: Uuid::from_u128(1),
                    claim_id: None,
                    change: RunChange::CancelRequested {
                        actor: context.actor.clone(),
                        reason: "clock rollback".into(),
                    },
                },
            )
            .await
            .expect("cancel after a backward clock step");
            assert!(applied);
            let (transaction, applied) = publish_request(
                transaction,
                &request((location, published.snapshot_id), 56, 56, &["orders"]),
            )
            .await
            .expect("new uncommitted request");
            assert!(applied);
            let repair_store = replica.clone();
            let mut repair =
                tokio::spawn(async move { rebuild_run_projections(&repair_store).await });
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(100), &mut repair)
                    .await
                    .is_err()
            );
            assert_eq!(
                list_runs(&replica, (location, "development"), 100)
                    .await
                    .expect("old source cut")
                    .len(),
                55
            );
            commit(transaction).await.expect("publish new source cut");
            tokio::time::timeout(std::time::Duration::from_secs(20), repair)
                .await
                .expect("repair deadline")
                .expect("join repair")
                .expect("serialized replay");
            run_replay_contract(store).await;
            let expected = logical_state(store).await;
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("missing projections");
            let (transaction, _) = query(transaction, "DELETE FROM sqb_run_nodes", vec![])
                .await
                .expect("remove nodes");
            let (transaction, _) = query(transaction, "DELETE FROM sqb_runs", vec![])
                .await
                .expect("remove metadata");
            commit(transaction)
                .await
                .expect("missing projections commit");
            rebuild_run_projections(store)
                .await
                .expect("restore missing runs");
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("runs")
                    .len(),
                test_case.expected_runs,
                "{}",
                test_case.description
            );
            assert_eq!(logical_state(store).await, expected);
            let runs = list_runs(store, (location, "development"), 100)
                .await
                .expect("replayed states");
            assert_eq!(
                runs.iter()
                    .find(|run| run.run_id == Uuid::from_u128(1))
                    .expect("cancelled run")
                    .status,
                RunStatus::Cancelled
            );
            let cancelled = runs
                .iter()
                .find(|run| run.run_id == Uuid::from_u128(1))
                .expect("original identity");
            assert_eq!(cancelled.queued_at, queued_at);
            assert_eq!(
                cancelled.ended_at,
                Some(published.published_at - chrono::Duration::days(1))
            );
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("source fault");
            let (transaction, _) = query(
                transaction,
                "UPDATE recall_events SET event_schema_version=999 WHERE storage_id=(SELECT MAX(storage_id) FROM recall_events WHERE producer='scheduler')",
                vec![],
            )
            .await
            .expect("unsupported schema");
            commit(transaction).await.expect("source fault commit");
            let unsupported = logical_state(store).await;
            assert!(
                rebuild_run_projections(store)
                    .await
                    .expect_err("unsupported source")
                    .to_string()
                    .contains("unsupported run fact schema")
            );
            assert_eq!(logical_state(store).await, unsupported);
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
