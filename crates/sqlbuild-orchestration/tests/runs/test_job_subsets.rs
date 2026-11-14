//! A narrowed job run keeps only the job's own nodes, stays a run of that job, writes only what it selects and needs view on every node.

use std::collections::BTreeSet;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::{
    SUPPORT_ROLE, credentials, orders_grant, role_fixture,
};
use crate::runs::main::change_run::change_run;
use crate::runs::main::list_person_runs::list_person_runs;
use crate::runs::main::list_runs::list_runs;
use crate::runs::main::request_job_run::request_job_run;
use crate::runs::main::request_person_run::request_person_run;
use crate::runs::main::request_rerun::request_rerun;
use crate::runs::models::{
    JobRunRequest, PersonRunRequest, RerunRequest, RunAuthority, RunChange, RunChangeRequest,
    RunKind, RunNarrowing, RunRequest,
};
use crate::runs::tests::helpers::{json_column, rows, snapshot};
use crate::runs::tests::test_types::{
    ExactKeyPlanner, JobSubsetTestCase, NamedRerunTestCase, NarrowingDenialTestCase,
};
use crate::runs::types::RunsListing;
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
            // Customers read orders, so customer_sync waits for daily_orders.
            input
                .nodes
                .get_mut("customers")
                .expect("customers model")
                .definition["deps"] = json!(["orders"]);
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
            let mut run_ids: Vec<Uuid> = Vec::new();
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
                assert!(
                    listed.iter().all(|run| run.window.is_none()),
                    "a run without a backfill window lists none"
                );
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
                run_ids.push(request.run_id);
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
            let source_id = run_ids[0];
            let narrowing = RunNarrowing {
                select: strings(test_cases[0].select),
                exclude: strings(test_cases[0].exclude),
            };
            change_run(
                store,
                RunAuthority::Operator(&context),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: source_id,
                    claim_id: None,
                    change: RunChange::CancelRequested {
                        actor: context.actor.clone(),
                        reason: "Stop before it starts".into(),
                    },
                },
            )
            .await
            .expect("cancel the queued narrowed run");
            let rerun_id = Uuid::now_v7();
            request_rerun(
                store,
                (&context, None),
                &RerunRequest {
                    run_id: rerun_id,
                    source_run_id: source_id,
                    from_failure: false,
                    reason: "Run the same part again".into(),
                    priority: 0,
                    nodes: Vec::new(),
                },
            )
            .await
            .expect("rerun");
            let listed = list_runs(store, (location_id, "development"), 50)
                .await
                .expect("runs");
            assert_eq!(
                listed
                    .iter()
                    .find(|run| run.run_id == rerun_id)
                    .map(|run| run.narrowing.clone()),
                Some(Some(narrowing)),
                "{backend:?}: a rerun of part of a job is listed with the same narrowing"
            );
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

#[test]
fn given_a_finished_run_when_rerunning_named_nodes_then_only_those_rerun_for_people_who_list_the_target()
 {
    let test_cases = [
        NamedRerunTestCase {
            description: "two of the run's nodes",
            nodes: &["orders", "inventory"],
            from_failure: false,
            by_member: false,
            expected_selection: Some(&["inventory", "orders"]),
            expected_denied: false,
        },
        NamedRerunTestCase {
            description: "a node outside the run",
            nodes: &["orders", "products"],
            from_failure: false,
            by_member: false,
            expected_selection: None,
            expected_denied: false,
        },
        NamedRerunTestCase {
            description: "named nodes and from failure together",
            nodes: &["orders"],
            from_failure: true,
            by_member: false,
            expected_selection: None,
            expected_denied: false,
        },
        NamedRerunTestCase {
            description: "a member without view on every node, naming a node they may rerun",
            nodes: &["orders"],
            from_failure: false,
            by_member: true,
            expected_selection: None,
            expected_denied: true,
        },
        NamedRerunTestCase {
            description: "a member without view on every node, naming a node outside the run",
            nodes: &["products"],
            from_failure: false,
            by_member: true,
            expected_selection: None,
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
                    "grants": [orders_grant(json!(["view", "run.rerun"]), &["orders"])]}}),
            );
            input.documents.insert("jobs".into(), json!({"daily_orders": {
                "description": "Orders and stock", "selection": ["customers", "inventory", "orders"]}}));
            publish_snapshot(store, &input).await.expect("publish jobs");
            let owner = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let source_id = Uuid::now_v7();
            request_job_run(
                store,
                (&owner, None),
                &JobRunRequest {
                    run_id: source_id,
                    location_id: roles.location_id,
                    target: "development".into(),
                    job_key: "daily_orders".into(),
                    reason: "Build orders and stock".into(),
                    priority: 0,
                    window: None,
                    full_refresh: false,
                    select: Vec::new(),
                    exclude: Vec::new(),
                },
            )
            .await
            .expect("source run");
            change_run(
                store,
                RunAuthority::Operator(&owner),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: source_id,
                    claim_id: None,
                    change: RunChange::CancelRequested {
                        actor: owner.actor.clone(),
                        reason: "Stop before it starts".into(),
                    },
                },
            )
            .await
            .expect("finish the source run");
            let people = [
                (&roles.context, credentials(&roles.administrator)),
                (&roles.context, credentials(&roles.member)),
            ];
            for test_case in &test_cases {
                let request = RerunRequest {
                    run_id: Uuid::now_v7(),
                    source_run_id: source_id,
                    from_failure: test_case.from_failure,
                    reason: "Rerun part of the run".into(),
                    priority: 0,
                    nodes: test_case
                        .nodes
                        .iter()
                        .map(|node| (*node).to_owned())
                        .collect(),
                };
                let (context, principal) = people[usize::from(test_case.by_member)];
                let outcome = request_rerun(store, (context, Some(principal)), &request).await;
                let stored = rows(
                    store,
                    "SELECT request_json FROM sqb_runs WHERE run_id=$1",
                    vec![Parameter::Id(request.run_id)],
                )
                .await;
                let frozen: Option<RunRequest> = stored.first().map(|row| {
                    serde_json::from_value(json_column(&row["request_json"])).expect("frozen")
                });
                let keys = |keys: &[&str]| -> BTreeSet<String> {
                    keys.iter().map(|key| (*key).to_owned()).collect()
                };
                assert_eq!(
                    (
                        frozen.map(|frozen| (frozen.selection, frozen.write_set)),
                        matches!(outcome, Err(ControlError::Denied { .. })),
                    ),
                    (
                        test_case
                            .expected_selection
                            .map(|expected| (keys(expected), keys(expected))),
                        test_case.expected_denied,
                    ),
                    "{backend:?}: {}: {outcome:?}",
                    test_case.description
                );
            }
            let list = |before| {
                list_person_runs(
                    store,
                    people[0],
                    (
                        roles.location_id,
                        "development",
                        RunsListing::Newest(1, before),
                    ),
                    DefaultRole::None,
                )
            };
            let everything = list_person_runs(
                store,
                people[0],
                (
                    roles.location_id,
                    "development",
                    RunsListing::Newest(1000, None),
                ),
                DefaultRole::None,
            )
            .await
            .expect("all runs");
            let mut paged: Vec<Uuid> = Vec::new();
            let mut before = None;
            for _ in 0..everything.len() {
                let page = list(before).await.expect("page");
                paged.extend(page.iter().map(|run| run.run_id));
                before = page.last().map(|run| (run.queued_at, run.run_id));
            }
            assert_eq!(
                (paged, everything.len()),
                (
                    everything.iter().map(|run| run.run_id).collect::<Vec<_>>(),
                    2
                ),
                "{backend:?}: pages of one continue after the last run of the previous page, newest first"
            );
            close_test_store(fixture).await;
        });
    }
}
