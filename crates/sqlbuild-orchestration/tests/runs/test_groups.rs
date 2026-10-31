//! Pinned groups coordinate real transactions, inherited slots, cancellation and replay.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use serde_json::json;
use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::runs::main::{
    advance_groups::advance_groups, change_run::change_run, claim_next_run::claim_next_run,
    list_runs::list_runs, request_job_run::request_job_run, run_execution::run_execution,
};
use sqlbuild_orchestration::runs::models::{
    JobRunRequest, NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunOutcome, RunStatus,
};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::{
    migrate_store::migrate_store, read_recall_events::read_recall_events,
};
use uuid::Uuid;

use crate::claim_helpers::{slot_counts, succeed};
use crate::fault_helpers::{install_fault, remove_fault};
use crate::helpers::snapshot;
use crate::location_helpers::create_location;
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::{RunLifecycleTestCase, TransactionFault};

#[test]
fn given_a_pinned_group_when_redeployed_and_cancelled_then_children_keep_scope_and_parent_waits_for_terminal_evidence()
 {
    let test_cases = [
        RunLifecycleTestCase {
            description: "SQLite group cancellation and replay",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Cancelled,
        },
        RunLifecycleTestCase {
            description: "PostgreSQL group cancellation and replay",
            backend: Backend::Postgres,
            expected_status: RunStatus::Cancelled,
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
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let mut input = snapshot(location);
            input.nodes.get_mut("orders").expect("orders").definition["deps"] =
                json!(["customers"]);
            input.documents.insert("jobs".into(), json!({
                "staging": {"description": "Stage customers", "selection": ["customers"], "concurrency_groups": ["warehouse"]},
                "marts": {"description": "Build orders", "selection": ["orders"], "concurrency_groups": ["warehouse"]}
            }));
            input.documents.insert("groups".into(), json!({"daily_orders": {
                "description": "Stage then build orders", "selection": ["customers", "orders"],
                "concurrency_groups": ["warehouse"], "jobs": {"staging": [], "marts": ["staging"]}
            }}));
            let published = publish_snapshot(store, &input)
                .await
                .expect("publish group");
            let request = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                job_key: "daily_orders".into(),
                reason: "Build the group".into(),
                priority: 1,
            };
            assert!(
                request_job_run(store, &context, &request)
                    .await
                    .expect("request group")
            );
            assert!(
                !request_job_run(store, &context, &request)
                    .await
                    .expect("retry group")
            );
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits =
                BTreeMap::from([("warehouse".into(), NonZeroU32::new(1).expect("one slot"))]);
            let scope = (location, "development");
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("start virtual parent");
            let first = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim staging")
                .expect("staging ready");
            assert_eq!(first.request.job_key.as_deref(), Some("staging"));
            assert_eq!(first.request.parent_run_id, Some(request.run_id));
            assert_eq!(first.request.write_set, ["customers".to_owned()].into());
            assert!(
                claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("dependency waits")
                    .is_none()
            );
            let authority = RunAuthority::Worker {
                run_id: first.request.run_id,
                claim_id: first.claim_id,
            };
            for change in [
                RunChange::Started,
                RunChange::NodeFinished {
                    node_key: "customers".into(),
                    outcome: NodeOutcome::Success,
                    rows_affected: Some(11),
                    error_code: None,
                    skipped_because_of: None,
                },
                RunChange::Finished {
                    outcome: RunOutcome::Success,
                    reason: "staging completed".into(),
                    exit_code: Some(0),
                },
            ] {
                change_run(
                    store,
                    authority,
                    &RunChangeRequest {
                        event_id: Uuid::now_v7(),
                        run_id: first.request.run_id,
                        claim_id: Some(first.claim_id),
                        change,
                    },
                )
                .await
                .expect("staging fact");
            }
            input.generation = 2;
            input.documents.get_mut("jobs").expect("jobs")["marts"]["selection"] =
                json!(["inventory"]);
            input.documents.get_mut("groups").expect("groups")["daily_orders"]["selection"] =
                json!(["customers", "inventory"]);
            publish_snapshot(store, &input)
                .await
                .expect("redeploy different membership");
            let second = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim marts")
                .expect("marts ready");
            assert_eq!(second.request.snapshot_id, published.snapshot_id);
            assert_eq!(second.request.selection, ["orders".to_owned()].into());
            assert_eq!(second.request.write_set, second.request.selection);
            assert!(
                run_execution(store, (second.request.run_id, second.claim_id))
                    .await
                    .expect("execution")
                    .expect("pinned")
                    .skipped_nodes
                    .is_empty()
            );
            let authority = RunAuthority::Worker {
                run_id: second.request.run_id,
                claim_id: second.claim_id,
            };
            change_run(
                store,
                authority,
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: second.request.run_id,
                    claim_id: Some(second.claim_id),
                    change: RunChange::Started,
                },
            )
            .await
            .expect("start marts");
            change_run(
                store,
                RunAuthority::Operator(&context),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: request.run_id,
                    claim_id: None,
                    change: RunChange::CancelRequested {
                        actor: context.actor.clone(),
                        reason: "Cancel the group".into(),
                    },
                },
            )
            .await
            .expect("cancel parent");
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("propagate cancellation");
            let runs = list_runs(store, scope, 100).await.expect("runs");
            assert_eq!(
                runs.iter()
                    .find(|run| run.run_id == request.run_id)
                    .expect("parent")
                    .status,
                RunStatus::Running,
                "{}",
                test_case.description
            );
            assert_eq!(
                runs.iter()
                    .find(|run| run.run_id == second.request.run_id)
                    .expect("active child")
                    .status,
                RunStatus::Running
            );
            let standalone = JobRunRequest {
                run_id: Uuid::now_v7(),
                job_key: "staging".into(),
                ..request.clone()
            };
            request_job_run(store, &context, &standalone)
                .await
                .expect("standalone request");
            assert!(
                claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("parent retains its slot")
                    .is_none()
            );
            change_run(
                store,
                authority,
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: second.request.run_id,
                    claim_id: Some(second.claim_id),
                    change: RunChange::Finished {
                        outcome: RunOutcome::Cancelled,
                        reason: "worker confirmed its executor stopped".into(),
                        exit_code: Some(143),
                    },
                },
            )
            .await
            .expect("physical child finished");
            let fault = TransactionFault {
                description: "group terminal publication rolls back",
                table: "recall_events",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected group publication failure')",
                postgres_body: "RAISE EXCEPTION 'injected group publication failure'",
            };
            install_fault(store, &fault).await;
            assert!(
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .is_err()
            );
            remove_fault(store, &fault).await;
            for _ in 0..2 {
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("retry group completion");
            }
            let runs = list_runs(store, scope, 100).await.expect("completed runs");
            assert_eq!(
                runs.iter()
                    .find(|run| run.run_id == request.run_id)
                    .expect("parent")
                    .status,
                test_case.expected_status
            );
            let standalone_claim = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("slot released")
                .expect("standalone staging ready");
            assert_eq!(standalone_claim.request.run_id, standalone.run_id);
            assert_eq!(standalone_claim.request.parent_run_id, None);
            let events = read_recall_events(store, 0, 1000).await.expect("facts");
            assert_eq!(
                events
                    .iter()
                    .filter(|event| event["event_type"] == "run_terminated"
                        && event["invocation_id"] == format!("scheduler/{}", request.run_id))
                    .count(),
                1
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_opposing_parent_and_child_limits_when_advancing_groups_then_both_groups_finish_without_exceeding_limits()
 {
    let test_cases = [
        RunLifecycleTestCase {
            description: "SQLite opposing group reservations",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Running,
        },
        RunLifecycleTestCase {
            description: "PostgreSQL opposing group reservations",
            backend: Backend::Postgres,
            expected_status: RunStatus::Running,
        },
    ];
    let mut completed = Vec::new();
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
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let mut input = snapshot(location);
            input.documents.insert("jobs".into(), json!({
                "orders_job": {"description": "Build orders", "selection": ["orders"], "concurrency_groups": ["inventory_limit"]},
                "inventory_job": {"description": "Build inventory", "selection": ["customers"], "concurrency_groups": ["orders_limit"]}
            }));
            input.documents.insert("groups".into(), json!({
                "orders_group": {"description": "Orders group", "selection": ["orders"], "concurrency_groups": ["orders_limit"], "jobs": {"orders_job": []}},
                "inventory_group": {"description": "Inventory group", "selection": ["customers"], "concurrency_groups": ["inventory_limit"], "jobs": {"inventory_job": []}}
            }));
            publish_snapshot(store, &input)
                .await
                .expect("publish groups");
            for job_key in ["orders_group", "inventory_group"] {
                request_job_run(
                    store,
                    &context,
                    &JobRunRequest {
                        run_id: Uuid::now_v7(),
                        location_id: location,
                        target: "development".into(),
                        job_key: job_key.into(),
                        reason: "Build independent groups".into(),
                        priority: 1,
                    },
                )
                .await
                .expect("request group");
            }
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::from([
                ("orders_limit".into(), NonZeroU32::new(1).expect("one slot")),
                (
                    "inventory_limit".into(),
                    NonZeroU32::new(1).expect("one slot"),
                ),
            ]);
            let scope = (location, "development");
            for _ in 0..2 {
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("advance groups");
            }
            let runs = list_runs(store, scope, 100).await.expect("runs");
            assert_eq!(runs.len(), 4, "{}", test_case.description);
            assert_eq!(
                runs.iter()
                    .filter(|run| run.status == test_case.expected_status)
                    .count(),
                1,
                "{}",
                test_case.description
            );
            for _ in 0..2 {
                let child = claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim child")
                    .expect("a group must make progress");
                let slots = slot_counts(store).await;
                assert_eq!(
                    slots.len(),
                    2,
                    "child-only slot is acquired only for its worker"
                );
                assert!(slots.values().all(|occupied| *occupied == 1), "{slots:?}");
                assert!(
                    claim_next_run(store, &lease, &limits, Some(scope))
                        .await
                        .expect("held child")
                        .is_none()
                );
                succeed(store, &child).await;
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("complete parent and retry waiting group");
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("retry independent of run-id ordering");
            }
            let runs = list_runs(store, scope, 100).await.expect("completed runs");
            assert!(
                runs.iter().all(|run| run.status == RunStatus::Success),
                "{runs:?}"
            );
            completed.push(test_case.description);
            close_test_store(fixture).await;
        });
    }
    assert_eq!(completed.len(), 2, "both backends must run");
}
