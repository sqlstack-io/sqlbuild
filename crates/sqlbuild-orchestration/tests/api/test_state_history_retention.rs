//! The control store keeps a bounded number of builds per node and results per check.

use crate::api::tests::state_history_helpers::{
    audit_results, counts, fingerprints, large_fingerprints, newest, pages, set_auto_vacuum,
};
use crate::api::tests::test_types::{SqliteReclaimTestCase, StateHistoryRetentionTestCase};
use crate::api::tests::warehouse_history_helpers::{batch, seed_target};
use crate::platform::_helpers::maintenance::retention::reclaim_sqlite_space;
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
            prune_state_history(store, retention).await.expect("prune");
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
    let test_cases = [
        SqliteReclaimTestCase {
            description: "a store in incremental mode frees pages step by step",
            auto_vacuum: "INCREMENTAL",
            expected_mode: 2,
        },
        SqliteReclaimTestCase {
            description: "a store without incremental mode is vacuumed into it once",
            auto_vacuum: "NONE",
            expected_mode: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture_store = open_test_store(Backend::Sqlite).await.expect("store");
            let store = &fixture_store.store;
            let (location, session_id) = seed_target(store).await;
            set_auto_vacuum(store, test_case.auto_vacuum);
            for records in large_fingerprints("orders", 48).chunks(10) {
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
            assert!(
                reclaim_sqlite_space(store).await.expect("reclaim"),
                "{}: one pass reclaims space",
                test_case.description
            );
            let after = pages(store).await;
            assert!(
                after.0 * 2 < before.0,
                "{}: the file shrinks from {} to {} pages",
                test_case.description,
                before.0,
                after.0
            );
            assert_eq!(
                after.2, test_case.expected_mode,
                "{}: the store reclaims space incrementally from now on",
                test_case.description
            );
            close_test_store(fixture_store).await;
        });
    }
}
