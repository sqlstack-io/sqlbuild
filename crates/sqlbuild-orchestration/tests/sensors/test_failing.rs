//! A sensor is failing from the first failed tick after its last good one, however many follow.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::credentials::models::DefaultRole;
use crate::credentials::tests::role_helpers::{credentials, role_fixture};
use crate::declarations::main::list_triggers::list_triggers;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::read_person_sensor::read_person_sensor;
use crate::sensors::main::sensor_failing::sensor_failing;
use crate::sensors::models::{AbandonReason, SensorEvaluationOutcome};
use crate::sensors::tests::helpers::{append_events, record_tick, snapshot};
use crate::sensors::tests::test_types::{FailingTick, SensorFailingTestCase};
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::{begin::begin, commit::commit, migrate_store::migrate_store};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_long_failing_streak_when_reading_whether_a_sensor_fails_then_since_is_its_first_failure_and_only_unanswered_abandonment_counts()
 {
    let test_cases = [
        SensorFailingTestCase {
            description: "SQLite: 205 failures, then the location never answers",
            backend: Backend::Sqlite,
            ticks: &[
                FailingTick::Failed(205),
                FailingTick::Abandoned(AbandonReason::Unanswered),
            ],
            expected_failed_ticks: Some(206),
            expected_reason: Some("the location didn't answer within 60s"),
            expected_abandoned_because: Some(AbandonReason::Unanswered),
        },
        SensorFailingTestCase {
            description: "PostgreSQL: a pause and a republish while failing raise nothing more",
            backend: Backend::Postgres,
            ticks: &[
                FailingTick::Failed(3),
                FailingTick::Abandoned(AbandonReason::Paused),
                FailingTick::Abandoned(AbandonReason::Republished),
            ],
            expected_failed_ticks: Some(3),
            expected_reason: None,
            expected_abandoned_because: Some(AbandonReason::Republished),
        },
        SensorFailingTestCase {
            description: "SQLite: a good tick ends the streak",
            backend: Backend::Sqlite,
            ticks: &[FailingTick::Failed(2), FailingTick::Completed],
            expected_failed_ticks: None,
            expected_reason: None,
            expected_abandoned_because: None,
        },
        SensorFailingTestCase {
            description: "PostgreSQL: a reset abandonment alone is not failing",
            backend: Backend::Postgres,
            ticks: &[FailingTick::Abandoned(AbandonReason::CursorReset)],
            expected_failed_ticks: None,
            expected_reason: None,
            expected_abandoned_because: Some(AbandonReason::CursorReset),
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let location = roles.location_id;
            let mut input = snapshot(location);
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("publish");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            append_events(store, 0..1, chrono::Utc::now()).await;
            let first = begin_sensor_evaluation(store, &lease, (trigger.trigger_id, None))
                .await
                .expect("request")
                .expect("due");
            let good = record_tick(store, &first, TimeDelta::zero(), completed()).await;
            let mut offset = 1;
            let mut first_failure = None;
            for tick in test_case.ticks {
                let (count, outcome) = match tick {
                    FailingTick::Failed(count) => (*count, failed()),
                    FailingTick::Completed => (1, completed()),
                    FailingTick::Abandoned(reason) => {
                        (1, SensorEvaluationOutcome::Abandoned { reason: *reason })
                    }
                };
                for _ in 0..count {
                    let request =
                        record_tick(store, &good, TimeDelta::seconds(offset), outcome.clone())
                            .await;
                    first_failure.get_or_insert(request.requested_at);
                    offset += 1;
                }
            }
            let (transaction, failing) = sensor_failing(
                begin(store, TransactionMode::Read).await.expect("read"),
                trigger.trigger_id,
            )
            .await
            .expect("failing");
            commit(transaction).await.expect("commit");
            assert_eq!(
                failing.as_ref().map(|failing| failing.failed_ticks),
                test_case.expected_failed_ticks,
                "{}",
                test_case.description
            );
            assert_eq!(
                failing
                    .as_ref()
                    .and_then(|failing| failing.reason.as_deref()),
                test_case.expected_reason,
                "{}",
                test_case.description
            );
            assert!(
                failing.is_none() || failing.map(|failing| failing.since) == first_failure,
                "{}: since stays at the first failure",
                test_case.description
            );
            let state = read_person_sensor(
                store,
                (&roles.context, credentials(&roles.administrator)),
                ((location, "development"), (trigger.trigger_id, 1)),
                DefaultRole::None,
            )
            .await
            .expect("person read")
            .expect("sensor");
            assert_eq!(
                state.evaluations[0].abandoned_because, test_case.expected_abandoned_because,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

fn completed() -> SensorEvaluationOutcome {
    SensorEvaluationOutcome::Completed {
        cursor: Default::default(),
        run_ids: Vec::new(),
        skipped: Vec::new(),
        skip_reason: None,
        skipped_by: None,
    }
}

fn failed() -> SensorEvaluationOutcome {
    SensorEvaluationOutcome::Failed {
        code: "sensor_function_failed".into(),
        error: Some("ValueError: orders feed is down".into()),
    }
}
