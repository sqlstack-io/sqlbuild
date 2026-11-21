//! A sensor whose definition, last check or queued job an earlier release left in a form that no longer decodes is skipped, or checked again from scratch, on its own; the scheduler carries on.

use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::runs::tests::helpers::rows;
use crate::scheduler::tests::contracts::helpers::{
    answer_sensor_work, make_sensors_due, outcome, scheduler_cycle, sensor_due_again,
    trigger_problems,
};
use crate::scheduler::tests::contracts::test_types::{
    SensorRecheckTestCase, UnreadableSensorTestCase,
};
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::tests::helpers::complete_starting_job;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// The sensor's name and the health problem a skipped trigger shows.
const SKIPPED: &str = "health_orders: this trigger's job was published by an earlier SQLBuild release and can't be read; it runs again after its location publishes again";
/// A sensor definition in a shape this release does not read.
const EARLIER_DEFINITION: &str = "UPDATE sqb_triggers SET definition_json='{\"when\": {\"kind\": \"retired\"}}' WHERE kind='sensor'";
/// A last check in a shape this release does not read.
const EARLIER_CHECK: &str =
    "UPDATE sqb_sensor_evaluation_events SET fact_json='{\"request\": {\"retired\": true}}'";
/// A stored job, which the sensor queues, in a shape this release does not read.
const EARLIER_JOB: &str =
    "UPDATE sqb_jobs SET definition_json='{\"retired\": true}' WHERE job_key='orders'";
/// The `sensor_rechecked` events recorded so far.
const RECHECKED_EVENTS: &str =
    "SELECT COUNT(*) AS count FROM recall_events WHERE event_type='platform_sensor_rechecked'";

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
            expected_rechecked_events: 0,
        },
        UnreadableSensorTestCase {
            description: "PostgreSQL store, its published definition",
            backend: Backend::Postgres,
            earlier_form: EARLIER_DEFINITION,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
            expected_rechecked_events: 0,
        },
        UnreadableSensorTestCase {
            description: "SQLite store, its last check, which counts as none and is not a skip",
            backend: Backend::Sqlite,
            earlier_form: EARLIER_CHECK,
            expected_cycle: "ok",
            expected_skipped: &[],
            expected_sensor_runs: 2,
            expected_rechecked_events: 1,
        },
        UnreadableSensorTestCase {
            description: "PostgreSQL store, its last check, which counts as none and is not a skip",
            backend: Backend::Postgres,
            earlier_form: EARLIER_CHECK,
            expected_cycle: "ok",
            expected_skipped: &[],
            expected_sensor_runs: 2,
            expected_rechecked_events: 1,
        },
        UnreadableSensorTestCase {
            description: "SQLite store, the stored job it queues",
            backend: Backend::Sqlite,
            earlier_form: EARLIER_JOB,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
            expected_rechecked_events: 0,
        },
        UnreadableSensorTestCase {
            description: "PostgreSQL store, the stored job it queues",
            backend: Backend::Postgres,
            earlier_form: EARLIER_JOB,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
            expected_rechecked_events: 0,
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
            let (state, _) =
                sensor_due_again(store, roles.location_id, test_case.earlier_form).await;

            let cycled = match scheduler_cycle(store, state).await {
                Ok((state, _)) => {
                    answer_sensor_work(store).await;
                    make_sensors_due(store).await;
                    scheduler_cycle(store, state).await
                }
                failed => failed,
            };

            let count = |sql: &'static str| async move {
                rows(store, sql, vec![]).await[0]["count"].as_i64()
            };
            assert_eq!(
                (
                    outcome(cycled),
                    trigger_problems(store, credentials(&roles.administrator)).await,
                    count("SELECT COUNT(*) AS count FROM sqb_runs WHERE origin='sensor'").await,
                    count(RECHECKED_EVENTS).await,
                ),
                (
                    test_case.expected_cycle.to_owned(),
                    (
                        test_case
                            .expected_skipped
                            .iter()
                            .map(|line| (*line).to_owned())
                            .collect::<Vec<_>>(),
                        Vec::new(),
                    ),
                    Some(test_case.expected_sensor_runs),
                    Some(test_case.expected_rechecked_events),
                ),
                "{}: the cycle carries on, and the live problems say what happened to the sensor",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_sensor_whose_last_check_is_unreadable_when_it_is_checked_again_then_its_problem_lasts_until_that_check_completes()
 {
    let test_cases = [
        SensorRecheckTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            expected_while_checking: (&[], &["health_orders"]),
            expected_after: &[],
        },
        SensorRecheckTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            expected_while_checking: (&[], &["health_orders"]),
            expected_after: &[],
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
            let (state, trigger_id) =
                sensor_due_again(store, roles.location_id, EARLIER_CHECK).await;
            let lease = state.lease.clone().expect("leader");
            let request = begin_sensor_evaluation(store, &lease, (trigger_id, None))
                .await
                .expect("begin the re-check")
                .expect("the sensor is due");
            let while_checking = trigger_problems(store, credentials(&roles.administrator)).await;
            complete_starting_job(store, &lease, &request)
                .await
                .expect("complete the re-check");
            let names = |lines: &[&str]| {
                lines
                    .iter()
                    .map(|line| (*line).to_owned())
                    .collect::<Vec<_>>()
            };

            assert_eq!(
                (
                    while_checking,
                    trigger_problems(store, credentials(&roles.administrator))
                        .await
                        .1,
                ),
                (
                    (
                        names(test_case.expected_while_checking.0),
                        names(test_case.expected_while_checking.1)
                    ),
                    names(test_case.expected_after),
                ),
                "{}: a re-check from scratch is its own problem, not a skip, and withdraws once it completes",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
