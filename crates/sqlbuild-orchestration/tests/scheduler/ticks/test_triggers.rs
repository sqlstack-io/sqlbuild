//! Operator trigger facts remain consistent across publication and retry boundaries.

use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::declarations::main::change_trigger::change_trigger;
use sqlbuild_orchestration::declarations::main::list_trigger_events::list_trigger_events;
use sqlbuild_orchestration::declarations::main::list_triggers::list_triggers;
use sqlbuild_orchestration::declarations::models::{
    TriggerChange, TriggerChangeRequest, TriggerStatus,
};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::scheduler::main::evaluate_trigger::evaluate_trigger;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::fault_helpers::{install_fault, remove_fault};
use crate::location_helpers::create_location;
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::ticks::helpers::{snapshot, trigger_audits};
use crate::ticks::test_types::{TransactionFault, TriggerControlsTestCase};

#[test]
fn given_operator_changes_when_retried_and_republished_then_facts_and_effective_state_remain_consistent()
 {
    let test_cases = [
        TriggerControlsTestCase {
            description: "SQLite operator facts",
            backend: Backend::Sqlite,
            expected_events: 5,
        },
        TriggerControlsTestCase {
            description: "PostgreSQL operator facts",
            backend: Backend::Postgres,
            expected_events: 5,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate trigger store");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            publish_snapshot(store, &input)
                .await
                .expect("initial publish");
            let trigger_id = list_triggers(store, location, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let pause = TriggerChangeRequest {
                event_id: Uuid::now_v7(),
                trigger_id,
                actor: "orders-operator".into(),
                reason: "maintenance".into(),
                change: TriggerChange::Pause,
            };
            let audit = AuditContext {
                actor: "orders-operator".into(),
                granting_role: "operator".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            assert!(change_trigger(store, &audit, &pause).await.expect("pause"));
            let paused = list_triggers(store, location, "development")
                .await
                .expect("paused trigger")
                .remove(0);
            assert_eq!(
                paused.status,
                TriggerStatus::Paused,
                "{}",
                test_case.description
            );
            assert!(paused.next_due_at.is_none());
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("election")
                .expect("lease");
            assert!(
                evaluate_trigger(store, &lease, trigger_id)
                    .await
                    .expect("paused evaluation")
                    .is_none()
            );
            let enable = TriggerChangeRequest {
                event_id: Uuid::now_v7(),
                change: TriggerChange::Enable,
                ..pause.clone()
            };
            for fault in [
                TransactionFault {
                    description: "operator event and projection",
                    table: "sqb_trigger_state",
                    operation: "UPDATE",
                    condition: "",
                    sqlite_body: "SELECT RAISE(ABORT,'injected state failure')",
                    postgres_body: "RAISE EXCEPTION 'injected state failure'",
                },
                TransactionFault {
                    description: "operator security audit",
                    table: "sqb_security_audit",
                    operation: "INSERT",
                    condition: "",
                    sqlite_body: "SELECT RAISE(ABORT,'injected audit failure')",
                    postgres_body: "RAISE EXCEPTION 'injected audit failure'",
                },
            ] {
                install_fault(store, &fault).await;
                assert!(change_trigger(store, &audit, &enable).await.is_err());
                remove_fault(store, &fault).await;
                assert_eq!(
                    list_trigger_events(store, trigger_id, 100)
                        .await
                        .expect("rolled-back fact")
                        .len(),
                    1
                );
                assert_eq!(
                    list_triggers(store, location, "development")
                        .await
                        .expect("rolled back projection")[0]
                        .status,
                    TriggerStatus::Paused
                );
            }
            assert!(
                change_trigger(store, &audit, &enable)
                    .await
                    .expect("retry enable")
            );
            assert!(
                !change_trigger(store, &audit, &pause)
                    .await
                    .expect("retry old pause is inert")
            );
            let conflict = TriggerChangeRequest {
                change: TriggerChange::ResetDefault,
                ..pause.clone()
            };
            assert!(
                change_trigger(store, &audit, &conflict).await.is_err(),
                "same event ID cannot change content"
            );
            input.generation = 2;
            input
                .documents
                .get_mut("triggers")
                .expect("trigger document")["daily_orders"]["default_status"] =
                serde_json::json!("paused");
            publish_snapshot(store, &input)
                .await
                .expect("publish a changed code default");
            assert_eq!(
                list_triggers(store, location, "development")
                    .await
                    .expect("retained override")[0]
                    .status,
                TriggerStatus::Running
            );
            let reset = TriggerChangeRequest {
                event_id: Uuid::now_v7(),
                change: TriggerChange::ResetDefault,
                ..pause.clone()
            };
            change_trigger(store, &audit, &reset)
                .await
                .expect("restore new code default");
            assert_eq!(
                list_triggers(store, location, "development")
                    .await
                    .expect("default restored")[0]
                    .status,
                TriggerStatus::Paused
            );
            let time = chrono::DateTime::parse_from_rfc3339("2020-01-02T00:00:00Z")
                .expect("cursor")
                .to_utc();
            let cursor = TriggerChangeRequest {
                event_id: Uuid::now_v7(),
                change: TriggerChange::SetCursor { time },
                ..pause.clone()
            };
            change_trigger(store, &audit, &cursor)
                .await
                .expect("set durable cursor");
            assert_eq!(
                list_triggers(store, location, "development")
                    .await
                    .expect("cursor set")[0]
                    .last_tick_for,
                Some(time)
            );
            let reset_cursor = TriggerChangeRequest {
                event_id: Uuid::now_v7(),
                change: TriggerChange::ResetCursor,
                ..pause.clone()
            };
            change_trigger(store, &audit, &reset_cursor)
                .await
                .expect("reset cursor");
            assert!(
                list_triggers(store, location, "development")
                    .await
                    .expect("cursor reset")[0]
                    .last_tick_for
                    .is_none()
            );
            let events = list_trigger_events(store, trigger_id, 100)
                .await
                .expect("operator history");
            assert_eq!(events.len(), test_case.expected_events);
            assert!(
                events
                    .iter()
                    .all(|event| event.request_json.actor == "orders-operator")
            );
            let other_actor = AuditContext {
                actor: "customers-operator".into(),
                ..audit.clone()
            };
            assert!(change_trigger(store, &other_actor, &pause).await.is_err());
            let audits = trigger_audits(store).await;
            assert_eq!(audits.len(), test_case.expected_events);
            assert!(
                audits
                    .iter()
                    .all(|row| row["actor"] == "orders-operator"
                        && row["granting_role"] == "operator")
            );
            let cleared = audits
                .iter()
                .find(|row| row["event_id"] == reset_cursor.event_id.to_string())
                .expect("cursor reset audit");
            assert_eq!(
                cleared["before_json"]["last_tick_for"],
                "2020-01-02T00:00:00Z"
            );
            assert!(cleared["after_json"]["last_tick_for"].is_null());
            assert_eq!(cleared["after_json"]["reason"], "maintenance");
            close_test_store(fixture).await;
        });
    }
}
