//! Terminal retries retain their snapshot and independent limits without repeating successful work.

use std::collections::{BTreeMap, BTreeSet};

use crate::credentials::models::AuditContext;
use crate::runs::main::{
    advance_groups::advance_groups, change_run::change_run, claim_next_run::claim_next_run,
    list_runs::list_runs, request_job_run::request_job_run, request_rerun::request_rerun,
};
use crate::runs::models::{
    JobRunRequest, NodeOutcome, RerunRequest, RunAuthority, RunChange, RunChangeRequest, RunOrigin,
    RunOutcome, RunStatus, RunWindow,
};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::runs::tests::claim_helpers::{slot_counts, succeed};
use crate::runs::tests::helpers::snapshot;
use crate::runs::tests::test_types::{
    RerunExecutionTestCase, RunLifecycleTestCase, TransactionFault,
};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

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
            input.documents.insert("jobs".into(), json!({
                "staging": {"description": "Stage orders", "selection": ["orders", "customers"], "concurrency_groups": ["warehouse"], "settings": {"vars": {"batch": 23}}},
                "marts": {"description": "Build inventory", "selection": ["inventory"], "concurrency_groups": ["inventory_limit"]},
                "completed": {"description": "Build products", "selection": ["products"]}
            }));
            input.documents.insert("groups".into(), json!({"daily_orders": {
                "description": "Build daily orders", "selection": ["customers", "orders", "inventory", "products"],
                "concurrency_groups": ["warehouse"], "jobs": {"staging": [], "marts": ["completed"], "completed": ["staging"]}
            }}));
            let published = publish_snapshot(store, &input).await.expect("publish");
            let source_id = Uuid::now_v7();
            request_job_run(
                store,
                (&context, None),
                &JobRunRequest {
                    run_id: source_id,
                    location_id: location,
                    target: "development".into(),
                    job_key: "daily_orders".into(),
                    reason: "Build orders".into(),
                    priority: 1,
                    window: Some(RunWindow::Integer { start: -3, end: 11 }),
                    full_refresh: false,
                    select: Vec::new(),
                    exclude: Vec::new(),
                },
            )
            .await
            .expect("request group");
            let retry = RerunRequest {
                run_id: Uuid::from_u128(1),
                source_run_id: source_id,
                from_failure: true,
                reason: "Retry failed orders".into(),
                priority: 7,
            };
            assert!(
                request_rerun(store, (&context, None), &retry)
                    .await
                    .is_err(),
                "a live source cannot be rerun"
            );
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::from([
                (
                    "warehouse".into(),
                    std::num::NonZeroU32::new(1).expect("positive capacity"),
                ),
                (
                    "inventory_limit".into(),
                    std::num::NonZeroU32::new(1).expect("positive capacity"),
                ),
            ]);
            let scope = (location, "development");
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("admit source");
            let node_outcomes = BTreeMap::from([
                ("products", NodeOutcome::Success),
                ("orders", NodeOutcome::Failed),
                ("customers", NodeOutcome::Cancelled),
                ("inventory", NodeOutcome::Skipped),
            ]);
            let job_outcomes = BTreeMap::from([
                ("staging", RunOutcome::Failed),
                ("completed", RunOutcome::Success),
                ("marts", RunOutcome::Failed),
            ]);
            for _ in 0..3 {
                let claim = claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim source")
                    .expect("ready child");
                let mut changes = vec![RunChange::Started];
                for node in &claim.request.selection {
                    let outcome = node_outcomes[node.as_str()];
                    changes.push(RunChange::NodeFinished {
                        node_key: node.clone(),
                        outcome,
                        rows_affected: None,
                        error_code: None,
                        skipped_because_of: (outcome == NodeOutcome::Skipped)
                            .then(|| "orders".into()),
                        error_message: None,
                    });
                }
                changes.push(RunChange::Finished {
                    outcome: job_outcomes[claim.request.job_key.as_deref().expect("source job")],
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
                    .expect("list completed source")
                    .iter()
                    .find(|run| run.run_id == source_id)
                    .expect("source exists")
                    .status,
                RunStatus::Partial
            );
            input.generation = 2;
            input.image_id = Some("sha256:synthetic-orders-image-2".into());
            input.documents.get_mut("jobs").expect("job declarations")["staging"]["settings"] =
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
            assert!(
                request_rerun(store, (&context, None), &retry)
                    .await
                    .is_err()
            );
            remove_fault(store, &fault).await;
            assert_eq!(
                list_runs(store, scope, 20)
                    .await
                    .expect("list rolled-back runs")
                    .len(),
                4,
                "audit failure rolls back parent and all children"
            );
            let (first, again) = tokio::join!(
                request_rerun(store, (&context, None), &retry),
                request_rerun(store, (&context, None), &retry)
            );
            assert_ne!(first.expect("rerun"), again.expect("replica retry"));
            let changed = RerunRequest {
                from_failure: false,
                ..retry.clone()
            };
            assert!(
                request_rerun(store, (&context, None), &changed)
                    .await
                    .is_err()
            );
            let other = AuditContext {
                actor: "other-operator".into(),
                ..context.clone()
            };
            assert!(request_rerun(store, (&other, None), &retry).await.is_err());
            let settings = BTreeMap::from([
                (
                    "staging",
                    BTreeMap::from([("vars".into(), json!({"batch": 23}))]),
                ),
                ("completed", BTreeMap::new()),
                ("marts", BTreeMap::new()),
            ]);
            for execution in [
                RerunExecutionTestCase {
                    description: "failure retry omits successful and cancelled nodes",
                    request: retry.clone(),
                    expected_applied: false,
                    expected_selection: ["orders", "inventory"].map(str::to_owned).into(),
                    expected_jobs: &["staging", "marts"],
                },
                RerunExecutionTestCase {
                    description: "full retry retains all original nodes",
                    request: RerunRequest {
                        run_id: Uuid::now_v7(),
                        from_failure: false,
                        ..retry.clone()
                    },
                    expected_applied: true,
                    expected_selection: ["orders", "inventory", "customers", "products"]
                        .map(str::to_owned)
                        .into(),
                    expected_jobs: &["staging", "completed", "marts"],
                },
            ] {
                let request = execution.request;
                assert_eq!(
                    request_rerun(store, (&context, None), &request)
                        .await
                        .expect("publish rerun"),
                    execution.expected_applied,
                    "{}",
                    execution.description
                );
                let runs = list_runs(store, scope, 20).await.expect("list reruns");
                assert!(
                    runs.iter()
                        .filter(|run| {
                            run.run_id == request.run_id
                                || run.parent_run_id == Some(request.run_id)
                        })
                        .all(|run| run.rewrite_history),
                    "{}: a backfill's rerun and its children record that they rewrite history",
                    execution.description
                );
                assert_eq!(
                    runs.iter()
                        .find(|run| run.run_id == request.run_id)
                        .expect("rerun exists")
                        .selection_json,
                    execution.expected_selection
                );
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("admit rerun");
                let mut selected: BTreeSet<String> = BTreeSet::new();
                for expected_job in execution.expected_jobs {
                    let claim = claim_next_run(store, &lease, &limits, Some(scope))
                        .await
                        .expect("claim rerun")
                        .expect("ready retry child");
                    assert_eq!(
                        claim.request.job_key.as_deref(),
                        Some(*expected_job),
                        "a completed intermediate job must not erase its ancestor's wait boundary"
                    );
                    assert_eq!(claim.request.snapshot_id, published.snapshot_id);
                    assert_eq!(
                        claim.request.image_id,
                        Some("sha256:synthetic-orders-image".into())
                    );
                    assert_eq!(claim.request.origin, RunOrigin::Rerun);
                    assert_eq!(claim.request.priority, 7);
                    assert_eq!(claim.request.settings, settings[expected_job]);
                    assert_eq!(
                        claim.request.window,
                        Some(RunWindow::Integer { start: -3, end: 11 })
                    );
                    selected.extend(claim.request.selection.iter().cloned());
                    succeed(store, &claim).await;
                }
                assert_eq!(selected, execution.expected_selection);
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("finish rerun");
                assert_eq!(
                    list_runs(store, scope, 20)
                        .await
                        .expect("list finished reruns")
                        .iter()
                        .find(|run| run.run_id == request.run_id)
                        .expect("finished rerun exists")
                        .status,
                    test_case.expected_status,
                    "{}",
                    test_case.description
                );
                assert!(slot_counts(store).await.is_empty());
                assert!(
                    !request_rerun(store, (&context, None), &request)
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
                    request_rerun(store, (&context, None), &successful)
                        .await
                        .is_err(),
                    "successful work has nothing to retry from failure"
                );
            }
            close_test_store(fixture).await;
        });
    }
}
