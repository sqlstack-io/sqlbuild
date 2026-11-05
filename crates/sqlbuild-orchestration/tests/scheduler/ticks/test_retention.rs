//! Expired covered ticks go; a cursor reset replays only the latest occurrence and recreates nothing.

use chrono::{Datelike, DurationRound, TimeDelta, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::{change_trigger::change_trigger, list_triggers::list_triggers};
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::runs::main::list_runs::list_runs;
use crate::scheduler::_helpers::tick_retention::prune_ticks;
use crate::scheduler::main::{
    _acquire_lease::acquire_lease, _evaluate_trigger::evaluate_trigger, list_ticks::list_ticks,
};
use crate::scheduler::models::TickOutcome;
use crate::scheduler::tests::contracts::ticks::helpers::{
    append_covered_tick, calendar_time, shift_schedule, snapshot,
};
use crate::scheduler::tests::contracts::ticks::test_types::{
    LatestCatchUpTestCase, TickRetentionTestCase,
};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_expired_covered_ticks_when_pruned_and_cursor_reset_then_run_linked_and_latest_ticks_survive_without_resurrection()
 {
    let test_cases = [
        TickRetentionTestCase {
            description: "SQLite keeps the requested and latest ticks",
            backend: Backend::Sqlite,
            expected_survivors: 2,
        },
        TickRetentionTestCase {
            description: "PostgreSQL keeps the requested and latest ticks",
            backend: Backend::Postgres,
            expected_survivors: 2,
        },
    ];
    for test_case in test_cases {
        run(async {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/tick_retention.json");
            let case: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("tick fixture"))
                    .expect("tick case");
            let shift = case["shift_days"].as_i64().expect("shift days");
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publication");
            let trigger_id = list_triggers(store, location, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("election")
                .expect("lease");
            evaluate_trigger(store, &lease, trigger_id)
                .await
                .expect("catch up");
            let outcomes: Vec<TickOutcome> = list_ticks(store, trigger_id, 10)
                .await
                .expect("ticks")
                .into_iter()
                .rev()
                .map(|tick| tick.outcome)
                .collect();
            assert_eq!(
                outcomes,
                [TickOutcome::Requested],
                "catch-up records only the latest missed time"
            );
            let later = calendar_time(2) + TimeDelta::hours(1);
            let backlog = case["backlog_ticks"].as_i64().expect("backlog");
            for second in 1..=backlog {
                append_covered_tick(
                    store,
                    trigger_id,
                    calendar_time(2) + TimeDelta::seconds(second),
                )
                .await;
            }
            append_covered_tick(store, trigger_id, later).await;
            assert_eq!(prune_ticks(store, &lease).await.expect("recent ticks"), 0);
            shift_schedule(store, trigger_id, shift).await;
            assert_eq!(
                prune_ticks(store, &lease).await.expect("bounded pass") as u64,
                case["expected_first_pass"].as_u64().expect("first pass"),
                "one pass stops at its batch cap so the lease renews before the rest"
            );
            assert_eq!(
                prune_ticks(store, &lease).await.expect("prune") as u64,
                case["expected_pruned"].as_u64().expect("pruned")
            );
            assert_eq!(prune_ticks(store, &lease).await.expect("idempotent"), 0);
            let survivors = list_ticks(store, trigger_id, 10).await.expect("survivors");
            assert_eq!(
                survivors.len(),
                test_case.expected_survivors,
                "{}",
                test_case.description
            );
            assert_eq!(
                survivors
                    .iter()
                    .map(|tick| (tick.scheduled_for, tick.outcome))
                    .collect::<Vec<_>>(),
                [
                    (later - TimeDelta::days(shift), TickOutcome::Covered),
                    (
                        calendar_time(2) - TimeDelta::days(shift),
                        TickOutcome::Requested
                    ),
                ],
                "latest and run-linked ticks survive"
            );
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
                    trigger_id,
                    actor: audit.actor.clone(),
                    reason: "Replay the retained calendar".into(),
                    change: TriggerChange::ResetCursor,
                },
            )
            .await
            .expect("reset");
            let plan = evaluate_trigger(store, &lease, trigger_id)
                .await
                .expect("re-evaluate")
                .expect("trigger");
            let replayed = calendar_time(2) - TimeDelta::days(shift);
            assert_eq!(
                plan.actions
                    .iter()
                    .map(|action| action.scheduled_for)
                    .collect::<Vec<_>>(),
                [replayed],
                "a reset replays only the latest occurrence"
            );
            let ticks = list_ticks(store, trigger_id, 10)
                .await
                .expect("after reset");
            assert_eq!(
                ticks.len(),
                test_case.expected_survivors,
                "the replayed tick already exists, so nothing is recreated"
            );
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("trigger")
                .remove(0);
            assert_eq!(trigger.last_tick_for, Some(replayed));
            assert_eq!(
                trigger.next_due_at,
                Some("2030-01-01T00:00:00Z".parse().expect("future"))
            );
            assert!(ticks.iter().all(|tick| tick.scheduled_for < Utc::now()));
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_an_old_yearly_schedule_when_first_evaluated_then_latest_catch_up_ticks_only_its_latest_occurrence()
 {
    let test_cases = [
        LatestCatchUpTestCase {
            description: "SQLite latest catch-up ticks an occurrence older than the horizon",
            backend: Backend::Sqlite,
            expected_ticks: 1,
        },
        LatestCatchUpTestCase {
            description: "PostgreSQL latest catch-up ticks an occurrence older than the horizon",
            backend: Backend::Postgres,
            expected_ticks: 1,
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
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../benchmarks/duckshop/edge_cases/orchestration/tick_retention.json");
            let case: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).expect("tick fixture"))
                    .expect("tick case");
            // A yearly date (day 28 at most) well past the horizon, so its latest occurrence is older than it.
            let days_ago = case["yearly_latest_days_ago"].as_i64().expect("days ago");
            let past = Utc::now() - TimeDelta::days(days_ago);
            let latest = past
                .with_day(past.day().min(28))
                .expect("day every year has")
                .duration_trunc(TimeDelta::days(1))
                .expect("midnight");
            let mut input = snapshot(location);
            input.documents.insert(
                "triggers".into(),
                json!({"yearly_orders": {"description": "Yearly orders", "job_key": "orders",
                    "starts_at": case["yearly_starts_at"],
                    "schedule": {"kind": "cron", "expression": format!("0 0 {} {} *", latest.day(), latest.month()), "timezone": "UTC"}}}),
            );
            publish_snapshot(store, &input).await.expect("publication");
            let trigger_id = list_triggers(store, location, "development")
                .await
                .expect("trigger")[0]
                .trigger_id;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("election")
                .expect("lease");
            for _ in 0..2 {
                evaluate_trigger(store, &lease, trigger_id)
                    .await
                    .expect("catch up")
                    .expect("trigger");
            }
            let ticks: Vec<_> = list_ticks(store, trigger_id, 10)
                .await
                .expect("ticks")
                .into_iter()
                .map(|tick| (tick.scheduled_for, tick.outcome))
                .collect();
            assert_eq!(
                ticks.len(),
                test_case.expected_ticks,
                "{}",
                test_case.description
            );
            assert!(
                ticks
                    .iter()
                    .all(|tick| *tick == (latest, TickOutcome::Requested)),
                "only the latest occurrence is ticked: {}",
                test_case.description
            );
            let runs = list_runs(store, (location, "development"), 10)
                .await
                .expect("runs");
            assert_eq!(
                runs.len(),
                test_case.expected_ticks,
                "{}",
                test_case.description
            );
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("trigger")
                .remove(0);
            assert_eq!(
                trigger.next_due_at,
                Some(latest.with_year(latest.year() + 1).expect("next year")),
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
