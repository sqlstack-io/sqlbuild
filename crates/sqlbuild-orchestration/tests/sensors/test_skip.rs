//! An operator skips the event a sensor is stuck on; later events wait behind it until then, and a reset replays every event.

use uuid::Uuid;

use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{AuditContext, BuiltinRole, DefaultRole, RoleAssignmentRequest};
use crate::credentials::tests::role_helpers::{assign, credentials, role_fixture};
use crate::declarations::main::change_trigger::change_trigger;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::complete_sensor::complete_sensor;
use crate::sensors::main::read_person_sensor::read_person_sensor;
use crate::sensors::models::SensorFunctionOutcome;
use crate::sensors::tests::helpers::{
    answer_sensor, append_events, change, complete_starting_job, snapshot,
};
use crate::sensors::tests::test_types::SkipEventTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_sensor_stuck_on_a_failing_event_when_an_operator_skips_it_then_the_next_event_runs_and_the_skip_is_recorded()
 {
    let test_cases = [
        SkipEventTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_next_event: "platform:00000000-0000-0000-0000-0000000003ea",
        },
        SkipEventTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
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
            let roles = role_fixture(store).await;
            let mut input = snapshot(roles.location_id);
            input.generation = 2;
            publish_snapshot(store, &input).await.expect("publish");
            let trigger = list_triggers(store, roles.location_id, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            append_events(store, 0..3, chrono::Utc::now()).await;
            let mut stuck_on = Vec::new();
            for _ in 0..2 {
                let tick = begin_sensor_evaluation(store, &lease, (trigger.trigger_id, None))
                    .await
                    .expect("request")
                    .expect("due");
                stuck_on.push(tick.event.as_ref().map(|event| event.id().to_owned()));
                answer_sensor(
                    store,
                    &tick,
                    SensorFunctionOutcome::Failed {
                        error: "ValueError: orders feed is down".into(),
                    },
                )
                .await;
                complete_sensor(store, &lease, &tick).await.expect("fail");
                tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
            }
            let stuck = stuck_on[0].clone().expect("an event");
            assert_eq!(
                stuck_on[1].as_ref(),
                Some(&stuck),
                "{}: a later event waits behind the failed one",
                test_case.description
            );

            let viewer = create_principal(store, &roles.context, "orders-viewer")
                .await
                .expect("viewer");
            assign(
                store,
                (&roles.context, &roles.administrator),
                RoleAssignmentRequest {
                    principal_id: viewer,
                    role: BuiltinRole::Viewer.into(),
                    location_id: None,
                    target: None,
                    revoke: false,
                },
            )
            .await;
            let mut tracebacks = Vec::new();
            for who in [
                (&roles.context, credentials(&roles.administrator)),
                (&roles.context, viewer),
            ] {
                let failed = read_person_sensor(
                    store,
                    who,
                    ((roles.location_id, "development"), (trigger.trigger_id, 1)),
                    DefaultRole::None,
                )
                .await
                .expect("person read")
                .expect("sensor")
                .evaluations
                .remove(0);
                tracebacks.push((failed.failure_code, failed.error));
            }
            assert_eq!(
                tracebacks,
                vec![
                    (
                        Some("sensor_function_failed".to_owned()),
                        Some("ValueError: orders feed is down".to_owned())
                    ),
                    (Some("sensor_function_failed".to_owned()), None),
                ],
                "{}: the traceback needs view.evidence; the failure code doesn't",
                test_case.description
            );

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
                reason: "The partner re-sent this file by hand".into(),
                change: TriggerChange::SkipEvent {
                    event: event.into(),
                },
            };
            let refused =
                change_trigger(store, &context, &skip(test_case.expected_next_event)).await;
            let request = skip(&stuck);
            let (applied, repeated) = (
                change_trigger(store, &context, &request)
                    .await
                    .expect("skip"),
                change_trigger(store, &context, &request)
                    .await
                    .expect("repeat"),
            );
            let next = begin_sensor_evaluation(store, &lease, (trigger.trigger_id, None))
                .await
                .expect("request next")
                .expect("due at once");
            let started = complete_starting_job(store, &lease, &next)
                .await
                .expect("next");
            assert_eq!(
                (
                    refused.is_err(),
                    applied,
                    repeated,
                    next.event
                        .as_ref()
                        .map(|event| event.id().to_owned())
                        .as_deref(),
                    started
                ),
                (true, true, false, Some(test_case.expected_next_event), 1),
                "{}: only the stuck event can be skipped, once, and the next event then runs",
                test_case.description
            );

            let state = read_person_sensor(
                store,
                (&roles.context, credentials(&roles.administrator)),
                ((roles.location_id, "development"), (trigger.trigger_id, 10)),
                DefaultRole::None,
            )
            .await
            .expect("person read")
            .expect("sensor");
            let skipped: Vec<_> = state
                .evaluations
                .iter()
                .filter(|tick| tick.skipped_by.is_some())
                .map(|tick| {
                    (
                        tick.event_id.clone(),
                        tick.outcome.clone(),
                        tick.skipped_by.clone(),
                        tick.finished_at.is_some(),
                    )
                })
                .collect();
            let (transaction, audited) = query(
                begin(store, TransactionMode::Read).await.expect("read"),
                "SELECT actor FROM sqb_security_audit WHERE action='trigger.changed'",
                Vec::new(),
            )
            .await
            .expect("audit");
            commit(transaction).await.expect("commit");
            assert_eq!(
                (skipped, state.failing.is_none(), audited.len()),
                (
                    vec![(
                        Some(stuck.clone()),
                        "skipped".to_owned(),
                        Some("test-admin".to_owned()),
                        true
                    )],
                    true,
                    1
                ),
                "{}: the tick history shows who skipped the event and when, the sensor is no longer failing, and the audit log records it",
                test_case.description
            );

            change(store, trigger.trigger_id, TriggerChange::ResetCursor).await;
            tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
            let replayed = begin_sensor_evaluation(store, &lease, (trigger.trigger_id, None))
                .await
                .expect("request after reset")
                .expect("due");
            assert_eq!(
                replayed.event.map(|event| event.id().to_owned()),
                Some(stuck),
                "{}: a reset re-delivers every retained event, the skipped one too",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
