//! A group's `[dependencies] on_failure` decides what each member does after a job it depends on fails.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::{
    execution::advance_groups::advance_groups, execution::claim_next_run::claim_next_run,
    listings::list_runs::list_runs, requests::request_job_run::request_job_run,
};
use crate::runs::models::{JobRunRequest, NodeOutcome, RunOutcome, RunStatus};
use crate::runs::tests::claim_helpers::{finish, member_reactions, publish_dependency_group};
use crate::runs::tests::test_types::{GroupDependencyTestCase, MemberReaction};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_failed_member_when_its_dependents_react_then_each_dependency_policy_and_override_applies()
 {
    let test_cases = [
        GroupDependencyTestCase {
            description: "SQLite: partial by default skips only downstream nodes",
            backend: Backend::Sqlite,
            dependency_failure: json!({}),
            expected_marts: MemberReaction::Runs(vec![("orders", "customers")]),
            expected_inventory: MemberReaction::Runs(vec![("inventory", "customers")]),
        },
        GroupDependencyTestCase {
            description: "SQLite: skip ends dependents without running them",
            backend: Backend::Sqlite,
            dependency_failure: json!({"marts_job": "skip", "inventory_job": "skip"}),
            expected_marts: MemberReaction::Skipped,
            expected_inventory: MemberReaction::Skipped,
        },
        GroupDependencyTestCase {
            description: "SQLite: run builds every node regardless",
            backend: Backend::Sqlite,
            dependency_failure: json!({"marts_job": "run", "inventory_job": "run"}),
            expected_marts: MemberReaction::Runs(Vec::new()),
            expected_inventory: MemberReaction::Runs(Vec::new()),
        },
        GroupDependencyTestCase {
            description: "SQLite: a member override differs from the group default",
            backend: Backend::Sqlite,
            dependency_failure: json!({"inventory_job": "skip"}),
            expected_marts: MemberReaction::Runs(vec![("orders", "customers")]),
            expected_inventory: MemberReaction::Skipped,
        },
        GroupDependencyTestCase {
            description: "PostgreSQL: partial by default skips only downstream nodes",
            backend: Backend::Postgres,
            dependency_failure: json!({}),
            expected_marts: MemberReaction::Runs(vec![("orders", "customers")]),
            expected_inventory: MemberReaction::Runs(vec![("inventory", "customers")]),
        },
        GroupDependencyTestCase {
            description: "PostgreSQL: skip ends dependents without running them",
            backend: Backend::Postgres,
            dependency_failure: json!({"marts_job": "skip", "inventory_job": "skip"}),
            expected_marts: MemberReaction::Skipped,
            expected_inventory: MemberReaction::Skipped,
        },
        GroupDependencyTestCase {
            description: "PostgreSQL: run builds every node regardless",
            backend: Backend::Postgres,
            dependency_failure: json!({"marts_job": "run", "inventory_job": "run"}),
            expected_marts: MemberReaction::Runs(Vec::new()),
            expected_inventory: MemberReaction::Runs(Vec::new()),
        },
        GroupDependencyTestCase {
            description: "PostgreSQL: a member override differs from the group default",
            backend: Backend::Postgres,
            dependency_failure: json!({"inventory_job": "skip"}),
            expected_marts: MemberReaction::Runs(vec![("orders", "customers")]),
            expected_inventory: MemberReaction::Skipped,
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
            publish_dependency_group(store, location, &test_case.dependency_failure).await;
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
            assert_eq!(
                staging.request.job_key.as_deref(),
                Some("staging_job"),
                "{}",
                test_case.description
            );
            assert!(
                claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("dependents wait")
                    .is_none(),
                "{}: dependents wait for the job they read from",
                test_case.description
            );
            finish(store, &staging, (NodeOutcome::Failed, RunOutcome::Failed)).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("react to the failure");
            let reactions: BTreeMap<String, MemberReaction> =
                member_reactions(store, &lease, (&limits, scope)).await;
            assert_eq!(
                (&reactions["marts_job"], &reactions["inventory_job"]),
                (&test_case.expected_marts, &test_case.expected_inventory),
                "{}",
                test_case.description
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
                parent.status == RunStatus::Failed,
                "{}: {:?}",
                test_case.description,
                parent.status
            );
            close_test_store(fixture).await;
        });
    }
}
