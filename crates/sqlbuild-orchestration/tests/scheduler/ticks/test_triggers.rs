//! Operator trigger facts remain consistent across publication and retry boundaries.

use crate::credentials::models::AuditContext;
use crate::declarations::main::change_trigger::change_trigger;
use crate::declarations::main::list_trigger_events::list_trigger_events;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest, TriggerStatus};
use crate::scheduler::main::_acquire_lease::acquire_lease;
use crate::scheduler::main::_evaluate_trigger::evaluate_trigger;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::scheduler::tests::contracts::ticks::helpers::{snapshot, trigger_audits};
use crate::scheduler::tests::contracts::ticks::test_types::{
    TickTransactionTestCase, TransactionFault, TriggerControlsTestCase,
};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

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

#[test]
fn given_a_calendar_with_unprocessed_due_times_when_refreshed_then_due_ticks_survive_and_only_future_times_are_replaced()
 {
    use crate::runs::main::list_runs::list_runs;
    use crate::scheduler::main::list_ticks::list_ticks;
    use serde_json::json;

    let test_cases = [
        TickTransactionTestCase {
            description: "SQLite retains due calendar ticks",
            backend: Backend::Sqlite,
            expected_ticks: 3,
            expected_runs: 1,
        },
        TickTransactionTestCase {
            description: "PostgreSQL retains due calendar ticks",
            backend: Backend::Postgres,
            expected_ticks: 3,
            expected_runs: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            publish_snapshot(store, &input)
                .await
                .expect("publish pending calendar");
            input.generation = 2;
            let future: chrono::DateTime<chrono::Utc> =
                "2031-01-01T00:00:00Z".parse().expect("future");
            input.documents.get_mut("triggers").expect("triggers")["daily_orders"]["schedule"]["times"] = json!(
                (0..10_000)
                    .map(|seconds| future + chrono::TimeDelta::seconds(seconds))
                    .collect::<Vec<_>>()
            );
            assert!(
                publish_snapshot(store, &input)
                    .await
                    .expect_err("pending ticks cannot be truncated to fit a refreshed horizon")
                    .to_string()
                    .contains("let the scheduler drain pending ticks")
            );
            input.documents.get_mut("triggers").expect("triggers")["daily_orders"]["schedule"]["times"] =
                json!(["2031-01-01T00:00:00Z"]);
            publish_snapshot(store, &input)
                .await
                .expect("refresh while scheduler is offline");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("refreshed trigger")
                .remove(0);
            assert_eq!(
                serde_json::to_value(&trigger.definition_json).expect("calendar")["schedule"]["times"],
                json!([
                    "2020-01-01T00:00:00Z",
                    "2020-01-02T00:00:00Z",
                    "2020-01-03T00:00:00Z",
                    "2031-01-01T00:00:00Z"
                ]),
                "{}",
                test_case.description
            );
            assert_eq!(
                trigger.next_due_at,
                Some("2020-01-01T00:00:00Z".parse().expect("due"))
            );
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("election")
                .expect("lease");
            let plan = evaluate_trigger(store, &lease, trigger.trigger_id)
                .await
                .expect("catch up")
                .expect("trigger");
            assert_eq!(plan.actions.len(), test_case.expected_ticks);
            assert_eq!(
                list_ticks(store, trigger.trigger_id, 100)
                    .await
                    .expect("ticks")
                    .len(),
                test_case.expected_ticks
            );
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("coalesced run")
                    .len(),
                test_case.expected_runs
            );
            input.generation = 3;
            input.documents.get_mut("triggers").expect("triggers")["daily_orders"]["schedule"]["times"] =
                json!(["2032-01-01T00:00:00Z"]);
            publish_snapshot(store, &input)
                .await
                .expect("refresh after catch-up");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("drained trigger")
                .remove(0);
            assert_eq!(
                serde_json::to_value(&trigger.definition_json).expect("calendar")["schedule"]["times"],
                json!(["2032-01-01T00:00:00Z"])
            );
            assert_eq!(
                list_ticks(store, trigger.trigger_id, 100)
                    .await
                    .expect("immutable tick history")
                    .len(),
                test_case.expected_ticks
            );
            close_test_store(fixture).await;
        });
    }
}
