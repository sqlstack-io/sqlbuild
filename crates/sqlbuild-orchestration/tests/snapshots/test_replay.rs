//! Current pointers and jobs recover from retained facts, independently of trigger replay.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::scheduler::tests::contracts::helpers::{acquire_lease, evaluate_trigger};
use crate::snapshots::_helpers::rebuild::rebuild_snapshot_projections;
use crate::snapshots::main::current_snapshot::current_snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::tests::helpers::{damage, snapshot};
use crate::snapshots::tests::test_types::SnapshotTestCase;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::logical_state::logical_state;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_corrupt_snapshot_projections_when_replayed_then_full_state_is_restored() {
    let test_cases = [
        SnapshotTestCase {
            description: "SQLite replay",
            backend: Backend::Sqlite,
            expected_generation: 7,
        },
        SnapshotTestCase {
            description: "PostgreSQL replay",
            backend: Backend::Postgres,
            expected_generation: 7,
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
            input.documents.insert("jobs".into(), json!({
                "customers":{"description":"Customers","selection":["customers"]},
                "orders":{"description":"Orders","selection":["orders"],"settings":{"threads":3}}
            }));
            input.documents.insert(
                "groups".into(),
                json!({
                    "nightly":{"description":"Nightly","selection":["customers","orders"],
                        "jobs":{"customers":[],"orders":["customers"]}}
                }),
            );
            input.documents.insert("triggers".into(), json!({
                "daily":{"description":"Daily","job_key":"orders","starts_at":"2020-01-01T00:00:00Z",
                    "schedule":{"kind":"calendar","times":["2020-01-01T00:00:00Z","2030-01-01T00:00:00Z"],"refresh_at":"2029-01-01T00:00:00Z"}}
            }));
            publish_snapshot(store, &input)
                .await
                .expect("first generation");
            input.generation = 7;
            input.documents.get_mut("jobs").expect("jobs")["orders"]["description"] =
                json!("Current orders");
            let latest = publish_snapshot(store, &input)
                .await
                .expect("current generation");
            assert_eq!(
                latest.generation, test_case.expected_generation,
                "{}",
                test_case.description
            );
            let trigger = crate::declarations::main::list_triggers::list_triggers(
                store,
                location,
                "development",
            )
            .await
            .expect("trigger")
            .remove(0);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("lease")
                .expect("leader");
            evaluate_trigger(store, &lease, trigger.trigger_id)
                .await
                .expect("retained run/tick facts");
            input.generation = 1;
            for index in 0..101 {
                input.target = format!("target_{index:03}");
                publish_snapshot(store, &input)
                    .await
                    .expect("independent target");
            }
            let other = create_location(store, "inventory", "test-admin")
                .await
                .expect("other location");
            input.location_id = other;
            input.target = "development".into();
            input.documents.clear();
            publish_snapshot(store, &input)
                .await
                .expect("jobless scope");
            let expected = logical_state(store).await;
            damage(store).await;
            let damaged = logical_state(store).await;
            assert_ne!(damaged, expected);
            let fault = TransactionFault {
                description: "late pointer failure after job repair",
                table: "sqb_current_snapshots",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected replay failure')",
                postgres_body: "RAISE EXCEPTION 'injected replay failure'",
            };
            install_fault(store, &fault).await;
            assert!(
                rebuild_snapshot_projections(store)
                    .await
                    .expect_err("injected failure")
                    .to_string()
                    .contains("injected replay failure")
            );
            assert_eq!(logical_state(store).await, damaged);
            remove_fault(store, &fault).await;
            rebuild_snapshot_projections(store).await.expect("replay");
            assert_eq!(logical_state(store).await, expected);
            assert_eq!(
                current_snapshot(store, location, "development")
                    .await
                    .expect("read")
                    .expect("restored")
                    .record,
                latest
            );
            rebuild_snapshot_projections(store)
                .await
                .expect("idempotent replay");
            assert_eq!(logical_state(store).await, expected);

            damage(store).await;
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("source fault");
            let (transaction, rows) = query(transaction, "SELECT document_hash,body FROM sqb_documents WHERE document_hash=(SELECT document_hash FROM sqb_snapshot_documents WHERE snapshot_id=$1 AND kind='jobs')", vec![Parameter::Id(latest.snapshot_id)]).await.expect("source body");
            let body: Vec<u8> = serde_json::from_value(rows[0]["body"].clone()).expect("bytes");
            let hash = rows[0]["document_hash"].as_str().expect("hash").to_owned();
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_documents SET body=$1 WHERE document_hash=$2",
                vec![
                    Parameter::Bytes(b"invalid compressed source".to_vec()),
                    Parameter::Text(hash.clone()),
                ],
            )
            .await
            .expect("malformed retained source");
            commit(transaction).await.expect("source fault commits");
            let malformed = logical_state(store).await;
            assert!(rebuild_snapshot_projections(store).await.is_err());
            assert_eq!(logical_state(store).await, malformed);
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("restore source");
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_documents SET body=$1 WHERE document_hash=$2",
                vec![Parameter::Bytes(body), Parameter::Text(hash)],
            )
            .await
            .expect("source restored");
            commit(transaction).await.expect("commit restored source");
            rebuild_snapshot_projections(store)
                .await
                .expect("retry restored source");
            assert_eq!(logical_state(store).await, expected);
            close_test_store(fixture).await;
        });
    }
}
