//! A sensor whose definition, last check or queued job an earlier release left in a form that no longer decodes, or whose check fails, is skipped, or checked again from scratch, on its own; the scheduler and the other sensors carry on.

use crate::credentials::models::AuditContext;
use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::declarations::main::change_trigger::change_trigger;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::runs::tests::helpers::rows;
use crate::scheduler::_helpers::unreadable_trigger::skip_failed_sensor;
use crate::scheduler::tests::contracts::helpers::{
    answer_sensor_work, execute_with, make_sensors_due, outcome, scheduler_cycle, sensor_due_again,
    trigger_problems,
};
use crate::scheduler::tests::contracts::test_types::{
    FailedAttemptsTestCase, FailedSensorTestCase, SensorRecheckTestCase, UnreadableSensorTestCase,
    UnreadableSkipTestCase,
};
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::tests::helpers::complete_starting_job;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

/// The sensor's name and the health problem a skipped trigger shows.
const SKIPPED: &str = "health_orders: this trigger's job was published by an earlier SQLBuild release and can't be read; it runs again after its location publishes again";
/// A sensor definition in a shape this release does not read.
const EARLIER_DEFINITION: &str = "UPDATE sqb_triggers SET definition_json='{\"when\": {\"kind\": \"retired\"}}' WHERE kind='sensor' AND name='health_orders'";
/// A last check in a shape this release does not read.
const EARLIER_CHECK: &str = "UPDATE sqb_sensor_evaluation_events SET fact_json='{\"request\": {\"retired\": true}}' \
     WHERE trigger_id IN (SELECT trigger_id FROM sqb_triggers WHERE name='health_orders')";
/// A stored job, which the sensor queues, in a shape this release does not read.
const EARLIER_JOB: &str =
    "UPDATE sqb_jobs SET definition_json='{\"retired\": true}' WHERE job_key='orders'";
/// The checks the healthy sensor `watch_overdue` has completed so far.
const HEALTHY_CHECKS: &str = "SELECT COUNT(*) AS count FROM sqb_sensor_evaluation_events e \
     JOIN sqb_triggers t ON t.trigger_id=e.trigger_id WHERE t.name='watch_overdue' AND e.phase='completed'";
/// The health problem a sensor shows when its location work is gone.
const FAILED: &str = "health_orders: this sensor's check failed and was skipped; it is checked again in a minute: control-plane data this release can't read or accept";
/// A denied check's problem, which names no decision.
const DENIED: &str = "health_orders: this sensor's check failed and was skipped; it is checked again in a minute: a control-plane operation was denied";
/// The `trigger_skipped` events recorded so far.
const SKIPPED_EVENTS: &str =
    "SELECT COUNT(*) AS count FROM recall_events WHERE event_type='platform_trigger_skipped'";
/// The skip refusal while the sensor runs: its next tick checks it again.
const RUNNING_REFUSAL: &str = "invalid control-plane data: the sensor's newest tick can't be read; its next tick checks it again from scratch";
/// The skip refusal while the sensor is paused: no next tick comes.
const PAUSED_REFUSAL: &str = "invalid control-plane data: the sensor's newest tick can't be read, and the sensor is paused, so no next tick checks it again; resume the sensor, or reset its cursor";
/// What a skipped sensor's server log line names, the full error included.
const LOGGED: &[&str] = &[
    "sensor health_orders in location orders, target development, was skipped: its check failed",
    "control-plane operation denied: the sensor's role can't queue job orders",
];
/// The `trigger_skipped` events' IDs, which start each episode.
const SKIPPED_EVENT_IDS: &str =
    "SELECT event_id FROM recall_events WHERE event_type='platform_trigger_skipped'";
/// Leaves nothing behind, for a sensor that stays readable.
const NOTHING: &str = "SELECT 1";
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
            expected_healthy_checks: 2,
        },
        UnreadableSensorTestCase {
            description: "PostgreSQL store, its published definition",
            backend: Backend::Postgres,
            earlier_form: EARLIER_DEFINITION,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
            expected_rechecked_events: 0,
            expected_healthy_checks: 2,
        },
        UnreadableSensorTestCase {
            description: "SQLite store, its last check, which counts as none and is not a skip",
            backend: Backend::Sqlite,
            earlier_form: EARLIER_CHECK,
            expected_cycle: "ok",
            expected_skipped: &[],
            expected_sensor_runs: 2,
            expected_rechecked_events: 1,
            expected_healthy_checks: 2,
        },
        UnreadableSensorTestCase {
            description: "PostgreSQL store, its last check, which counts as none and is not a skip",
            backend: Backend::Postgres,
            earlier_form: EARLIER_CHECK,
            expected_cycle: "ok",
            expected_skipped: &[],
            expected_sensor_runs: 2,
            expected_rechecked_events: 1,
            expected_healthy_checks: 2,
        },
        UnreadableSensorTestCase {
            description: "SQLite store, the stored job it queues",
            backend: Backend::Sqlite,
            earlier_form: EARLIER_JOB,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
            expected_rechecked_events: 0,
            expected_healthy_checks: 2,
        },
        UnreadableSensorTestCase {
            description: "PostgreSQL store, the stored job it queues",
            backend: Backend::Postgres,
            earlier_form: EARLIER_JOB,
            expected_cycle: "ok",
            expected_skipped: &[SKIPPED],
            expected_sensor_runs: 1,
            expected_rechecked_events: 0,
            expected_healthy_checks: 2,
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

            let (state, _) = scheduler_cycle(store, state)
                .await
                .expect("the cycle that meets the unreadable sensor carries on");
            answer_sensor_work(store).await;
            make_sensors_due(store).await;
            let cycled = scheduler_cycle(store, state).await;

            let count = |sql: &'static str| async move {
                rows(store, sql, vec![]).await[0]["count"].as_i64()
            };
            assert_eq!(
                (
                    outcome(cycled),
                    trigger_problems(store, credentials(&roles.administrator)).await,
                    count("SELECT COUNT(*) AS count FROM sqb_runs WHERE origin='sensor'").await,
                    count(RECHECKED_EVENTS).await,
                    count(HEALTHY_CHECKS).await,
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
                    Some(test_case.expected_healthy_checks),
                ),
                "{}: the cycle carries on, the healthy sensor still ticks, and the live problems say what happened to the sensor",
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

#[test]
fn given_a_sensor_whose_check_fails_when_the_scheduler_cycles_then_only_that_sensor_is_skipped_until_a_check_completes()
 {
    let test_cases = [
        FailedSensorTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            expected_skipped: &[FAILED],
            expected_healthy_checks: 2,
            expected_skipped_after: &[],
        },
        FailedSensorTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            expected_skipped: &[FAILED],
            expected_healthy_checks: 2,
            expected_skipped_after: &[],
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
            let (state, trigger_id) = sensor_due_again(store, roles.location_id, NOTHING).await;
            let (state, _) = scheduler_cycle(store, state)
                .await
                .expect("both sensors check");
            answer_sensor_work(store).await;
            let pending: uuid::Uuid = serde_json::from_value(
                rows(
                    store,
                    "SELECT evaluation_id FROM sqb_sensor_evaluation_events WHERE trigger_id=$1 \
                     AND phase='requested' AND evaluation_id NOT IN (SELECT evaluation_id \
                     FROM sqb_sensor_evaluation_events WHERE phase<>'requested')",
                    vec![Parameter::Id(trigger_id)],
                )
                .await[0]["evaluation_id"]
                    .clone(),
            )
            .expect("the check still out");
            let hidden = uuid::Uuid::now_v7();
            let move_work = |from: uuid::Uuid, to: uuid::Uuid| async move {
                execute_with(
                    store,
                    "UPDATE sqb_location_work SET request_id=$2 WHERE request_id=$1",
                    vec![Parameter::Id(from), Parameter::Id(to)],
                )
                .await;
            };
            move_work(pending, hidden).await;
            make_sensors_due(store).await;

            let (state, _) = scheduler_cycle(store, state)
                .await
                .expect("the cycle in which the check fails carries on");
            let skipped = trigger_problems(store, credentials(&roles.administrator))
                .await
                .0;
            let healthy = rows(store, HEALTHY_CHECKS, vec![]).await[0]["count"].as_i64();
            move_work(hidden, pending).await;
            execute_with(
                store,
                "UPDATE recall_events SET occurred_at=$1 WHERE event_type='platform_trigger_skipped'",
                vec![Parameter::Time(
                    chrono::Utc::now() - chrono::TimeDelta::minutes(2),
                )],
            )
            .await;
            make_sensors_due(store).await;
            scheduler_cycle(store, state)
                .await
                .expect("the failed check completes");
            let after = trigger_problems(store, credentials(&roles.administrator))
                .await
                .0;
            let names = |lines: &[&str]| {
                lines
                    .iter()
                    .map(|line| (*line).to_owned())
                    .collect::<Vec<_>>()
            };

            assert_eq!(
                (skipped, healthy, after),
                (
                    names(test_case.expected_skipped),
                    Some(test_case.expected_healthy_checks),
                    names(test_case.expected_skipped_after),
                ),
                "{}: the failing sensor is skipped with its error, the healthy one still ticks, and the problem clears once the check completes",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_sensor_whose_newest_tick_is_unreadable_when_an_operator_skips_its_event_then_the_refusal_says_what_unsticks_it()
 {
    let test_cases = [
        UnreadableSkipTestCase {
            description: "SQLite store, a running sensor",
            backend: Backend::Sqlite,
            before: &[],
            expected_refusal: RUNNING_REFUSAL,
        },
        UnreadableSkipTestCase {
            description: "PostgreSQL store, a running sensor",
            backend: Backend::Postgres,
            before: &[],
            expected_refusal: RUNNING_REFUSAL,
        },
        UnreadableSkipTestCase {
            description: "SQLite store, a paused sensor",
            backend: Backend::Sqlite,
            before: &[TriggerChange::Pause],
            expected_refusal: PAUSED_REFUSAL,
        },
        UnreadableSkipTestCase {
            description: "PostgreSQL store, a paused sensor",
            backend: Backend::Postgres,
            before: &[TriggerChange::Pause],
            expected_refusal: PAUSED_REFUSAL,
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
            let (_, trigger_id) = sensor_due_again(store, roles.location_id, EARLIER_CHECK).await;
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: uuid::Uuid::now_v7(),
                ip_address: None,
            };
            let request = |change: TriggerChange| TriggerChangeRequest {
                event_id: uuid::Uuid::now_v7(),
                trigger_id,
                actor: "test-admin".into(),
                reason: "The partner re-sent this file by hand".into(),
                change,
            };
            for change in test_case.before {
                change_trigger(store, &context, &request(change.clone()))
                    .await
                    .expect("change before the skip");
            }

            let refused = change_trigger(
                store,
                &context,
                &request(TriggerChange::SkipEvent {
                    event: "platform:00000000-0000-0000-0000-000000000006".into(),
                }),
            )
            .await;

            assert_eq!(
                refused.map(|_| ()).map_err(|error| error.to_string()),
                Err(test_case.expected_refusal.to_owned()),
                "{}: the refusal says what unsticks the sensor",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_sensor_failing_with_a_different_id_each_attempt_when_it_is_skipped_then_one_problem_names_the_kind()
 {
    let test_cases = [
        FailedAttemptsTestCase {
            description: "SQLite store",
            backend: Backend::Sqlite,
            expected_events: 1,
            expected_skipped: &[DENIED],
            expected_logged: LOGGED,
            expected_logged_again: None,
        },
        FailedAttemptsTestCase {
            description: "PostgreSQL store",
            backend: Backend::Postgres,
            expected_events: 1,
            expected_skipped: &[DENIED],
            expected_logged: LOGGED,
            expected_logged_again: None,
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
            let (state, trigger_id) = sensor_due_again(store, roles.location_id, NOTHING).await;
            let lease = state.lease.clone().expect("leader");
            let denied = || ControlError::Denied {
                reason: "the sensor's role can't queue job orders".into(),
                decision_id: uuid::Uuid::now_v7(),
            };

            let first = skip_failed_sensor(store, &lease, trigger_id, &denied())
                .await
                .expect("first skip")
                .unwrap_or_default();
            let again = skip_failed_sensor(store, &lease, trigger_id, &denied())
                .await
                .expect("second skip");
            let episodes = rows(store, SKIPPED_EVENT_IDS, vec![]).await;
            let episode = episodes[0]["event_id"].as_str().unwrap_or_default();

            assert_eq!(
                (
                    rows(store, SKIPPED_EVENTS, vec![]).await[0]["count"].as_i64(),
                    trigger_problems(store, credentials(&roles.administrator))
                        .await
                        .0,
                ),
                (
                    Some(test_case.expected_events),
                    test_case
                        .expected_skipped
                        .iter()
                        .map(|line| (*line).to_owned())
                        .collect::<Vec<_>>(),
                ),
                "{}: attempts that differ only in their IDs are one problem, which names the error's kind",
                test_case.description
            );
            assert_eq!(
                (
                    test_case
                        .expected_logged
                        .iter()
                        .filter(|part| !first.contains(**part))
                        .collect::<Vec<_>>(),
                    first.contains(&format!("problem episode {episode}")),
                    again.as_deref(),
                    DENIED.contains("can't queue job orders"),
                ),
                (
                    Vec::<&&str>::new(),
                    true,
                    test_case.expected_logged_again,
                    false
                ),
                "{}: the server log has the full error once per episode, and the problem never does: {first}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
