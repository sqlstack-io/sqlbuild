//! Actual lifecycle facts feed scoped outcome selectors and atomic recovery on both stores.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::declarations::main::list_triggers::list_triggers;
use crate::declarations::models::TriggerChange;
use crate::runs::main::list_runs::list_runs;
use crate::runs::main::request_rerun::request_rerun;
use crate::runs::models::{NodeOutcome, RerunRequest, RunOutcome};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::sensors::main::{
    begin_sensor_evaluation::begin_sensor_evaluation,
    complete_builtin_sensor::complete_builtin_sensor,
};
use crate::sensors::tests::helpers::{change, finish_claimed_run, finish_run, snapshot};
use crate::sensors::tests::test_types::{SensorExecutionTestCase, SensorTestCase};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::{
    begin::begin, commit::commit, migrate_store::migrate_store, query::query,
};
use crate::store::models::{Parameter, TransactionMode};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_terminal_jobs_and_groups_when_selectors_retry_then_outcomes_scope_and_run_keys_are_preserved()
 {
    let test_cases = [
        SensorTestCase {
            description: "SQLite terminal outcome sensor recovery",
            backend: Backend::Sqlite,
            expected_runs: 106,
        },
        SensorTestCase {
            description: "PostgreSQL terminal outcome sensor recovery",
            backend: Backend::Postgres,
            expected_runs: 106,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            migrate_store(store)
                .await
                .expect("warm migration preserves index");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let other = create_location(store, "products", "test-admin")
                .await
                .expect("other location");
            let mut input = snapshot(location);
            input.documents.insert("jobs".into(), json!({
                "orders": {"description": "Source orders", "selection": ["orders", "customers"], "overlap": "allow"},
                "inventory": {"description": "Sensor destination", "selection": ["orders", "customers"], "overlap": "allow"}
            }));
            input.documents.insert("groups".into(), json!({
                "daily_orders": {"description": "Source group", "selection": ["orders", "customers"], "jobs": {"orders": []}, "overlap": "allow"}
            }));
            let selectors = [
                "completed",
                "success",
                "warn",
                "partial",
                "failed",
                "cancelled",
                "presumed_failed",
            ];
            let mut sensors = serde_json::Map::new();
            for job_key in ["orders", "daily_orders"] {
                for selector in selectors {
                    sensors.insert(format!("{job_key}_{selector}"), json!({
                        "description": "Consume exact terminal outcomes", "job_key": "inventory",
                        "when": {"kind": "run_event", "job_key": job_key, "kinds": [selector]},
                        "minimum_interval_seconds": 1
                    }));
                }
                for (name, kind, nodes) in [
                    ("both_completed", "completed", vec!["customers", "orders"]),
                    ("customers_completed", "completed", vec!["customers"]),
                    ("customers_success", "success", vec!["customers"]),
                    ("customers_partial", "partial", vec!["customers"]),
                    ("orders_completed", "completed", vec!["orders"]),
                ] {
                    sensors.insert(format!("{job_key}_executed_{name}"), json!({
                        "description": "Require every selected node to actually succeed", "job_key": "inventory",
                        "when": {"kind": "run_event", "job_key": job_key, "kinds": [kind], "executed_successfully": nodes},
                        "minimum_interval_seconds": 1
                    }));
                }
            }
            input.documents.insert("triggers".into(), json!(sensors));
            publish_snapshot(store, &input)
                .await
                .expect("publish sensors");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let mut expected: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            let outcomes = [
                (
                    "success",
                    RunOutcome::Success,
                    [NodeOutcome::Success, NodeOutcome::Success],
                ),
                (
                    "warn",
                    RunOutcome::Warn,
                    [NodeOutcome::Warn, NodeOutcome::Success],
                ),
                (
                    "partial",
                    RunOutcome::Partial,
                    [NodeOutcome::Success, NodeOutcome::Failed],
                ),
                (
                    "failed",
                    RunOutcome::Failed,
                    [NodeOutcome::Failed, NodeOutcome::Failed],
                ),
                (
                    "cancelled",
                    RunOutcome::Cancelled,
                    [NodeOutcome::Cancelled, NodeOutcome::Cancelled],
                ),
                (
                    "presumed_failed",
                    RunOutcome::PresumedFailed,
                    [NodeOutcome::Failed, NodeOutcome::Failed],
                ),
            ];
            for (selector, outcome, nodes) in outcomes {
                let id = finish_run(
                    store,
                    &lease,
                    (location, "development"),
                    "orders",
                    outcome,
                    nodes,
                    [false; 2],
                )
                .await
                .to_string();
                expected
                    .entry(format!("orders_{selector}"))
                    .or_default()
                    .insert(id.clone());
                expected
                    .entry("orders_completed".into())
                    .or_default()
                    .insert(id);
            }
            let group = finish_run(
                store,
                &lease,
                (location, "development"),
                "daily_orders",
                RunOutcome::Success,
                [NodeOutcome::Success; 2],
                [false; 2],
            )
            .await;
            let sources = list_runs(store, (location, "development"), 100)
                .await
                .expect("sources");
            let child = sources
                .iter()
                .find(|record| {
                    record.run_id != group
                        && record.job_key.as_deref() == Some("orders")
                        && !expected["orders_completed"].contains(&record.run_id.to_string())
                })
                .expect("group child");
            for selector in ["completed", "success"] {
                expected
                    .entry(format!("daily_orders_{selector}"))
                    .or_default()
                    .insert(group.to_string());
                expected
                    .entry(format!("orders_{selector}"))
                    .or_default()
                    .insert(child.run_id.to_string());
            }
            let executions = [
                SensorExecutionTestCase {
                    description: "freshness-only success cannot borrow another execution's starts",
                    outcome: RunOutcome::Success,
                    nodes: [NodeOutcome::Success; 2],
                    started: [false; 2],
                    expected_selector: "success",
                    expected_filters: &[],
                },
                SensorExecutionTestCase {
                    description: "all selected nodes must start, not merely any node",
                    outcome: RunOutcome::Success,
                    nodes: [NodeOutcome::Success; 2],
                    started: [true, false],
                    expected_selector: "success",
                    expected_filters: &["customers_completed", "customers_success"],
                },
                SensorExecutionTestCase {
                    description: "both selected nodes actually succeed",
                    outcome: RunOutcome::Success,
                    nodes: [NodeOutcome::Success; 2],
                    started: [true; 2],
                    expected_selector: "success",
                    expected_filters: &[
                        "both_completed",
                        "customers_completed",
                        "customers_success",
                        "orders_completed",
                    ],
                },
                SensorExecutionTestCase {
                    description: "partial outcome is independent of selected-node success",
                    outcome: RunOutcome::Partial,
                    nodes: [NodeOutcome::Success, NodeOutcome::Failed],
                    started: [true; 2],
                    expected_selector: "partial",
                    expected_filters: &["customers_completed", "customers_partial"],
                },
                SensorExecutionTestCase {
                    description: "warning is not successful execution",
                    outcome: RunOutcome::Warn,
                    nodes: [NodeOutcome::Warn, NodeOutcome::Success],
                    started: [true; 2],
                    expected_selector: "warn",
                    expected_filters: &["orders_completed"],
                },
                SensorExecutionTestCase {
                    description: "start without success cannot borrow success from another run",
                    outcome: RunOutcome::Failed,
                    nodes: [NodeOutcome::Failed; 2],
                    started: [true; 2],
                    expected_selector: "failed",
                    expected_filters: &[],
                },
                SensorExecutionTestCase {
                    description: "skipped selected node is not successful",
                    outcome: RunOutcome::Partial,
                    nodes: [NodeOutcome::Skipped, NodeOutcome::Success],
                    started: [false, true],
                    expected_selector: "partial",
                    expected_filters: &["orders_completed"],
                },
                SensorExecutionTestCase {
                    description: "a partial run's freshness-only node still does not qualify",
                    outcome: RunOutcome::Partial,
                    nodes: [NodeOutcome::Success, NodeOutcome::Failed],
                    started: [false, true],
                    expected_selector: "partial",
                    expected_filters: &[],
                },
                SensorExecutionTestCase {
                    description: "a genuinely started warning is not success",
                    outcome: RunOutcome::Warn,
                    nodes: [NodeOutcome::Warn, NodeOutcome::Success],
                    started: [true, false],
                    expected_selector: "warn",
                    expected_filters: &[],
                },
            ];
            let mut source_ids: BTreeMap<(&str, &str), Uuid> = BTreeMap::new();
            for execution in executions {
                for job_key in ["orders", "daily_orders"] {
                    let id = finish_run(
                        store,
                        &lease,
                        (location, "development"),
                        job_key,
                        execution.outcome,
                        execution.nodes,
                        execution.started,
                    )
                    .await;
                    source_ids.insert((execution.description, job_key), id);
                    let transaction = begin(store, TransactionMode::Read)
                        .await
                        .expect("read lineage");
                    let (transaction, children) = query(
                        transaction,
                        "SELECT run_id FROM sqb_runs WHERE parent_run_id=$1",
                        vec![Parameter::Id(id)],
                    )
                    .await
                    .expect(execution.description);
                    commit(transaction).await.expect("read lineage committed");
                    for (source, id) in std::iter::once((job_key, id.to_string())).chain(
                        children.into_iter().map(|row| {
                            (
                                "orders",
                                row["run_id"].as_str().expect("child id").to_owned(),
                            )
                        }),
                    ) {
                        for kind in ["completed", execution.expected_selector] {
                            expected
                                .entry(format!("{source}_{kind}"))
                                .or_default()
                                .insert(id.clone());
                        }
                        for filter in execution.expected_filters {
                            expected
                                .entry(format!("{source}_executed_{filter}"))
                                .or_default()
                                .insert(id.clone());
                        }
                    }
                }
            }
            let retry = RerunRequest {
                run_id: Uuid::now_v7(),
                source_run_id: source_ids[&(
                    "partial outcome is independent of selected-node success",
                    "daily_orders",
                )],
                from_failure: true,
                reason: "Retry only failed orders without borrowing earlier customer success"
                    .into(),
                priority: 10,
            };
            let context = AuditContext {
                actor: "test-admin".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            assert!(
                request_rerun(store, &context, &retry)
                    .await
                    .expect("request rerun")
            );
            assert!(
                !request_rerun(store, &context, &retry)
                    .await
                    .expect("retry request")
            );
            finish_claimed_run(
                store,
                &lease,
                (location, "development"),
                retry.run_id,
                RunOutcome::Success,
                [NodeOutcome::Success; 2],
                [true; 2],
            )
            .await;
            let transaction = begin(store, TransactionMode::Read)
                .await
                .expect("read rerun lineage");
            let (transaction, children) = query(
                transaction,
                "SELECT run_id FROM sqb_runs WHERE parent_run_id=$1",
                vec![Parameter::Id(retry.run_id)],
            )
            .await
            .expect("rerun child");
            commit(transaction).await.expect("read rerun committed");
            for (source, id) in std::iter::once(("daily_orders", retry.run_id.to_string())).chain(
                children.into_iter().map(|row| {
                    (
                        "orders",
                        row["run_id"].as_str().expect("child id").to_owned(),
                    )
                }),
            ) {
                for suffix in ["completed", "success", "executed_orders_completed"] {
                    expected
                        .entry(format!("{source}_{suffix}"))
                        .or_default()
                        .insert(id.clone());
                }
            }
            let sources = list_runs(store, (location, "development"), 500)
                .await
                .expect("all source executions");
            // Same key in another target/location and a different job in this scope must not match.
            let mut other_input = input.clone();
            other_input.location_id = other;
            publish_snapshot(store, &other_input)
                .await
                .expect("other publication");
            other_input.location_id = location;
            other_input.target = "preview".into();
            publish_snapshot(store, &other_input)
                .await
                .expect("other target");
            for scope in [(other, "development"), (location, "preview")] {
                finish_run(
                    store,
                    &lease,
                    scope,
                    "orders",
                    RunOutcome::Success,
                    [NodeOutcome::Success; 2],
                    [false; 2],
                )
                .await;
            }
            finish_run(
                store,
                &lease,
                (location, "development"),
                "inventory",
                RunOutcome::Success,
                [NodeOutcome::Success; 2],
                [false; 2],
            )
            .await;
            // Recreate the exact pre-change index/migration state with populated run history.
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("upgrade fixture");
            let (transaction, _) =
                query(transaction, "DROP TABLE sqb_warehouse_fingerprints", vec![])
                    .await
                    .expect("remove subsequent fingerprint mirror migration");
            let (transaction, _) = query(transaction, "DROP TABLE sqb_warehouse_freshness", vec![])
                .await
                .expect("remove subsequent warehouse mirror migration");
            let (transaction, _) =
                query(transaction, "DROP INDEX sqb_node_finished_cursor", vec![])
                    .await
                    .expect("remove node finish cursor index");
            let (transaction, _) =
                query(transaction, "DROP INDEX sqb_node_started_identity", vec![])
                    .await
                    .expect("remove node start evidence index");
            let (transaction, _) = query(transaction, "DROP INDEX sqb_runs_parent", vec![])
                .await
                .expect("remove execution lookup index");
            let (transaction, _) =
                query(transaction, "DROP INDEX sqb_runs_terminal_cursor", vec![])
                    .await
                    .expect("remove new index");
            let (transaction, _) = query(
                transaction,
                "DELETE FROM sqb_schema_migrations WHERE version>=6",
                vec![],
            )
            .await
            .expect("remove new migration record");
            commit(transaction).await.expect("pre-change schema ready");
            migrate_store(store)
                .await
                .expect("upgrade populated prior schema");
            let replica = second_store(store).await;
            let fault = TransactionFault {
                description: "fail atomic sensor cursor",
                table: "sqb_sensor_cursor_commits",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT, 'injected sensor failure')",
                postgres_body: "RAISE EXCEPTION 'injected sensor failure'",
            };
            let triggers = list_triggers(store, location, "development")
                .await
                .expect("triggers");
            for trigger in &triggers {
                let requested = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                    .await
                    .expect("request")
                    .expect("due");
                assert_eq!(
                    begin_sensor_evaluation(&replica, &lease, trigger.trigger_id)
                        .await
                        .expect("recover"),
                    Some(requested.clone())
                );
                install_fault(store, &fault).await;
                assert!(
                    complete_builtin_sensor(store, &lease, &requested)
                        .await
                        .is_err()
                );
                remove_fault(store, &fault).await;
                let wanted = expected.get(&trigger.name).cloned().unwrap_or_default();
                assert_eq!(
                    complete_builtin_sensor(&replica, &lease, &requested)
                        .await
                        .expect("retry"),
                    wanted.len(),
                    "{}",
                    trigger.name
                );
                assert_eq!(
                    complete_builtin_sensor(store, &lease, &requested)
                        .await
                        .expect("duplicate completion"),
                    0
                );
                let transaction = begin(store, TransactionMode::Read)
                    .await
                    .expect("read requests");
                let (transaction, rows) = query(
                    transaction,
                    "SELECT run_key FROM sqb_run_requests WHERE trigger_id=$1",
                    vec![Parameter::Id(trigger.trigger_id)],
                )
                .await
                .expect("keys");
                commit(transaction).await.expect("read commit");
                let actual: BTreeSet<String> = rows
                    .iter()
                    .map(|row| row["run_key"].as_str().expect("run key").into())
                    .collect();
                assert_eq!(actual, wanted, "{}", trigger.name);
                change(store, trigger.trigger_id, TriggerChange::ResetCursor).await;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            for trigger in &triggers {
                let replay = begin_sensor_evaluation(store, &lease, trigger.trigger_id)
                    .await
                    .expect("reset request")
                    .expect("due");
                complete_builtin_sensor(store, &lease, &replay)
                    .await
                    .expect("replay deduplicates");
            }
            let after = list_runs(store, (location, "development"), 500)
                .await
                .expect("queued sensors");
            assert_eq!(
                after.len(),
                sources.len() + 1 + test_case.expected_runs,
                "{}: reset adds no runs",
                test_case.description
            );
            for source in sources {
                assert_eq!(
                    after.iter().find(|record| record.run_id == source.run_id),
                    Some(&source),
                    "sensor never changes source state"
                );
            }
            close_store(replica).await;
            close_test_store(fixture).await;
        });
    }
}
