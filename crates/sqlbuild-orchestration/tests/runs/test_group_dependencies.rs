//! A group's `[dependencies] on_failure` decides what each member does after a job it depends on fails.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::{
    advance_groups::advance_groups, claim_next_run::claim_next_run, list_runs::list_runs,
    request_job_run::request_job_run, run_execution::run_execution,
};
use crate::runs::models::{JobRunRequest, NodeOutcome, RunOutcome, RunStatus};
use crate::runs::tests::claim_helpers::finish;
use crate::runs::tests::helpers::{rows, snapshot};
use crate::runs::tests::test_types::{GroupDependencyTestCase, MemberReaction};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_failed_member_when_its_dependents_react_then_each_dependency_policy_and_override_applies()
 {
    let partial = || MemberReaction::Runs(vec![("orders", "customers")]);
    let test_cases = [
        (Backend::Sqlite, "SQLite"),
        (Backend::Postgres, "PostgreSQL"),
    ]
    .into_iter()
    .flat_map(|(backend, name)| {
        [
            GroupDependencyTestCase {
                description: "partial by default skips only downstream nodes",
                backend,
                dependency_failure: json!({}),
                expected_marts: partial(),
                expected_inventory: MemberReaction::Runs(vec![("inventory", "customers")]),
            },
            GroupDependencyTestCase {
                description: "skip ends dependents without running them",
                backend,
                dependency_failure: json!({"marts_job": "skip", "inventory_job": "skip"}),
                expected_marts: MemberReaction::Skipped,
                expected_inventory: MemberReaction::Skipped,
            },
            GroupDependencyTestCase {
                description: "run builds every node regardless",
                backend,
                dependency_failure: json!({"marts_job": "run", "inventory_job": "run"}),
                expected_marts: MemberReaction::Runs(Vec::new()),
                expected_inventory: MemberReaction::Runs(Vec::new()),
            },
            GroupDependencyTestCase {
                description: "a member override differs from the group default",
                backend,
                dependency_failure: json!({"inventory_job": "skip"}),
                expected_marts: partial(),
                expected_inventory: MemberReaction::Skipped,
            },
        ]
        .map(|case| (name, case))
    });
    let mut completed = 0;
    for (backend_name, test_case) in test_cases {
        let description = format!("{backend_name}: {}", test_case.description);
        run(async {
            let fixture = open_test_store(test_case.backend)
                .await
                .expect("required backend");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let mut input = snapshot(location);
            for node in ["orders", "inventory"] {
                input.nodes.get_mut(node).expect("node").definition["deps"] = json!(["customers"]);
            }
            input.documents.insert(
                "jobs".into(),
                json!({
                    "staging_job": {"description": "Stage customers", "selection": ["customers"]},
                    "marts_job": {"description": "Build orders", "selection": ["orders"]},
                    "inventory_job": {"description": "Build inventory", "selection": ["inventory"]}
                }),
            );
            input.documents.insert("groups".into(), json!({"nightly_job_group": {
                "description": "Stage, then build orders and inventory",
                "selection": ["customers", "inventory", "orders"],
                "jobs": {"staging_job": [], "marts_job": ["staging_job"], "inventory_job": ["staging_job"]},
                "dependency_failure": test_case.dependency_failure
            }}));
            publish_snapshot(store, &input).await.expect("publish");
            let context = AuditContext {
                actor: "test-operator".into(),
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
            let lease = acquire_lease(store, Uuid::now_v7())
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
            assert!(
                claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("dependents wait")
                    .is_none(),
                "{description}: dependents wait for the job they read from"
            );
            finish(store, &staging, (NodeOutcome::Failed, RunOutcome::Failed)).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("react to the failure");

            let mut reactions: BTreeMap<String, MemberReaction> = BTreeMap::new();
            while let Some(claim) = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim dependent")
            {
                let execution = run_execution(store, (claim.request.run_id, claim.claim_id))
                    .await
                    .expect("execution")
                    .expect("claimed");
                let mut skipped: Vec<(&'static str, &'static str)> = execution
                    .skipped_nodes
                    .iter()
                    .map(|(node, cause)| {
                        (
                            ["orders", "inventory"]
                                .into_iter()
                                .find(|known| known == node)
                                .expect("known node"),
                            if cause == "customers" {
                                "customers"
                            } else {
                                "other"
                            },
                        )
                    })
                    .collect();
                skipped.sort_unstable();
                let outcome = if skipped.is_empty() {
                    (NodeOutcome::Success, RunOutcome::Success)
                } else {
                    (NodeOutcome::Skipped, RunOutcome::Failed)
                };
                finish(store, &claim, outcome).await;
                reactions.insert(
                    claim.request.job_key.clone().expect("member"),
                    MemberReaction::Runs(skipped),
                );
            }
            let runs = list_runs(store, scope, 100).await.expect("runs");
            for member in ["marts_job", "inventory_job"] {
                if reactions.contains_key(member) {
                    continue;
                }
                let record = runs
                    .iter()
                    .find(|record| record.job_key.as_deref() == Some(member))
                    .expect("member run");
                assert_eq!(
                    (record.status, record.launched_at),
                    (RunStatus::Skipped, None),
                    "{description}: {member} ends skipped without launching"
                );
                let nodes = rows(
                    store,
                    "SELECT status, skipped_because_of FROM sqb_run_nodes WHERE run_id=$1 AND kind='node'",
                    vec![Parameter::Id(record.run_id)],
                )
                .await;
                assert!(
                    nodes.iter().all(|node| node["status"] == "skipped"
                        && node["skipped_because_of"] == "customers"),
                    "{description}: {nodes:?}"
                );
                reactions.insert(member.into(), MemberReaction::Skipped);
            }
            assert_eq!(
                (&reactions["marts_job"], &reactions["inventory_job"]),
                (&test_case.expected_marts, &test_case.expected_inventory),
                "{description}"
            );
            for _ in 0..2 {
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("finish group");
            }
            let runs = list_runs(store, scope, 100).await.expect("runs");
            let parent = runs
                .iter()
                .find(|record| record.run_id == group)
                .expect("group");
            assert!(
                matches!(parent.status, RunStatus::Failed | RunStatus::Partial),
                "{description}: {:?}",
                parent.status
            );
            completed += 1;
            close_test_store(fixture).await;
        });
    }
    assert_eq!(completed, 8, "every case runs on both backends");
}
