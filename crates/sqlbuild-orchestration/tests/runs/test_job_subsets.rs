//! A narrowed job run keeps only the job's own nodes, stays a run of that job, writes only what it selects and needs view on every node.

use std::collections::BTreeSet;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::{
    SUPPORT_ROLE, credentials, orders_grant, role_fixture,
};
use crate::runs::main::list_runs::list_runs;
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::main::request_person_run::request_person_run;
use crate::runs::models::{JobRunRequest, PersonRunRequest, RunKind, RunNarrowing, RunRequest};
use crate::runs::tests::helpers::{json_column, rows, snapshot};
use crate::runs::tests::test_types::{ExactKeyPlanner, JobSubsetTestCase, NarrowingDenialTestCase};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_a_published_job_when_a_run_narrows_it_then_only_the_jobs_own_selected_nodes_are_frozen() {
    let test_cases = [
        JobSubsetTestCase {
            description: "an exclusion leaves the job's other nodes",
            job_key: "daily_orders",
            select: &[],
            exclude: &["inventory"],
            expected_selection: Some(&["customers", "orders"]),
        },
        JobSubsetTestCase {
            description: "a selection outside the job adds nothing",
            job_key: "daily_orders",
            select: &["orders", "products"],
            exclude: &[],
            expected_selection: Some(&["orders"]),
        },
        JobSubsetTestCase {
            description: "a job selector excludes another job's nodes",
            job_key: "daily_orders",
            select: &[],
            exclude: &["job:customer_sync"],
            expected_selection: Some(&["inventory", "orders"]),
        },
        JobSubsetTestCase {
            description: "a selection that leaves none of the job's nodes is refused",
            job_key: "daily_orders",
            select: &["products"],
            exclude: &[],
            expected_selection: None,
        },
        JobSubsetTestCase {
            description: "a flag cannot ride in as a selector",
            job_key: "daily_orders",
            select: &["orders", "--full-refresh"],
            exclude: &[],
            expected_selection: None,
        },
        JobSubsetTestCase {
            description: "an unknown selector is the planner's refusal",
            job_key: "daily_orders",
            select: &[],
            exclude: &["shipments"],
            expected_selection: None,
        },
        JobSubsetTestCase {
            description: "a job group cannot be narrowed",
            job_key: "nightly",
            select: &[],
            exclude: &["inventory"],
            expected_selection: None,
        },
    ];
    for backend in [Backend::Sqlite, Backend::Postgres] {
        run(async {
            let Some(fixture) = open_test_store(backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location_id = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let mut input = snapshot(location_id);
            input.documents.insert("jobs".into(), json!({
                "daily_orders": {"description": "Orders and stock", "selection": ["customers", "inventory", "orders"],
                    "concurrency_groups": ["orders-load"]},
                "customer_sync": {"description": "Customers", "selection": ["customers"]},
            }));
            input.documents.insert(
                "groups".into(),
                json!({"nightly": {
                "description": "Both jobs", "selection": ["customers", "inventory", "orders"],
                "jobs": {"customer_sync": ["daily_orders"], "daily_orders": []}}}),
            );
            publish_snapshot(store, &input).await.expect("publish jobs");
            let strings =
                |tokens: &[&str]| tokens.iter().map(|token| (*token).to_owned()).collect();
            for test_case in &test_cases {
                let request = JobRunRequest {
                    run_id: Uuid::now_v7(),
                    location_id,
                    target: "development".into(),
                    job_key: test_case.job_key.into(),
                    reason: "Rebuild part of the job".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: strings(test_case.select),
                    exclude: strings(test_case.exclude),
                };
                let outcome =
                    request_job_run(store, (&context, Some(&ExactKeyPlanner)), &request).await;
                let refused = matches!(outcome, Err(ControlError::Invalid(_)));
                assert!(
                    refused || outcome.as_ref().is_ok_and(|applied| *applied),
                    "{backend:?}: {}: {outcome:?}",
                    test_case.description
                );
                let stored = rows(
                    store,
                    "SELECT request_json FROM sqb_runs WHERE run_id=$1",
                    vec![Parameter::Id(request.run_id)],
                )
                .await;
                let frozen: Option<RunRequest> = stored.first().map(|row| {
                    serde_json::from_value(json_column(&row["request_json"])).expect("frozen")
                });
                let listed = list_runs(store, (location_id, "development"), 50)
                    .await
                    .expect("runs");
                assert_eq!(
                    listed
                        .iter()
                        .find(|run| run.run_id == request.run_id)
                        .map(|run| run.narrowing.clone()),
                    test_case.expected_selection.map(|_| {
                        Some(RunNarrowing {
                            select: request.select.clone(),
                            exclude: request.exclude.clone(),
                        })
                    }),
                    "{backend:?}: {}: the run lists the selectors it was narrowed with, and nothing else it was requested with",
                    test_case.description
                );
                let keys = |keys: &[&str]| -> BTreeSet<String> {
                    keys.iter().map(|key| (*key).to_owned()).collect()
                };
                assert_eq!(
                    frozen.map(|frozen| (
                        frozen.kind,
                        frozen.job_key,
                        frozen.selection,
                        frozen.write_set,
                        frozen.concurrency_groups,
                    )),
                    test_case.expected_selection.map(|expected| (
                        RunKind::Job,
                        Some("daily_orders".to_owned()),
                        keys(expected),
                        keys(expected),
                        keys(&["orders-load"]),
                    )),
                    "{backend:?}: {}: a run of the job, writing only what it selects, or refused",
                    test_case.description
                );
            }
            let unplanned = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id,
                target: "development".into(),
                job_key: "daily_orders".into(),
                reason: "Rebuild part of the job".into(),
                priority: 0,
                window: None,
                full_refresh: false,
                select: Vec::new(),
                exclude: vec!["inventory".into()],
            };
            assert!(
                matches!(
                    request_job_run(store, (&context, None), &unplanned).await,
                    Err(ControlError::Invalid(_))
                ),
                "{backend:?}: a caller without a planner cannot narrow a job"
            );
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_member_without_view_on_every_node_when_narrowing_a_job_then_every_selector_is_refused_alike()
 {
    let test_cases = [
        NarrowingDenialTestCase {
            description: "a node the member may run",
            select: &["orders"],
            exclude: &[],
            expected_denied: true,
        },
        NarrowingDenialTestCase {
            description: "a node that does not exist",
            select: &[],
            exclude: &["shipments"],
            expected_denied: true,
        },
        NarrowingDenialTestCase {
            description: "a node outside the job",
            select: &["products"],
            exclude: &[],
            expected_denied: true,
        },
        NarrowingDenialTestCase {
            description: "a job node without a grant",
            select: &["customers"],
            exclude: &[],
            expected_denied: true,
        },
        NarrowingDenialTestCase {
            description: "a job selector",
            select: &["job:daily_orders"],
            exclude: &[],
            expected_denied: true,
        },
    ];
    for backend in [Backend::Sqlite, Backend::Postgres] {
        run(async {
            let Some(fixture) = open_test_store(backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let roles = role_fixture(store).await;
            let mut input = snapshot(roles.location_id);
            input.generation = 2;
            input.documents.insert(
                "roles".into(),
                json!({SUPPORT_ROLE: {"description": "Answer order questions",
                    "grants": [orders_grant(json!(["view", "job.run"]), &["orders"])]}}),
            );
            input.documents.insert("jobs".into(), json!({"daily_orders": {
                "description": "Orders and stock", "selection": ["customers", "inventory", "orders"]}}));
            publish_snapshot(store, &input).await.expect("publish jobs");
            let member = (&roles.context, credentials(&roles.member));
            let strings =
                |tokens: &[&str]| tokens.iter().map(|token| (*token).to_owned()).collect();
            for test_case in &test_cases {
                let request = JobRunRequest {
                    run_id: Uuid::now_v7(),
                    location_id: roles.location_id,
                    target: "development".into(),
                    job_key: "daily_orders".into(),
                    reason: "Rebuild part of the job".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: strings(test_case.select),
                    exclude: strings(test_case.exclude),
                };
                let outcome = request_person_run(
                    store,
                    member,
                    PersonRunRequest::Job(&ExactKeyPlanner, &request),
                )
                .await;
                assert_eq!(
                    matches!(outcome, Err(ControlError::Denied { .. })),
                    test_case.expected_denied,
                    "{backend:?}: {}: denied before any selector resolves, so nothing about the nodes leaks: {outcome:?}",
                    test_case.description
                );
            }
            close_test_store(fixture).await;
        });
    }
}
