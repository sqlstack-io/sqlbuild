//! A member job's outcome inside a group run is a run event of that job, as soon as the member ends.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::runs::main::{
    execution::advance_groups::advance_groups, execution::change_run::change_run,
    execution::claim_next_run::claim_next_run, listings::list_runs::list_runs,
    requests::request_job_run::request_job_run,
};
use crate::runs::models::{
    JobRunRequest, NodeOutcome, RunChange, RunChangeRequest, RunOutcome, RunStatus,
};
use crate::runs::tests::claim_helpers::finish;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::tests::helpers::{evaluate_sensors, node_finished};
use crate::sensors::tests::test_types::{
    GroupMemberSensorTestCase, SensorTestCase, SkippedMemberSensorTestCase,
};
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_member_job_watched_by_a_sensor_when_its_group_runs_then_the_member_outcome_fires_once_while_the_group_runs()
 {
    let test_cases = [
        GroupMemberSensorTestCase {
            description: "SQLite member succeeds",
            backend: Backend::Sqlite,
            staging: (NodeOutcome::Success, RunOutcome::Success),
            marts: (NodeOutcome::Success, RunOutcome::Success),
            expected_firing: &["staging_success"],
            expected_after_marts: &["staging_success"],
        },
        GroupMemberSensorTestCase {
            description: "PostgreSQL member succeeds",
            backend: Backend::Postgres,
            staging: (NodeOutcome::Success, RunOutcome::Success),
            marts: (NodeOutcome::Success, RunOutcome::Success),
            expected_firing: &["staging_success"],
            expected_after_marts: &["staging_success"],
        },
        GroupMemberSensorTestCase {
            description: "SQLite member fails and its dependent is skipped",
            backend: Backend::Sqlite,
            staging: (NodeOutcome::Failed, RunOutcome::Failed),
            marts: (NodeOutcome::Skipped, RunOutcome::Failed),
            expected_firing: &["staging_failed"],
            expected_after_marts: &["marts_failed", "staging_failed"],
        },
        GroupMemberSensorTestCase {
            description: "PostgreSQL member fails and its dependent is skipped",
            backend: Backend::Postgres,
            staging: (NodeOutcome::Failed, RunOutcome::Failed),
            marts: (NodeOutcome::Skipped, RunOutcome::Failed),
            expected_firing: &["staging_failed"],
            expected_after_marts: &["marts_failed", "staging_failed"],
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = crate::runs::tests::helpers::snapshot(location);
            input.nodes.get_mut("orders").expect("orders").definition["deps"] =
                json!(["customers"]);
            input.documents.insert(
                "jobs".into(),
                json!({
                    "staging_job": {"description": "Stage customers", "selection": ["customers"]},
                    "marts_job": {"description": "Build orders", "selection": ["orders"]},
                    "report_job": {"description": "Report on inventory", "selection": ["inventory"]}
                }),
            );
            input.documents.insert(
                "groups".into(),
                json!({"nightly_job_group": {
                    "description": "Stage then build orders", "selection": ["customers", "orders"],
                    "jobs": {"staging_job": [], "marts_job": ["staging_job"]}
                }}),
            );
            let mut sensors = serde_json::Map::new();
            for (name, job, kind) in [
                ("staging_success", "staging_job", "success"),
                ("staging_failed", "staging_job", "failed"),
                ("marts_failed", "marts_job", "failed"),
                ("group_completed", "nightly_job_group", "completed"),
            ] {
                sensors.insert(
                    name.into(),
                    json!({
                        "description": "Report when a job ends", "function": "start_report_job", "code_hash": "v1", "timeout_seconds": 60,
                        "when": {"kind": "run_event", "job_key": job, "kinds": [kind]},
                        "minimum_interval_seconds": 1
                    }),
                );
            }
            input.documents.insert("triggers".into(), json!(sensors));
            publish_snapshot(store, &input).await.expect("publish");
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let group = Uuid::now_v7();
            request_job_run(
                store,
                (&context, None),
                &JobRunRequest {
                    run_id: group,
                    location_id: location,
                    target: "development".into(),
                    job_key: "nightly_job_group".into(),
                    reason: "Build the nightly group".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: Vec::new(),
                    exclude: Vec::new(),
                },
            )
            .await
            .expect("request group");
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            let scope = (location, "development");
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("start group");
            let staging = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim staging")
                .expect("staging ready");
            assert_eq!(staging.request.job_key.as_deref(), Some("staging_job"));
            finish(store, &staging, test_case.staging).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("advance group");

            let triggers = list_triggers(store, location, "development")
                .await
                .expect("triggers");
            let mut fired: BTreeMap<String, usize> = BTreeMap::new();
            let evaluate = async |fired: &mut BTreeMap<String, usize>| {
                evaluate_sensors(store, &lease, &triggers)
                    .await
                    .into_iter()
                    .for_each(|(name, count)| *fired.entry(name).or_default() += count);
            };
            evaluate(&mut fired).await;
            let firing: Vec<&str> = fired
                .iter()
                .filter(|(_, count)| **count > 0)
                .map(|(name, _)| name.as_str())
                .collect();
            assert_eq!(
                firing, test_case.expected_firing,
                "{}: the member's outcome fires before the group ends",
                test_case.description
            );
            assert!(
                fired.values().all(|count| *count <= 1),
                "{}: {fired:?}",
                test_case.description
            );
            let runs = list_runs(store, scope, 100).await.expect("runs");
            assert_eq!(
                runs.iter()
                    .find(|record| record.run_id == group)
                    .expect("group")
                    .status,
                RunStatus::Running,
                "{}: the group is still running",
                test_case.description
            );

            let marts = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim marts")
                .expect("marts ready");
            assert_eq!(marts.request.job_key.as_deref(), Some("marts_job"));
            finish(store, &marts, test_case.marts).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("finish group");
            tokio::time::sleep(Duration::from_millis(1100)).await;
            evaluate(&mut fired).await;
            let mut expected: Vec<&str> = test_case.expected_after_marts.to_vec();
            expected.push("group_completed");
            expected.sort_unstable();
            let firing: Vec<&str> = fired
                .iter()
                .filter(|(_, count)| **count > 0)
                .map(|(name, _)| name.as_str())
                .collect();
            assert_eq!(firing, expected, "{}", test_case.description);
            assert!(
                fired.values().all(|count| *count <= 1),
                "{}: each outcome fires once: {fired:?}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_member_retried_inside_its_group_when_sensors_watch_it_then_only_its_final_attempt_fires()
{
    let test_cases = [
        SensorTestCase {
            description: "SQLite member retry fires once",
            backend: Backend::Sqlite,
            expected_runs: 3,
        },
        SensorTestCase {
            description: "PostgreSQL member retry fires once",
            backend: Backend::Postgres,
            expected_runs: 3,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = crate::runs::tests::helpers::snapshot(location);
            input.documents.insert("jobs".into(), json!({
                "staging_job": {"description": "Stage customers and products", "selection": ["customers", "products"],
                    "on_failure": {"retries": 1, "retry_delay_seconds": 0}},
                "report_job": {"description": "Report on inventory", "selection": ["inventory"]}
            }));
            input.documents.insert(
                "groups".into(),
                json!({"nightly_job_group": {
                    "description": "Stage", "selection": ["customers", "products"],
                    "jobs": {"staging_job": []}
                }}),
            );
            let mut sensors = serde_json::Map::new();
            for (name, kind, nodes) in [
                ("staging_success", "success", None),
                ("staging_failed", "failed", None),
                ("staging_completed", "completed", None),
                (
                    "staging_executed",
                    "completed",
                    Some(["customers", "products"]),
                ),
            ] {
                let mut when =
                    json!({"kind": "run_event", "job_key": "staging_job", "kinds": [kind]});
                when.as_object_mut()
                    .expect("when")
                    .extend(nodes.map(|nodes| ("executed_successfully".to_owned(), json!(nodes))));
                sensors.insert(
                    name.into(),
                    json!({
                        "description": "Report when staging ends", "function": "start_report_job", "code_hash": "v1", "timeout_seconds": 60,
                        "when": when, "minimum_interval_seconds": 1
                    }),
                );
            }
            input.documents.insert("triggers".into(), json!(sensors));
            publish_snapshot(store, &input).await.expect("publish");
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            request_job_run(
                store,
                (&context, None),
                &JobRunRequest {
                    run_id: Uuid::now_v7(),
                    location_id: location,
                    target: "development".into(),
                    job_key: "nightly_job_group".into(),
                    reason: "Build the nightly group".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: Vec::new(),
                    exclude: Vec::new(),
                },
            )
            .await
            .expect("request group");
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            let scope = (location, "development");
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("start group");
            let first = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim staging")
                .expect("staging ready");
            let worker = crate::runs::models::RunAuthority::Worker {
                run_id: first.request.run_id,
                claim_id: first.claim_id,
            };
            for change in [
                RunChange::Started,
                RunChange::NodeStarted {
                    node_key: "products".into(),
                },
                node_finished("products", NodeOutcome::Success),
                RunChange::NodeStarted {
                    node_key: "customers".into(),
                },
                node_finished("customers", NodeOutcome::Failed),
                RunChange::Finished {
                    outcome: RunOutcome::Partial,
                    reason: "customers failed".into(),
                    exit_code: Some(1),
                },
            ] {
                change_run(
                    store,
                    worker,
                    &RunChangeRequest {
                        event_id: Uuid::now_v7(),
                        run_id: first.request.run_id,
                        claim_id: Some(first.claim_id),
                        change,
                    },
                )
                .await
                .expect("first attempt");
            }
            let triggers = list_triggers(store, location, "development")
                .await
                .expect("triggers");
            let evaluate = async || {
                let mut fired = evaluate_sensors(store, &lease, &triggers).await;
                fired.retain(|_, count| *count > 0);
                fired
            };
            assert!(
                evaluate().await.is_empty(),
                "{}: an attempt the group retries fires nothing",
                test_case.description
            );
            let second = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim retry")
                .expect("retry ready");
            assert_eq!(second.request.selection, ["customers".to_owned()].into());
            let worker = crate::runs::models::RunAuthority::Worker {
                run_id: second.request.run_id,
                claim_id: second.claim_id,
            };
            for change in [
                RunChange::Started,
                RunChange::NodeStarted {
                    node_key: "customers".into(),
                },
                node_finished("customers", NodeOutcome::Success),
                RunChange::Finished {
                    outcome: RunOutcome::Success,
                    reason: "retry succeeded".into(),
                    exit_code: Some(0),
                },
            ] {
                change_run(
                    store,
                    worker,
                    &RunChangeRequest {
                        event_id: Uuid::now_v7(),
                        run_id: second.request.run_id,
                        claim_id: Some(second.claim_id),
                        change,
                    },
                )
                .await
                .expect("second attempt");
            }
            tokio::time::sleep(Duration::from_millis(1100)).await;
            let fired = evaluate().await;
            assert_eq!(
                fired,
                BTreeMap::from([
                    ("staging_completed".to_owned(), 1),
                    ("staging_executed".to_owned(), 1),
                    ("staging_success".to_owned(), 1),
                ]),
                "{}: the final attempt fires once, and nodes built by earlier attempts count",
                test_case.description
            );
            assert_eq!(fired.values().sum::<usize>(), test_case.expected_runs);
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_member_the_group_skips_when_sensors_and_alerts_watch_it_then_it_is_skipped_not_failed() {
    let test_cases = [
        SkippedMemberSensorTestCase {
            description: "SQLite group skips its dependent member",
            backend: Backend::Sqlite,
            expected_marts_status: RunStatus::Skipped,
            expected_fired: &[("marts_completed", 1), ("marts_skipped", 1)],
        },
        SkippedMemberSensorTestCase {
            description: "PostgreSQL group skips its dependent member",
            backend: Backend::Postgres,
            expected_marts_status: RunStatus::Skipped,
            expected_fired: &[("marts_completed", 1), ("marts_skipped", 1)],
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = crate::runs::tests::helpers::snapshot(location);
            input.nodes.get_mut("orders").expect("orders").definition["deps"] =
                json!(["customers"]);
            input.documents.insert(
                "jobs".into(),
                json!({
                    "staging_job": {"description": "Stage customers", "selection": ["customers"]},
                    "marts_job": {"description": "Build orders", "selection": ["orders"]},
                    "report_job": {"description": "Report on inventory", "selection": ["inventory"]}
                }),
            );
            input.documents.insert(
                "groups".into(),
                json!({"nightly_job_group": {
                    "description": "Stage then build orders", "selection": ["customers", "orders"],
                    "jobs": {"staging_job": [], "marts_job": ["staging_job"]},
                    "dependency_failure": {"marts_job": "skip"}
                }}),
            );
            let mut sensors = serde_json::Map::new();
            for kind in ["skipped", "failed", "completed"] {
                sensors.insert(
                    format!("marts_{kind}"),
                    json!({
                        "description": "Report when marts ends", "function": "start_report_job", "code_hash": "v1", "timeout_seconds": 60,
                        "when": {"kind": "run_event", "job_key": "marts_job", "kinds": [kind]},
                        "minimum_interval_seconds": 1
                    }),
                );
            }
            input.documents.insert("triggers".into(), json!(sensors));
            publish_snapshot(store, &input).await.expect("publish");
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            request_job_run(
                store,
                (&context, None),
                &JobRunRequest {
                    run_id: Uuid::now_v7(),
                    location_id: location,
                    target: "development".into(),
                    job_key: "nightly_job_group".into(),
                    reason: "Build the nightly group".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: Vec::new(),
                    exclude: Vec::new(),
                },
            )
            .await
            .expect("request group");
            let lease = acquire_lease(store, (Uuid::now_v7(), None))
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            let scope = (location, "development");
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("start group");
            let staging = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim staging")
                .expect("staging ready");
            finish(store, &staging, (NodeOutcome::Failed, RunOutcome::Failed)).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("skip marts");
            let runs = list_runs(store, scope, 100).await.expect("runs");
            let marts = runs
                .iter()
                .find(|record| record.job_key.as_deref() == Some("marts_job"))
                .expect("marts run");
            assert_eq!(
                marts.status, test_case.expected_marts_status,
                "{}",
                test_case.description
            );
            let alerts = crate::runs::tests::helpers::rows(
                store,
                "SELECT run_id FROM recall_events WHERE event_type='platform_run_failed'",
                vec![],
            )
            .await;
            assert_eq!(
                alerts
                    .iter()
                    .map(|row| row["run_id"].as_str().unwrap_or_default().to_owned())
                    .collect::<Vec<_>>(),
                vec![staging.request.run_id.to_string()],
                "{}: only the failed member alerts",
                test_case.description
            );
            let triggers = list_triggers(store, location, "development")
                .await
                .expect("triggers");
            let mut fired = evaluate_sensors(store, &lease, &triggers).await;
            fired.retain(|_, count| *count > 0);
            assert_eq!(
                fired,
                test_case
                    .expected_fired
                    .iter()
                    .map(|(name, count)| ((*name).to_owned(), *count))
                    .collect::<BTreeMap<_, _>>(),
                "{}: a skipped member fires skipped, never failed",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
