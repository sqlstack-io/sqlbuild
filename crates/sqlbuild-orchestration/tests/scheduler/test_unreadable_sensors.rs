//! A sensor whose definition, last check or queued job an earlier release left in a form that no longer decodes is skipped, or checked again, on its own; the scheduler carries on.

use std::time::Duration;

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::runs::tests::helpers::rows;
use crate::scheduler::models::SchedulerState;
use crate::scheduler::tests::contracts::helpers::{
    execute, outcome, scheduler_cycle, skipped_triggers, upgrade_snapshot,
};
use crate::scheduler::tests::contracts::test_types::UnreadableSensorTestCase;
use crate::sensors::tests::helpers::append_events;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// The sensor's name and the health problem a skipped trigger shows.
const SKIPPED: &str = "health_orders: this trigger's job was published by an earlier SQLBuild release and can't be read; it runs again after its location publishes again";
/// The sensor's name and the health problem an unreadable last check shows.
const CHECKED_AGAIN: &str = "health_orders: this sensor's last check was recorded by an earlier SQLBuild release and can't be read; it was checked again as if for the first time";
/// A sensor definition in a shape this release does not read.
const EARLIER_DEFINITION: &str = "UPDATE sqb_triggers SET definition_json='{\"when\": {\"kind\": \"retired\"}}' WHERE kind='sensor'";
/// A last check in a shape this release does not read.
const EARLIER_CHECK: &str =
    "UPDATE sqb_sensor_evaluation_events SET fact_json='{\"request\": {\"retired\": true}}'";
/// A stored job, which the sensor queues, in a shape this release does not read.
const EARLIER_JOB: &str =
    "UPDATE sqb_jobs SET definition_json='{\"retired\": true}' WHERE job_key='orders'";

#[test]
fn given_a_sensor_an_earlier_release_left_unreadable_when_the_scheduler_cycles_then_only_that_sensor_is_affected()
 {
    let test_cases = [
        UnreadableSensorTestCase {
            description: "SQLite store, its published definition",
            backend: Backend::Sqlite,
            earlier_form: EARLIER_DEFINITION,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
        },
        UnreadableSensorTestCase {
            description: "PostgreSQL store, its published definition",
            backend: Backend::Postgres,
            earlier_form: EARLIER_DEFINITION,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
        },
        UnreadableSensorTestCase {
            description: "SQLite store, its last check, which counts as none",
            backend: Backend::Sqlite,
            earlier_form: EARLIER_CHECK,
            expected_cycle: "ok",
            expected_skipped: &[CHECKED_AGAIN],
            expected_sensor_runs: 2,
        },
        UnreadableSensorTestCase {
            description: "PostgreSQL store, its last check, which counts as none",
            backend: Backend::Postgres,
            earlier_form: EARLIER_CHECK,
            expected_cycle: "ok",
            expected_skipped: &[CHECKED_AGAIN],
            expected_sensor_runs: 2,
        },
        UnreadableSensorTestCase {
            description: "SQLite store, the stored job it queues",
            backend: Backend::Sqlite,
            earlier_form: EARLIER_JOB,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
        },
        UnreadableSensorTestCase {
            description: "PostgreSQL store, the stored job it queues",
            backend: Backend::Postgres,
            earlier_form: EARLIER_JOB,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
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
            let mut input = upgrade_snapshot(
                roles.location_id,
                json!({"health_orders": {"description": "Refresh orders on overdue ticks",
                    "job_key": "orders",
                    "when": {"kind": "platform_event", "kinds": ["tick_overdue"]},
                    "minimum_interval_seconds": 1}}),
            );
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("publish");
            append_events(store, 0..1, Utc::now()).await;
            let (state, _) = scheduler_cycle(store, SchedulerState::new(Uuid::now_v7()))
                .await
                .expect("the sensor's first check");
            execute(store, test_case.earlier_form).await;
            tokio::time::sleep(Duration::from_millis(1_100)).await;
            append_events(store, 6..7, Utc::now()).await;
            execute(
                store,
                "UPDATE sqb_trigger_state SET next_due_at='2000-01-01T00:00:00Z' \
                 WHERE trigger_id IN (SELECT trigger_id FROM sqb_triggers WHERE kind='sensor')",
            )
            .await;

            let cycled = scheduler_cycle(store, state).await;

            let sensor_runs = rows(
                store,
                "SELECT COUNT(*) AS runs FROM sqb_runs WHERE origin='sensor'",
                vec![],
            )
            .await[0]["runs"]
                .as_i64();
            assert_eq!(
                (
                    outcome(cycled),
                    skipped_triggers(store, credentials(&roles.administrator)).await,
                    sensor_runs,
                ),
                (
                    test_case.expected_cycle.to_owned(),
                    test_case
                        .expected_skipped
                        .iter()
                        .map(|line| (*line).to_owned())
                        .collect::<Vec<_>>(),
                    Some(test_case.expected_sensor_runs),
                ),
                "{}: the cycle carries on, and the live problems say what happened to the sensor",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
