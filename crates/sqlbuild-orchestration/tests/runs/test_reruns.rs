//! Terminal retries retain their snapshot and independent limits without repeating successful work.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::runs::main::{
    advance_groups::advance_groups, change_run::change_run, claim_next_run::claim_next_run,
    list_runs::list_runs, request_job_run::request_job_run, request_rerun::request_rerun,
};
use sqlbuild_orchestration::runs::models::{
    JobRunRequest, NodeOutcome, RerunRequest, RunAuthority, RunChange, RunChangeRequest, RunOrigin,
    RunOutcome, RunStatus,
};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::claim_helpers::{slot_counts, succeed};
use crate::fault_helpers::{install_fault, remove_fault};
use crate::helpers::snapshot;
use crate::location_helpers::create_location;
use crate::store_helpers::{close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::{RunLifecycleTestCase, TransactionFault};

#[test]
fn given_a_partial_group_when_rerun_after_redeploy_then_only_failed_and_skipped_nodes_retry_atomically()
 {
    let test_cases = [
        RunLifecycleTestCase {
            description: "SQLite pinned group rerun",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Success,
        },
        RunLifecycleTestCase {
            description: "PostgreSQL pinned group rerun",
            backend: Backend::Postgres,
            expected_status: RunStatus::Success,
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
            input
                .nodes
                .get_mut("inventory")
                .expect("inventory")
                .definition["deps"] = json!(["orders"]);
            input.documents.insert("jobs".into(), json!({
                "staging": {"description": "Stage orders", "selection": ["orders", "customers"], "concurrency_groups": ["warehouse"], "settings": {"vars": {"batch": 23}}},
                "marts": {"description": "Build inventory", "selection": ["inventory"], "concurrency_groups": ["inventory_limit"]},
                "completed": {"description": "Build products", "selection": ["products"]}
            }));
            input.documents.insert("groups".into(), json!({"daily_orders": {
                "description": "Build daily orders", "selection": ["customers", "orders", "inventory", "products"],
                "concurrency_groups": ["warehouse"], "jobs": {"staging": [], "marts": ["staging"], "completed": []}
            }}));
            let published = publish_snapshot(store, &input).await.expect("publish");
            let source_id = Uuid::now_v7();
            request_job_run(
                store,
                &context,
                &JobRunRequest {
                    run_id: source_id,
                    location_id: location,
                    target: "development".into(),
                    job_key: "daily_orders".into(),
                    reason: "Build orders".into(),
                    priority: 1,
                },
            )
            .await
            .expect("request group");
            let retry = RerunRequest {
                run_id: Uuid::now_v7(),
                source_run_id: source_id,
                from_failure: true,
                reason: "Retry failed orders".into(),
                priority: 7,
            };
            assert!(
                request_rerun(store, &context, &retry).await.is_err(),
                "a live source cannot be rerun"
            );
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::from([
                ("warehouse".into(), std::num::NonZeroU32::new(1).unwrap()),
                (
                    "inventory_limit".into(),
                    std::num::NonZeroU32::new(1).unwrap(),
                ),
            ]);
            let scope = (location, "development");
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("admit source");
            for _ in 0..3 {
                let claim = claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim source")
                    .expect("ready child");
                let mut changes = vec![RunChange::Started];
                for node in &claim.request.selection {
                    let outcome = match node.as_str() {
                        "products" => NodeOutcome::Success,
                        "orders" => NodeOutcome::Failed,
                        "customers" => NodeOutcome::Cancelled,
                        _ => NodeOutcome::Skipped,
                    };
                    changes.push(RunChange::NodeFinished {
                        node_key: node.clone(),
                        outcome,
                        rows_affected: None,
                        error_code: None,
                        skipped_because_of: (outcome == NodeOutcome::Skipped)
                            .then(|| "orders".into()),
                    });
                }
                changes.push(RunChange::Finished {
                    outcome: if claim.request.selection.contains("products") {
                        RunOutcome::Success
                    } else {
                        RunOutcome::Failed
                    },
                    reason: "synthetic source outcome".into(),
                    exit_code: Some(1),
                });
                for change in changes {
                    change_run(
                        store,
                        RunAuthority::Worker {
                            run_id: claim.request.run_id,
                            claim_id: claim.claim_id,
                        },
                        &RunChangeRequest {
                            event_id: Uuid::now_v7(),
                            run_id: claim.request.run_id,
                            claim_id: Some(claim.claim_id),
                            change,
                        },
                    )
                    .await
                    .expect("source worker fact");
                }
            }
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("finish source");
            assert_eq!(
                list_runs(store, scope, 20)
                    .await
                    .unwrap()
                    .iter()
                    .find(|run| run.run_id == source_id)
                    .unwrap()
                    .status,
                RunStatus::Partial
            );
            input.generation = 2;
            input.image_id = Some("sha256:synthetic-orders-image-2".into());
            input.documents.get_mut("jobs").unwrap()["staging"]["settings"] =
                json!({"vars": {"batch": 91}});
            publish_snapshot(store, &input).await.expect("redeploy");
            let fault = TransactionFault {
                description: "rerun audit failure",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected audit failure')",
                postgres_body: "RAISE EXCEPTION 'injected audit failure'",
            };
            install_fault(store, &fault).await;
            assert!(request_rerun(store, &context, &retry).await.is_err());
            remove_fault(store, &fault).await;
            assert_eq!(
                list_runs(store, scope, 20).await.unwrap().len(),
                4,
                "audit failure rolls back parent and all children"
            );
            let (first, again) = tokio::join!(
                request_rerun(store, &context, &retry),
                request_rerun(store, &context, &retry)
            );
            assert_ne!(first.expect("rerun"), again.expect("replica retry"));
            let changed = RerunRequest {
                from_failure: false,
                ..retry.clone()
            };
            assert!(request_rerun(store, &context, &changed).await.is_err());
            let other = AuditContext {
                actor: "other-operator".into(),
                ..context.clone()
            };
            assert!(request_rerun(store, &other, &retry).await.is_err());
            for request in [
                retry.clone(),
                RerunRequest {
                    run_id: Uuid::now_v7(),
                    from_failure: false,
                    ..retry.clone()
                },
            ] {
                if !request.from_failure {
                    assert!(request_rerun(store, &context, &request).await.unwrap());
                }
                let expected: BTreeSet<String> = if request.from_failure {
                    ["orders", "inventory"].map(str::to_owned).into()
                } else {
                    ["orders", "inventory", "customers", "products"]
                        .map(str::to_owned)
                        .into()
                };
                let runs = list_runs(store, scope, 20).await.unwrap();
                assert_eq!(
                    runs.iter()
                        .find(|run| run.run_id == request.run_id)
                        .unwrap()
                        .selection_json,
                    expected
                );
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("admit rerun");
                let mut selected = BTreeSet::new();
                for _ in 0..if request.from_failure { 2 } else { 3 } {
                    let claim = claim_next_run(store, &lease, &limits, Some(scope))
                        .await
                        .expect("claim rerun")
                        .expect("ready retry child");
                    assert_eq!(claim.request.snapshot_id, published.snapshot_id);
                    assert_eq!(
                        claim.request.image_id,
                        Some("sha256:synthetic-orders-image".into())
                    );
                    assert_eq!(claim.request.origin, RunOrigin::Rerun);
                    assert_eq!(claim.request.priority, 7);
                    if claim.request.job_key.as_deref() == Some("staging") {
                        assert_eq!(
                            claim.request.settings,
                            BTreeMap::from([("vars".into(), json!({"batch": 23}))])
                        );
                    }
                    selected.extend(claim.request.selection.iter().cloned());
                    succeed(store, &claim).await;
                }
                assert_eq!(selected, expected);
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("finish rerun");
                assert_eq!(
                    list_runs(store, scope, 20)
                        .await
                        .unwrap()
                        .iter()
                        .find(|run| run.run_id == request.run_id)
                        .unwrap()
                        .status,
                    test_case.expected_status,
                    "{}",
                    test_case.description
                );
                assert!(slot_counts(store).await.is_empty());
                assert!(
                    !request_rerun(store, &context, &request)
                        .await
                        .expect("completed transport retry")
                );
                let successful = RerunRequest {
                    run_id: Uuid::now_v7(),
                    source_run_id: request.run_id,
                    from_failure: true,
                    ..request.clone()
                };
                assert!(
                    request_rerun(store, &context, &successful).await.is_err(),
                    "successful work has nothing to retry from failure"
                );
            }
            close_test_store(fixture).await;
        });
    }
}
