//! A republish over a schedule whose stored definition an earlier release left unreadable keeps its timing, operator status and cursor; a calendar's pending times are dropped and reported until its next tick. A skipped schedule's problem is not withdrawn by a tick.

use chrono::{SecondsFormat, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::declarations::main::change_trigger::change_trigger;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::runs::tests::helpers::rows;
use crate::scheduler::models::SchedulerState;
use crate::scheduler::tests::contracts::helpers::{
    dropped_calendar_times, dropped_days_problem, execute, nightly_calendar, nightly_cron,
    operator, scheduler_cycle, skipped_triggers, upgrade_snapshot,
};
use crate::scheduler::tests::contracts::test_types::{
    CoveredTickTestCase, UnreadableScheduleRepublishTestCase,
};
use crate::scheduler::tests::contracts::ticks::helpers::calendar_text;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// The calendar's problem once its job can't be read.
const SKIPPED_JOB: &str = "nightly_orders: this trigger's job was published by an earlier SQLBuild release and can't be read; it runs again after its location publishes again";

#[test]
fn given_a_schedule_an_earlier_release_stored_unreadable_when_its_location_publishes_again_then_its_state_survives()
 {
    let test_cases = [
        UnreadableScheduleRepublishTestCase {
            description: "SQLite store, a paused cron schedule stays paused",
            backend: Backend::Sqlite,
            published: nightly_cron(),
            operator_changes: &[TriggerChange::Pause],
            earlier_form: json!({"schedule": {"kind": "retired"}}),
            republished: nightly_cron(),
            expected_status: ("paused", Some("paused")),
            expected_same_next_due: true,
            expected_problems: [Vec::new(), Vec::new()],
        },
        UnreadableScheduleRepublishTestCase {
            description: "SQLite store, a running cron schedule keeps its next due time",
            backend: Backend::Sqlite,
            published: nightly_cron(),
            operator_changes: &[],
            earlier_form: json!({"schedule": {"kind": "retired"}}),
            republished: nightly_cron(),
            expected_status: ("running", None),
            expected_same_next_due: true,
            expected_problems: [Vec::new(), Vec::new()],
        },
        UnreadableScheduleRepublishTestCase {
            description: "SQLite store, a calendar's pending times are dropped and reported until its next tick",
            backend: Backend::Sqlite,
            published: nightly_calendar(&[0, 1, 2]),
            operator_changes: &[],
            earlier_form: json!({"schedule": {"kind": "calendar", "retired": true,
                "times": [calendar_text(0), calendar_text(1), calendar_text(2),
                    "2030-01-01T00:00:00Z"]}}),
            republished: nightly_calendar(&[3]),
            expected_status: ("running", None),
            expected_same_next_due: false,
            expected_problems: [vec![dropped_days_problem()], Vec::new()],
        },
        UnreadableScheduleRepublishTestCase {
            description: "PostgreSQL store, a paused cron schedule stays paused",
            backend: Backend::Postgres,
            published: nightly_cron(),
            operator_changes: &[TriggerChange::Pause],
            earlier_form: json!({"schedule": {"kind": "retired"}}),
            republished: nightly_cron(),
            expected_status: ("paused", Some("paused")),
            expected_same_next_due: true,
            expected_problems: [Vec::new(), Vec::new()],
        },
        UnreadableScheduleRepublishTestCase {
            description: "PostgreSQL store, a running cron schedule keeps its next due time",
            backend: Backend::Postgres,
            published: nightly_cron(),
            operator_changes: &[],
            earlier_form: json!({"schedule": {"kind": "retired"}}),
            republished: nightly_cron(),
            expected_status: ("running", None),
            expected_same_next_due: true,
            expected_problems: [Vec::new(), Vec::new()],
        },
        UnreadableScheduleRepublishTestCase {
            description: "PostgreSQL store, a calendar's pending times are dropped and reported until its next tick",
            backend: Backend::Postgres,
            published: nightly_calendar(&[0, 1, 2]),
            operator_changes: &[],
            earlier_form: json!({"schedule": {"kind": "calendar", "retired": true,
                "times": [calendar_text(0), calendar_text(1), calendar_text(2),
                    "2030-01-01T00:00:00Z"]}}),
            republished: nightly_calendar(&[3]),
            expected_status: ("running", None),
            expected_same_next_due: false,
            expected_problems: [vec![dropped_days_problem()], Vec::new()],
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let mut input = upgrade_snapshot(roles.location_id, test_case.published.clone());
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("publish");
            let trigger_id: Uuid = serde_json::from_value(
                rows(
                    store,
                    "SELECT trigger_id FROM sqb_triggers WHERE name='nightly_orders'",
                    vec![],
                )
                .await[0]["trigger_id"]
                    .clone(),
            )
            .expect("trigger ID");
            for change in test_case.operator_changes {
                change_trigger(
                    store,
                    &operator(),
                    &TriggerChangeRequest {
                        event_id: Uuid::now_v7(),
                        trigger_id,
                        actor: operator().actor,
                        reason: "Hold it".into(),
                        change: change.clone(),
                    },
                )
                .await
                .expect("an operator change");
            }
            execute(
                store,
                &format!(
                    "UPDATE sqb_triggers SET definition_json='{}' WHERE name='nightly_orders'",
                    test_case.earlier_form
                ),
            )
            .await;
            let state = "SELECT status, status_override, next_due_at FROM sqb_trigger_state";
            let before = rows(store, state, vec![]).await;
            input
                .documents
                .insert("triggers".into(), test_case.republished.clone());
            input.generation = 3;

            publish_snapshot(store, &input)
                .await
                .expect("the location publishes again");

            let after = rows(store, state, vec![]).await;
            let problems = dropped_calendar_times(store, credentials(&roles.administrator)).await;
            scheduler_cycle(store, SchedulerState::new(Uuid::now_v7()))
                .await
                .expect("a scheduler cycle");
            let cleared = dropped_calendar_times(store, credentials(&roles.administrator)).await;
            assert_eq!(
                (
                    (
                        after[0]["status"].as_str().unwrap_or_default(),
                        after[0]["status_override"].as_str()
                    ),
                    before[0]["next_due_at"] == after[0]["next_due_at"],
                    [problems, cleared],
                ),
                (
                    test_case.expected_status,
                    test_case.expected_same_next_due,
                    test_case.expected_problems.clone(),
                ),
                "{}: the operator status and cursor survive the republish, and only dropped calendar times are reported",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_schedule_skipped_for_an_unreadable_job_when_a_tick_is_covered_then_its_problem_stays() {
    let test_cases = [
        CoveredTickTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            expected_skipped: &[SKIPPED_JOB],
        },
        CoveredTickTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            expected_skipped: &[SKIPPED_JOB],
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let mut input = upgrade_snapshot(roles.location_id, nightly_calendar(&[0, 1, 2]));
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("publish");
            execute(
                store,
                "UPDATE sqb_jobs SET definition_json='{\"retired\": true}' WHERE job_key='orders'",
            )
            .await;
            scheduler_cycle(store, SchedulerState::new(Uuid::now_v7()))
                .await
                .expect("a cycle that skips the schedule");
            let skipped = skipped_triggers(store, credentials(&roles.administrator)).await;

            execute(
                store,
                &format!(
                    "INSERT INTO sqb_ticks (tick_id, trigger_id, scheduled_for, evaluated_at, epoch, outcome) \
                     SELECT '{}', trigger_id, '{}', '{}', 1, 'covered' FROM sqb_triggers WHERE name='nightly_orders'",
                    Uuid::now_v7(),
                    calendar_text(3),
                    Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true)
                ),
            )
            .await;

            let names = |lines: &[&str]| {
                lines
                    .iter()
                    .map(|line| (*line).to_owned())
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                (
                    skipped,
                    skipped_triggers(store, credentials(&roles.administrator)).await
                ),
                (
                    names(test_case.expected_skipped),
                    names(test_case.expected_skipped)
                ),
                "{}: a tick covered by a run already queued reads no job, so the problem stays until the location publishes again",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
