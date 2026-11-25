//! The event a sensor is stuck on can be skipped while a tick for it is still in flight, as a timing-out or paused sensor's usually is.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::change_trigger::change_trigger;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::complete_sensor::complete_sensor;
use crate::sensors::models::{AbandonReason, SensorEvaluationOutcome};
use crate::sensors::tests::helpers::{
    answer_sensor, append_events, change, complete_starting_job, evaluations, snapshot,
    stand_in_outcome,
};
use crate::sensors::tests::test_types::SkipInFlightTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_tick_in_flight_on_the_stuck_event_when_an_operator_skips_it_then_the_late_result_is_abandoned_and_the_next_event_runs()
 {
    let test_cases = [
        SkipInFlightTestCase {
            description: "SQLite: a running sensor whose tick is still out",
            backend: Backend::Sqlite,
            before_skip: &[],
            after_skip: &[],
            expected_next_event: "platform:00000000-0000-0000-0000-0000000003ea",
        },
        SkipInFlightTestCase {
            description: "PostgreSQL: a paused sensor, whose tick never finishes",
            backend: Backend::Postgres,
            before_skip: &[TriggerChange::Pause],
            after_skip: &[TriggerChange::Enable],
            expected_next_event: "platform:00000000-0000-0000-0000-0000000003ea",
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
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            append_events(store, 0..3, chrono::Utc::now()).await;
            let in_flight = begin_sensor_evaluation(store, &lease, (trigger.trigger_id, None))
                .await
                .expect("request")
                .expect("due");
            let stuck = in_flight
                .event
                .as_ref()
                .map(|event| event.id().to_owned())
                .expect("an event");
            for before in test_case.before_skip {
                change(store, trigger.trigger_id, before.clone()).await;
            }
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let skip = |event: &str| TriggerChangeRequest {
                event_id: Uuid::now_v7(),
                trigger_id: trigger.trigger_id,
                actor: "test-admin".into(),
                reason: "The feed is down; move on".into(),
                change: TriggerChange::SkipEvent {
                    event: event.into(),
                },
            };
            let refused =
                change_trigger(store, &context, &skip(test_case.expected_next_event)).await;
            let applied = change_trigger(store, &context, &skip(&stuck))
                .await
                .expect("skip while in flight");
            answer_sensor(store, &in_flight, stand_in_outcome(&in_flight)).await;
            let late = complete_sensor(store, &lease, &in_flight)
                .await
                .unwrap_or_default();
            for after in test_case.after_skip {
                change(store, trigger.trigger_id, after.clone()).await;
            }
            let next = begin_sensor_evaluation(store, &lease, (trigger.trigger_id, None))
                .await
                .expect("request next")
                .expect("due at once");
            let started = complete_starting_job(store, &lease, &next)
                .await
                .expect("next");
            let abandoned: Vec<SensorEvaluationOutcome> = evaluations(store)
                .await
                .into_iter()
                .filter(|fact| fact.request.evaluation_id == in_flight.evaluation_id)
                .map(|fact| fact.outcome)
                .collect();
            let (transaction, runs) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT run_id FROM sqb_runs WHERE origin='sensor'",
                Vec::new(),
            )
            .await
            .expect("runs");
            commit(transaction).await.expect("commit");
            assert_eq!(
                (
                    refused.is_err(),
                    applied,
                    late,
                    abandoned,
                    next.event
                        .as_ref()
                        .map(|event| event.id().to_owned())
                        .as_deref(),
                    started,
                    runs.len(),
                ),
                (
                    true,
                    true,
                    0,
                    vec![
                        SensorEvaluationOutcome::Requested,
                        SensorEvaluationOutcome::Abandoned {
                            reason: AbandonReason::Skipped
                        }
                    ],
                    Some(test_case.expected_next_event),
                    1,
                    1,
                ),
                "{}: only the stuck event can be skipped; the late answer starts nothing and the next event runs",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
