use std::time::Duration;

use chrono::TimeDelta;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::{TriggerChange, TriggerStatus};
use crate::platform::_helpers::maintenance::retention::retain;
use crate::runs::main::listings::list_runs::list_runs;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::models::{AbandonReason, SensorEvaluationOutcome};
use crate::sensors::tests::helpers::complete_starting_job;
use crate::sensors::tests::helpers::{append_events, change, evaluations, snapshot};
use crate::sensors::tests::test_types::SensorTestCase;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_crashed_sensor_when_leadership_or_snapshot_changes_then_stale_results_cannot_queue_runs()
{
    let test_cases = [
        SensorTestCase {
            description: "SQLite sensor recovery",
            backend: Backend::Sqlite,
            expected_runs: 1,
        },
        SensorTestCase {
            description: "PostgreSQL sensor recovery",
            backend: Backend::Postgres,
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
                .expect("publish sensor");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            append_events(store, 0..1, chrono::Utc::now()).await;
            let first = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let pending = begin_sensor_evaluation(store, &first, (trigger.trigger_id, None))
                .await
                .expect("request")
                .expect("due");
            expire_lease(store).await;
            let replica = second_store(store).await;
            let successor = acquire_lease(&replica, Uuid::now_v7())
                .await
                .expect("takeover")
                .expect("new leader");
            assert!(matches!(
                complete_starting_job(store, &first, &pending).await,
                Err(ControlError::Fenced)
            ));
            assert!(
                begin_sensor_evaluation(&replica, &successor, (trigger.trigger_id, None))
                    .await
                    .expect("recover abandoned attempt")
                    .is_none(),
                "a crash still counts toward the minimum interval"
            );
            assert!(matches!(
                evaluations(store)
                    .await
                    .last()
                    .expect("abandoned fact")
                    .outcome,
                SensorEvaluationOutcome::Abandoned {
                    reason: AbandonReason::LeaderChanged
                }
            ));
            tokio::time::sleep(Duration::from_secs(1)).await;
            let requested = begin_sensor_evaluation(store, &successor, (trigger.trigger_id, None))
                .await
                .expect("new request")
                .expect("due");
            assert_ne!(pending.evaluation_id, requested.evaluation_id);
            let mut forged = requested.clone();
            forged.cursor.event_storage_id = 999;
            assert!(matches!(
                complete_starting_job(store, &successor, &forged).await,
                Err(ControlError::Fenced)
            ));
            input.generation += 1;
            publish_snapshot(store, &input).await.expect("redeploy");
            assert_eq!(
                complete_starting_job(store, &successor, &requested)
                    .await
                    .expect("abandon stale snapshot"),
                0
            );
            assert!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("no stale runs")
                    .is_empty()
            );
            change(store, trigger.trigger_id, TriggerChange::Pause).await;
            input.generation += 1;
            publish_snapshot(store, &input)
                .await
                .expect("redeploy paused sensor");
            assert_eq!(
                list_triggers(store, location, "development")
                    .await
                    .expect("preserved override")[0]
                    .status,
                TriggerStatus::Paused
            );
            assert!(
                begin_sensor_evaluation(store, &successor, (trigger.trigger_id, None))
                    .await
                    .expect("paused")
                    .is_none()
            );
            change(store, trigger.trigger_id, TriggerChange::ResetDefault).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            let current = begin_sensor_evaluation(store, &successor, (trigger.trigger_id, None))
                .await
                .expect("code default")
                .expect("running");
            assert_eq!(
                complete_starting_job(store, &successor, &current)
                    .await
                    .expect("catch up retained health"),
                test_case.expected_runs,
                "{}",
                test_case.description
            );
            append_events(store, 2..3, chrono::Utc::now()).await;
            let before_reset =
                begin_sensor_evaluation(store, &successor, (trigger.trigger_id, None))
                    .await
                    .expect("request before reset")
                    .expect("a new event is due at once after a delivered one");
            change(store, trigger.trigger_id, TriggerChange::ResetCursor).await;
            assert_eq!(
                complete_starting_job(store, &successor, &before_reset)
                    .await
                    .expect("reset fences old cursor"),
                0
            );
            assert!(matches!(
                evaluations(store)
                    .await
                    .last()
                    .expect("reset abandonment")
                    .outcome,
                SensorEvaluationOutcome::Abandoned {
                    reason: AbandonReason::CursorReset
                }
            ));
            tokio::time::sleep(Duration::from_secs(1)).await;
            let before_pause =
                begin_sensor_evaluation(store, &successor, (trigger.trigger_id, None))
                    .await
                    .expect("request before pause")
                    .expect("due");
            change(store, trigger.trigger_id, TriggerChange::Pause).await;
            assert_eq!(
                complete_starting_job(store, &successor, &before_pause)
                    .await
                    .expect("pause fences pending result"),
                0
            );
            assert!(matches!(
                evaluations(store)
                    .await
                    .last()
                    .expect("pause abandonment")
                    .outcome,
                SensorEvaluationOutcome::Abandoned {
                    reason: AbandonReason::Paused
                }
            ));
            change(store, trigger.trigger_id, TriggerChange::ResetDefault).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            let delayed = begin_sensor_evaluation(store, &successor, (trigger.trigger_id, None))
                .await
                .expect("request delayed evaluation")
                .expect("due");
            tokio::time::sleep(Duration::from_secs(1)).await;
            complete_starting_job(store, &successor, &delayed)
                .await
                .expect("complete the replayed first event");
            let immediate = begin_sensor_evaluation(store, &successor, (trigger.trigger_id, None))
                .await
                .expect("the next event is due at once")
                .expect("already due");
            assert_ne!(immediate.evaluation_id, delayed.evaluation_id);
            complete_starting_job(store, &successor, &immediate)
                .await
                .expect("complete the second event");
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("deduplicated runs")
                    .len(),
                test_case.expected_runs + 1,
                "{}: the replayed first event reuses its run; the second starts one",
                test_case.description
            );
            let transaction = begin(store, TransactionMode::Write).await.expect("age");
            let aged = transaction.now - TimeDelta::days(8);
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_sensor_evaluation_events SET occurred_at=$1 WHERE evaluation_id<>$2",
                vec![
                    Parameter::Time(aged),
                    Parameter::Id(immediate.evaluation_id),
                ],
            )
            .await
            .expect("aged evaluations");
            commit(transaction)
                .await
                .expect("aged evaluations committed");
            retain(store, Default::default(), &[])
                .await
                .expect("retention pass");
            assert_eq!(
                evaluations(store)
                    .await
                    .iter()
                    .map(|fact| fact.request.evaluation_id)
                    .collect::<Vec<_>>(),
                [immediate.evaluation_id, immediate.evaluation_id],
                "{}: retention keeps only the latest attempt's phases",
                test_case.description
            );
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
