//! Reset/re-evaluation records the distinguishing fact even when every tick already exists.

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::_helpers::operator_change::change_trigger_in;
use crate::declarations::main::{change_trigger::change_trigger, list_triggers::list_triggers};
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::platform::main::rebuild_projections::rebuild_projections;
use crate::scheduler::main::{_acquire_lease::acquire_lease, _evaluate_trigger::evaluate_trigger};
use crate::scheduler::tests::contracts::ticks::helpers::{calendar_text, calendar_time, snapshot};
use crate::scheduler::tests::contracts::ticks::test_types::TriggerControlsTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::logical_state::logical_state;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_reset_existing_ticks_when_progress_is_replayed_then_cursor_and_due_time_are_exact() {
    let test_cases = [
        TriggerControlsTestCase {
            description: "SQLite exact semantic progress and atomic repair",
            backend: Backend::Sqlite,
            expected_events: 4,
        },
        TriggerControlsTestCase {
            description: "PostgreSQL exact semantic progress and atomic repair",
            backend: Backend::Postgres,
            expected_events: 4,
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
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publication");
            let id = list_triggers(store, location, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("election")
                .expect("lease");
            evaluate_trigger(store, &lease, id)
                .await
                .expect("first drain");
            let audit = AuditContext {
                actor: "orders-operator".into(),
                granting_role: "operator".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            change_trigger(
                store,
                &audit,
                &TriggerChangeRequest {
                    event_id: Uuid::now_v7(),
                    trigger_id: id,
                    actor: audit.actor.clone(),
                    reason: "Replay retained calendar".into(),
                    change: TriggerChange::ResetCursor,
                },
            )
            .await
            .expect("reset");
            let reset = list_triggers(store, location, "development")
                .await
                .expect("reset state")
                .remove(0);
            assert!(reset.last_tick_for.is_none());
            assert_eq!(reset.next_due_at.expect("first due"), calendar_time(0));
            let fault = TransactionFault {
                description: "progress publication failure",
                table: "sqb_trigger_progress",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'synthetic progress failure')",
                postgres_body: "RAISE EXCEPTION 'synthetic progress failure'",
            };
            install_fault(store, &fault).await;
            let before = logical_state(store).await;
            let error = evaluate_trigger(store, &lease, id)
                .await
                .expect_err("fact failure rejects cursor update");
            assert!(error.to_string().contains("synthetic progress failure"));
            assert_eq!(
                logical_state(store).await,
                before,
                "entire transaction rolled back"
            );
            remove_fault(store, &fault).await;
            evaluate_trigger(store, &lease, id)
                .await
                .expect("re-evaluate retained ticks");
            let expected = list_triggers(store, location, "development")
                .await
                .expect("advanced state");
            assert_eq!(
                expected[0].last_tick_for.expect("last due"),
                calendar_time(2)
            );
            assert_eq!(
                expected[0].next_due_at.expect("future due").to_rfc3339(),
                "2030-01-01T00:00:00+00:00"
            );
            let transaction = begin(store, TransactionMode::Read)
                .await
                .expect("source cut");
            let (transaction, facts) = query(
                transaction,
                "SELECT projection_json FROM sqb_trigger_progress ORDER BY storage_id",
                vec![],
            )
            .await
            .expect("progress");
            assert_eq!(
                facts.len(),
                test_case.expected_events,
                "{}: publish, drain, reset, drain despite duplicate ticks",
                test_case.description
            );
            assert_eq!(facts[2]["projection_json"]["last_tick_for"], json!(null));
            assert_eq!(
                facts[3]["projection_json"]["last_tick_for"],
                json!(calendar_text(2))
            );
            commit(transaction).await.expect("source cut complete");
            let mut transaction = begin(store, TransactionMode::Write)
                .await
                .expect("backward clock transition");
            transaction.now = "1999-01-01T00:00:00Z"
                .parse()
                .expect("synthetic database clock");
            let pause = TriggerChangeRequest {
                event_id: Uuid::now_v7(),
                trigger_id: id,
                actor: audit.actor.clone(),
                reason: "Pause after clock step".into(),
                change: TriggerChange::Pause,
            };
            let (transaction, applied) = change_trigger_in(transaction, &audit, &pause)
                .await
                .expect("causally later pause");
            assert!(applied);
            commit(transaction)
                .await
                .expect("clock-step fact and state");
            let expected = list_triggers(store, location, "development")
                .await
                .expect("causally latest paused trigger");
            assert_eq!(
                expected[0].status,
                crate::declarations::models::TriggerStatus::Paused
            );
            let original = logical_state(store).await;
            let mut transaction = begin(store, TransactionMode::Write).await.expect("damage");
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/projection_replay.json");
            let damage: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("damage fixture"))
                    .expect("damage cases");
            for sql in damage["trigger_damage"].as_array().expect("trigger damage") {
                (transaction, _) = query(transaction, sql.as_str().expect("damage SQL"), vec![])
                    .await
                    .expect("corrupt projections");
            }
            commit(transaction).await.expect("commit damage");
            let damaged = logical_state(store).await;
            let late_fault = TransactionFault {
                description: "late run replay failure after trigger repair",
                table: "sqb_run_nodes",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'synthetic late replay failure')",
                postgres_body: "RAISE EXCEPTION 'synthetic late replay failure'",
            };
            install_fault(store, &late_fault).await;
            assert!(
                rebuild_projections(store)
                    .await
                    .expect_err("late failure")
                    .to_string()
                    .contains("synthetic late replay failure")
            );
            assert_eq!(
                logical_state(store).await,
                damaged,
                "no earlier phase commits"
            );
            remove_fault(store, &late_fault).await;
            rebuild_projections(store).await.expect("atomic replay");
            assert_eq!(
                list_triggers(store, location, "development")
                    .await
                    .expect("restored"),
                expected
            );
            assert_eq!(
                logical_state(store).await,
                original,
                "all protected rows and original timestamps retained"
            );
            rebuild_projections(store).await.expect("idempotent replay");
            assert_eq!(logical_state(store).await, original);
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("legacy source cut");
            let (transaction, _) = query(transaction, "DELETE FROM sqb_trigger_progress", vec![])
                .await
                .expect("legacy history contains no progress facts");
            let (transaction, _) = query(transaction, "DELETE FROM sqb_current_snapshots", vec![])
                .await
                .expect("earlier repair needed");
            commit(transaction).await.expect("legacy fixture");
            let legacy = logical_state(store).await;
            let error = rebuild_projections(store)
                .await
                .expect_err("ambiguous reset/re-evaluation cannot be guessed");
            assert!(error.to_string().contains("lacks semantic progress"));
            assert_eq!(
                logical_state(store).await,
                legacy,
                "legacy refusal rolls back current pointers too"
            );
            close_test_store(fixture).await;
        });
    }
}
