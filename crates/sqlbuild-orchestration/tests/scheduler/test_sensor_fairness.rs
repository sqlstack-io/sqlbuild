//! Unavailable callback locations cannot monopolize the shared due-sensor page.

use chrono::{TimeDelta, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::scheduler::main::_scheduler_cycle::scheduler_cycle;
use crate::scheduler::models::SchedulerState;
use crate::scheduler::tests::test_types::SensorFairnessTestCase;
use crate::sensors::models::SensorEvaluationOutcome;
use crate::sensors::tests::helpers::{evaluations, snapshot};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::query::query;
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_pending_custom_sensors_when_scheduling_then_rechecks_wait_and_other_locations_progress() {
    let test_cases = [
        SensorFairnessTestCase {
            description: "SQLite pending callback waits",
            backend: Backend::Sqlite,
            pending: 1,
            builtin_status: "paused",
            expected_completed: 0,
        },
        SensorFairnessTestCase {
            description: "PostgreSQL pending callback waits",
            backend: Backend::Postgres,
            pending: 1,
            builtin_status: "paused",
            expected_completed: 0,
        },
        SensorFairnessTestCase {
            description: "SQLite unavailable callback page permits builtin progress",
            backend: Backend::Sqlite,
            pending: 100,
            builtin_status: "running",
            expected_completed: 1,
        },
        SensorFairnessTestCase {
            description: "PostgreSQL unavailable callback page permits builtin progress",
            backend: Backend::Postgres,
            pending: 100,
            builtin_status: "running",
            expected_completed: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate fairness store");
            let custom = create_location(store, "fulfillment", "test-admin")
                .await
                .expect("callback location");
            let builtin = create_location(store, "reporting", "test-admin")
                .await
                .expect("builtin location");
            let mut input = snapshot(custom);
            input.documents.insert("triggers".into(), json!({}));
            for index in 0..test_case.pending {
                input.documents.get_mut("triggers").expect("triggers")[format!("orders_{index}")] = json!({
                    "description": "Poll fulfillment", "job_key": "orders", "when": {"kind": "custom", "function": "orders_ready", "code_hash": "orders_v1"},
                    "minimum_interval_seconds": 1
                });
            }
            publish_snapshot(store, &input)
                .await
                .expect("publish unavailable callbacks");
            publish_snapshot(store, &snapshot(builtin))
                .await
                .expect("publish independent builtin");
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("set due order");
            let (transaction, _) = query(transaction,
                "UPDATE sqb_trigger_state SET next_due_at=$1 WHERE trigger_id IN (SELECT trigger_id FROM sqb_triggers WHERE location_id=$2)",
                vec![Parameter::Time(Utc::now() - TimeDelta::hours(2)), Parameter::Id(custom)]).await.expect("callbacks first");
            let (transaction, _) = query(transaction,
                "UPDATE sqb_trigger_state SET next_due_at=$1,status=$2 WHERE trigger_id IN (SELECT trigger_id FROM sqb_triggers WHERE location_id=$3)",
                vec![Parameter::Time(Utc::now() - TimeDelta::hours(1)), Parameter::Text(test_case.builtin_status.into()), Parameter::Id(builtin)]).await.expect("builtin last");
            commit(transaction).await.expect("commit due order");
            let (state, first) = scheduler_cycle(store, SchedulerState::new(Uuid::now_v7()))
                .await
                .expect("pending page");
            assert_eq!(
                first.evaluated, test_case.pending,
                "{}",
                test_case.description
            );
            assert_eq!(
                first.wait_for.is_zero(),
                test_case.expected_completed == 1,
                "{}: only genuinely overdue unprocessed work should require an immediate retry",
                test_case.description
            );
            scheduler_cycle(store, state).await.expect("next page");
            let facts = evaluations(store).await;
            assert_eq!(
                facts
                    .iter()
                    .filter(|fact| fact.outcome == SensorEvaluationOutcome::Requested)
                    .count(),
                test_case.pending + test_case.expected_completed,
                "{}",
                test_case.description
            );
            assert_eq!(
                facts
                    .iter()
                    .filter(|fact| fact.outcome != SensorEvaluationOutcome::Requested)
                    .count(),
                test_case.expected_completed,
                "{}: builtin at another location completes without waiting for unavailable callbacks",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
