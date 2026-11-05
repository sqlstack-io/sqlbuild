//! Reset and re-evaluation advance the cursor exactly even when every tick already exists.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::_helpers::operator_change::change_trigger_in;
use crate::declarations::main::{
    change_trigger::change_trigger, list_trigger_events::list_trigger_events,
    list_triggers::list_triggers,
};
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::scheduler::main::{_acquire_lease::acquire_lease, _evaluate_trigger::evaluate_trigger};
use crate::scheduler::tests::contracts::ticks::helpers::{calendar_time, snapshot};
use crate::scheduler::tests::contracts::ticks::test_types::TriggerControlsTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::{begin::begin, commit::commit, migrate_store::migrate_store};
use crate::store::models::TransactionMode;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::logical_state::logical_state;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_reset_existing_ticks_when_re_evaluated_then_cursor_and_due_time_are_exact() {
    let test_cases = [
        TriggerControlsTestCase {
            description: "SQLite exact reset and re-evaluation",
            backend: Backend::Sqlite,
            expected_events: 2,
        },
        TriggerControlsTestCase {
            description: "PostgreSQL exact reset and re-evaluation",
            backend: Backend::Postgres,
            expected_events: 2,
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
                description: "trigger state failure",
                table: "sqb_trigger_state",
                operation: "UPDATE",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'synthetic trigger state failure')",
                postgres_body: "RAISE EXCEPTION 'synthetic trigger state failure'",
            };
            install_fault(store, &fault).await;
            let before = logical_state(store).await;
            let error = evaluate_trigger(store, &lease, id)
                .await
                .expect_err("fact failure rejects cursor update");
            assert!(
                error
                    .to_string()
                    .contains("synthetic trigger state failure")
            );
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
            assert_eq!(
                list_trigger_events(store, id, 100)
                    .await
                    .expect("operator history")
                    .len(),
                test_case.expected_events,
                "{}: reset and pause are the only operator facts",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
