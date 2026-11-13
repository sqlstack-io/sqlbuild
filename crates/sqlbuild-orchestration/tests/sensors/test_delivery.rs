use std::collections::BTreeSet;
use std::time::Duration;

use chrono::{TimeDelta, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::TriggerChange;
use crate::runs::main::list_runs::list_runs;
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::models::JobRunRequest;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::complete_builtin_sensor::complete_builtin_sensor;
use crate::sensors::models::SensorEvaluationOutcome;
use crate::sensors::tests::helpers::{append_events, change, evaluations, snapshot};
use crate::sensors::tests::test_types::SensorTestCase;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_paged_platform_events_when_sensor_publication_fails_then_retry_is_atomic_and_reset_deduplicates()
 {
    let test_cases = [
        SensorTestCase {
            description: "SQLite sensor delivery",
            backend: Backend::Sqlite,
            expected_runs: 54,
        },
        SensorTestCase {
            description: "PostgreSQL sensor delivery",
            backend: Backend::Postgres,
            expected_runs: 54,
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
                .expect("publish sensor");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let now = Utc::now();
            append_events(store, 0..105, now).await;
            let requested = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("request")
                .expect("due");
            let replica = second_store(store).await;
            assert_eq!(
                begin_sensor_evaluation(&replica, &lease, trigger.trigger_id)
                    .await
                    .expect("recover pending"),
                Some(requested.clone())
            );
            for table in [
                "sqb_runs",
                "sqb_run_requests",
                "sqb_sensor_evaluation_events",
            ] {
                let point = TransactionFault {
                    description: "fail sensor transaction",
                    table,
                    operation: "INSERT",
                    condition: "",
                    sqlite_body: "SELECT RAISE(ABORT, 'injected sensor failure')",
                    postgres_body: "RAISE EXCEPTION 'injected sensor failure'",
                };
                install_fault(store, &point).await;
                assert!(
                    complete_builtin_sensor(store, &lease, &requested)
                        .await
                        .is_err(),
                    "{table}"
                );
                remove_fault(store, &point).await;
                assert!(
                    list_runs(store, (location, "development"), 1000)
                        .await
                        .expect("rolled back runs")
                        .is_empty(),
                    "{table}"
                );
                assert_eq!(
                    evaluations(store).await.len(),
                    1,
                    "only requested survives {table}"
                );
            }
            assert_eq!(
                complete_builtin_sensor(&replica, &lease, &requested)
                    .await
                    .expect("retry first page"),
                50
            );
            assert_eq!(
                complete_builtin_sensor(store, &lease, &requested)
                    .await
                    .expect("retry completion"),
                0
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
            let second = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("next page")
                .expect("due");
            assert!(second.cursor.event_storage_id > 0);
            assert_eq!(
                complete_builtin_sensor(store, &lease, &second)
                    .await
                    .expect("remaining page"),
                3
            );
            // Committed later, but its occurrence precedes the entire first page.
            append_events(store, 106..107, now - TimeDelta::days(1)).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            let late = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("late request")
                .expect("due");
            assert!(late.cursor.event_storage_id > second.cursor.event_storage_id);
            assert_eq!(
                complete_builtin_sensor(store, &lease, &late)
                    .await
                    .expect("late arrival"),
                1
            );
            change(store, trigger.trigger_id, TriggerChange::ResetCursor).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
            let replay = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("replay")
                .expect("due");
            assert_eq!(replay.cursor.event_storage_id, 0);
            assert_eq!(
                complete_builtin_sensor(store, &lease, &replay)
                    .await
                    .expect("replay retained first page"),
                50
            );
            let runs = list_runs(store, (location, "development"), 1000)
                .await
                .expect("queued runs");
            assert_eq!(
                runs.len(),
                test_case.expected_runs,
                "{}: reset never requeues an existing run key",
                test_case.description
            );
            assert!(
                runs.iter()
                    .all(|run| run.selection_json == BTreeSet::from(["orders".into()]))
            );
            let facts = evaluations(store).await;
            assert_eq!(facts.len(), 8);
            assert!(facts.iter().all(|fact| matches!(
                fact.outcome,
                SensorEvaluationOutcome::Requested | SensorEvaluationOutcome::Completed { .. }
            )));
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_skip_overlap_job_with_an_active_run_when_a_sensor_matches_then_it_records_skips_instead_of_queueing()
 {
    let test_cases = [
        SensorTestCase {
            description: "SQLite sensor skip",
            backend: Backend::Sqlite,
            expected_runs: 1,
        },
        SensorTestCase {
            description: "PostgreSQL sensor skip",
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
            input.documents.insert(
                "jobs".into(),
                json!({"orders": {"description": "Refresh orders", "selection": ["orders"], "overlap": "skip"}}),
            );
            publish_snapshot(store, &input)
                .await
                .expect("publish sensor");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let active = Uuid::now_v7();
            request_job_run(
                store,
                (
                    &AuditContext {
                        actor: "test-operator".into(),
                        granting_role: "control-store-owner".into(),
                        request_id: Uuid::now_v7(),
                        ip_address: None,
                    },
                    None,
                ),
                &JobRunRequest {
                    run_id: active,
                    location_id: location,
                    target: "development".into(),
                    job_key: "orders".into(),
                    reason: "Refresh by hand".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: Vec::new(),
                    exclude: Vec::new(),
                },
            )
            .await
            .expect("a person's run queues regardless of overlap");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            // The first fixture event is a tick-overdue event, which the sensor matches.
            append_events(store, 0..1, Utc::now()).await;
            let requested = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("request")
                .expect("due");
            assert_eq!(
                complete_builtin_sensor(store, &lease, &requested)
                    .await
                    .expect("complete"),
                0,
                "{}",
                test_case.description
            );
            let facts = evaluations(store).await;
            let Some(SensorEvaluationOutcome::Completed {
                run_ids, skipped, ..
            }) = facts.last().map(|fact| fact.outcome.clone())
            else {
                panic!("a completed evaluation, not {facts:?}");
            };
            assert!(run_ids.is_empty(), "{}", test_case.description);
            assert_eq!(
                skipped
                    .iter()
                    .map(|skip| skip.active_run_id)
                    .collect::<Vec<_>>(),
                vec![active],
                "{}: the matched event names the active run",
                test_case.description
            );
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("runs")
                    .len(),
                test_case.expected_runs,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
