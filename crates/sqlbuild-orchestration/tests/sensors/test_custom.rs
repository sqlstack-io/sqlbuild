//! The real outbound HTTP boundary returns data, never scheduler authority.

use std::time::Duration;

use chrono::{TimeDelta, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::api::tests::helpers::{start_api, stop_api};
use crate::credentials::main::tokens::issue_token::issue_token;
use crate::credentials::models::{AuditContext, CredentialScope};
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::platform::main::connect_location_work::connect_location_work;
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{
    LocationHealth, LocationHealthReport, LocationWork, LocationWorkOutcome, LocationWorkPoll,
    LocationWorkResult,
};
use crate::runs::main::list_runs::list_runs;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation;
use crate::sensors::main::complete_custom_sensor::complete_custom_sensor;
use crate::sensors::models::{CustomSensorOutcome, SensorEvaluationOutcome};
use crate::sensors::tests::helpers::{evaluations, snapshot};
use crate::sensors::tests::test_types::{CustomSensorResultCase, SensorTestCase};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_custom_sensor_work_when_results_retry_then_cursor_runs_and_outcome_commit_atomically() {
    let test_cases = [
        SensorTestCase {
            description: "SQLite custom sensor protocol",
            backend: Backend::Sqlite,
            expected_runs: 2,
        },
        SensorTestCase {
            description: "PostgreSQL custom sensor protocol",
            backend: Backend::Postgres,
            expected_runs: 2,
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
            input.documents.get_mut("triggers").expect("triggers")["health_orders"]["when"] =
                json!({"kind": "custom", "function": "orders_ready", "code_hash": "orders_v1"});
            publish_snapshot(store, &input)
                .await
                .expect("publish custom sensor data");
            let trigger = list_triggers(store, location, "development")
                .await
                .expect("triggers")
                .remove(0);
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "admin".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let token = issue_token(
                store,
                &context,
                CredentialScope::Location {
                    location_id: location,
                },
                Utc::now() + TimeDelta::hours(1),
            )
            .await
            .expect("scoped token");
            let api = start_api(store).await;
            let client =
                connect_location_work((&api.url, &token.bearer), location).expect("client");
            let poll = LocationWorkPoll {
                session_id: Uuid::now_v7(),
                target: "development".into(),
            };
            report_location_health(
                store,
                location,
                &LocationHealthReport {
                    event_id: Uuid::now_v7(),
                    session_id: poll.session_id,
                    target: poll.target.clone(),
                    health: LocationHealth::Heartbeat {},
                },
            )
            .await
            .expect("register location instance");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let point = TransactionFault {
                description: "request and outbound work are atomic",
                table: "sqb_location_work",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT, 'injected sensor request failure')",
                postgres_body: "RAISE EXCEPTION 'injected sensor request failure'",
            };
            install_fault(store, &point).await;
            assert!(
                begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                    .await
                    .is_err()
            );
            remove_fault(store, &point).await;
            assert!(
                evaluations(store).await.is_empty(),
                "request fact rolled back with work"
            );
            let requested = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("begin")
                .expect("due");
            assert_eq!(
                complete_custom_sensor(store, &lease, &requested)
                    .await
                    .expect("still waiting"),
                0
            );
            assert_eq!(
                evaluations(store).await.len(),
                1,
                "no invented terminal phase while waiting"
            );
            let claim = client.poll(&poll).await.expect("poll").expect("work");
            assert_eq!(claim.request.request_id, requested.evaluation_id);
            assert_eq!(
                claim.request.work,
                LocationWork::EvaluateSensor {
                    evaluation: Box::new(requested.clone())
                }
            );
            assert_eq!(
                client.poll(&poll).await.expect("lost poll response"),
                Some(claim.clone())
            );
            let wrong = LocationWorkResult {
                claim_id: claim.claim_id,
                session_id: poll.session_id,
                outcome: LocationWorkOutcome::Connection { healthy: true },
            };
            assert!(
                client
                    .complete(requested.evaluation_id, &wrong)
                    .await
                    .is_err(),
                "wrong result kind"
            );
            let empty_key = LocationWorkResult {
                outcome: LocationWorkOutcome::SensorEvaluation {
                    outcome: CustomSensorOutcome::Completed {
                        cursor: Some("unsafe".into()),
                        run_keys: vec![" ".into()],
                    },
                },
                ..wrong.clone()
            };
            assert!(
                client
                    .complete(requested.evaluation_id, &empty_key)
                    .await
                    .is_err(),
                "empty keys fail closed"
            );
            let result = LocationWorkResult {
                outcome: LocationWorkOutcome::SensorEvaluation {
                    outcome: CustomSensorOutcome::Completed {
                        cursor: Some("orders/page-2".into()),
                        run_keys: vec!["orders-2".into(), "orders-1".into(), "orders-2".into()],
                    },
                },
                ..wrong
            };
            assert!(
                client
                    .complete(requested.evaluation_id, &result)
                    .await
                    .expect("durable location result")
            );
            assert!(
                !client
                    .complete(requested.evaluation_id, &result)
                    .await
                    .expect("uncertain acknowledgement retry")
            );
            assert!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("runs")
                    .is_empty(),
                "location result alone has no queue authority"
            );
            for table in [
                "sqb_runs",
                "sqb_run_requests",
                "sqb_sensor_evaluation_events",
            ] {
                let point = TransactionFault {
                    description: "custom sensor atomic publication",
                    table,
                    operation: "INSERT",
                    condition: "",
                    sqlite_body: "SELECT RAISE(ABORT, 'injected sensor failure')",
                    postgres_body: "RAISE EXCEPTION 'injected sensor failure'",
                };
                install_fault(store, &point).await;
                assert!(
                    complete_custom_sensor(store, &lease, &requested)
                        .await
                        .is_err()
                );
                remove_fault(store, &point).await;
                assert!(
                    list_runs(store, (location, "development"), 100)
                        .await
                        .expect("rollback runs")
                        .is_empty()
                );
                assert_eq!(
                    evaluations(store).await.len(),
                    1,
                    "outcome rolled back with cursor and queue"
                );
                assert_eq!(
                    begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                        .await
                        .expect("unchanged cursor"),
                    Some(requested.clone())
                );
            }
            let replica = second_store(store).await;
            assert_eq!(
                complete_custom_sensor(&replica, &lease, &requested)
                    .await
                    .expect("replica commit"),
                test_case.expected_runs
            );
            assert_eq!(
                complete_custom_sensor(store, &lease, &requested)
                    .await
                    .expect("retry commit"),
                0
            );
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("dedup runs")
                    .len(),
                test_case.expected_runs,
                "{}",
                test_case.description
            );
            let facts = evaluations(store).await;
            let SensorEvaluationOutcome::Completed { cursor, run_ids } =
                &facts.last().expect("completed").outcome
            else {
                panic!("completed phase required");
            };
            assert_eq!(cursor.custom.as_deref(), Some("orders/page-2"));
            assert_eq!(cursor.event_storage_id, requested.cursor.event_storage_id);
            assert_eq!(run_ids.len(), test_case.expected_runs);
            for result_case in [
                CustomSensorResultCase {
                    outcome: CustomSensorOutcome::Completed {
                        cursor: Some("orders/page-2".into()),
                        run_keys: vec!["orders-2".into(), "orders-1".into()],
                    },
                    expected_phase: "completed",
                    expected_cursor: Some("orders/page-2"),
                },
                CustomSensorResultCase {
                    outcome: CustomSensorOutcome::Failed,
                    expected_phase: "failed",
                    expected_cursor: None,
                },
                CustomSensorResultCase {
                    outcome: CustomSensorOutcome::TimedOut,
                    expected_phase: "timed_out",
                    expected_cursor: None,
                },
                CustomSensorResultCase {
                    outcome: CustomSensorOutcome::Skipped {
                        cursor: Some("orders/page-3".into()),
                        reason: "No new orders".into(),
                    },
                    expected_phase: "completed",
                    expected_cursor: Some("orders/page-3"),
                },
            ] {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let pending = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                    .await
                    .expect("next attempt")
                    .expect("due");
                assert_eq!(
                    pending.cursor.custom.as_deref(),
                    Some("orders/page-2"),
                    "failed and timeout leave cursor unchanged"
                );
                let claim = client
                    .poll(&poll)
                    .await
                    .expect("next poll")
                    .expect("next work");
                assert_eq!(claim.request.request_id, pending.evaluation_id);
                let result = LocationWorkResult {
                    claim_id: claim.claim_id,
                    session_id: poll.session_id,
                    outcome: LocationWorkOutcome::SensorEvaluation {
                        outcome: result_case.outcome,
                    },
                };
                assert!(
                    client
                        .complete(pending.evaluation_id, &result)
                        .await
                        .expect("complete callback")
                );
                complete_custom_sensor(store, &lease, &pending)
                    .await
                    .expect("apply phase");
                let facts = evaluations(store).await;
                let outcome = &facts.last().expect("terminal evaluation phase").outcome;
                assert_eq!(outcome.phase(), result_case.expected_phase);
                let payload = serde_json::to_value(outcome).expect("outcome payload");
                assert_eq!(
                    payload["cursor"]["custom"].as_str(),
                    result_case.expected_cursor
                );
            }
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("still deduped")
                    .len(),
                test_case.expected_runs
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
            let stale = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                .await
                .expect("request before takeover")
                .expect("due");
            let stale_claim = client
                .poll(&poll)
                .await
                .expect("claim before takeover")
                .expect("work");
            assert_eq!(stale_claim.request.request_id, stale.evaluation_id);
            expire_lease(store).await;
            assert!(
                client
                    .poll(&poll)
                    .await
                    .expect("no expired-epoch dispatch")
                    .is_none()
            );
            assert!(matches!(
                complete_custom_sensor(store, &lease, &stale).await,
                Err(ControlError::Fenced)
            ));
            let successor = acquire_lease(&replica, Uuid::now_v7())
                .await
                .expect("takeover")
                .expect("leader");
            let late = LocationWorkResult {
                claim_id: stale_claim.claim_id,
                session_id: poll.session_id,
                outcome: LocationWorkOutcome::SensorEvaluation {
                    outcome: CustomSensorOutcome::Completed {
                        cursor: Some("uncommitted/stale".into()),
                        run_keys: vec!["must-not-queue".into()],
                    },
                },
            };
            assert!(
                client
                    .complete(stale.evaluation_id, &late)
                    .await
                    .expect("late location result")
            );
            assert!(matches!(
                complete_custom_sensor(&replica, &successor, &stale).await,
                Err(ControlError::Fenced)
            ));
            tokio::time::sleep(Duration::from_secs(1)).await;
            let recovered = begin_sensor_evaluation(&replica, &successor, trigger.trigger_id)
                .await
                .expect("recover abandoned callback")
                .expect("due after minimum interval");
            assert_eq!(recovered.cursor.custom.as_deref(), Some("orders/page-3"));
            assert_eq!(
                list_runs(store, (location, "development"), 100)
                    .await
                    .expect("no stale queued request")
                    .len(),
                test_case.expected_runs
            );
            assert_eq!(
                evaluations(store)
                    .await
                    .iter()
                    .filter(|fact| fact.request.evaluation_id == stale.evaluation_id
                        && fact.outcome == SensorEvaluationOutcome::Abandoned)
                    .count(),
                1
            );
            assert_eq!(
                client
                    .poll(&poll)
                    .await
                    .expect("new epoch work")
                    .expect("new claim")
                    .request
                    .request_id,
                recovered.evaluation_id,
                "retired evaluation must not block or execute before current work"
            );
            close_store(replica).await;
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
