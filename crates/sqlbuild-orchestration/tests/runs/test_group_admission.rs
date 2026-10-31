//! Real database admission preserves completion orders, capacities and fair retries across replicas.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU32;

use serde_json::json;
use sqlbuild_orchestration::credentials::models::AuditContext;
use sqlbuild_orchestration::runs::main::{
    advance_groups::advance_groups, claim_next_run::claim_next_run, list_runs::list_runs,
    request_job_run::request_job_run,
};
use sqlbuild_orchestration::runs::models::{JobRunRequest, RunDispatch, RunStatus};
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use uuid::Uuid;

use crate::claim_helpers::{slot_counts, succeed};
use crate::helpers::{queue, request, snapshot};
use crate::location_helpers::create_location;
use crate::replica_helpers::second_store;
use crate::store_helpers::{close_store, close_test_store, open_test_store, run};
use crate::store_test_types::Backend;
use crate::test_types::{
    AdmissionParent, GroupAdmissionTestCase, GroupQueueTestCase, MixedAdmissionTestCase,
    RunLifecycleTestCase,
};

#[test]
fn given_mixed_physical_and_parent_waiters_when_admitting_then_priority_and_queue_order_win() {
    let test_cases = [
        MixedAdmissionTestCase {
            description: "higher-priority physical request precedes parent admission",
            physical_priority: 10,
            parent_priority: 1,
            expected_selections: ["inventory", "orders"],
        },
        MixedAdmissionTestCase {
            description: "earlier physical request wins at equal priority",
            physical_priority: 1,
            parent_priority: 1,
            expected_selections: ["inventory", "orders"],
        },
        MixedAdmissionTestCase {
            description: "higher-priority parent precedes physical request",
            physical_priority: 1,
            parent_priority: 10,
            expected_selections: ["orders", "inventory"],
        },
    ];
    for test_case in test_cases {
        for backend in [Backend::Sqlite, Backend::Postgres] {
            run(async {
                let Some(fixture) = open_test_store(backend).await else {
                    return;
                };
                let store = &fixture.store;
                migrate_store(store).await.expect("migrate");
                let location = create_location(store, "orders", "test-admin")
                    .await
                    .expect("location");
                let actor = AuditContext {
                    actor: "test-operator".into(),
                    granting_role: "control-store-owner".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                };
                let mut input = snapshot(location);
                input.documents.insert(
                    "jobs".into(),
                    json!({"stage": {"description": "Build orders", "selection": ["orders"]}}),
                );
                input.documents.insert("groups".into(), json!({"daily": {"description": "Group orders", "selection": ["orders"], "concurrency_groups": ["warehouse"], "jobs": {"stage": []}}}));
                let published = publish_snapshot(store, &input).await.expect("publish");
                let mut physical = request(
                    (location, published.snapshot_id),
                    900,
                    test_case.physical_priority,
                    &["inventory"],
                );
                physical.concurrency_groups.insert("warehouse".into());
                queue(store, &physical).await;
                request_job_run(
                    store,
                    &actor,
                    &JobRunRequest {
                        run_id: Uuid::from_u128(100),
                        location_id: location,
                        target: "development".into(),
                        job_key: "daily".into(),
                        reason: "Exercise mixed queue ordering".into(),
                        priority: test_case.parent_priority,
                    },
                )
                .await
                .expect("queue group");
                let lease = acquire_lease(store, Uuid::now_v7())
                    .await
                    .expect("elect")
                    .expect("leader");
                let limits =
                    BTreeMap::from([("warehouse".into(), NonZeroU32::new(1).expect("one"))]);
                let scope = (location, "development");
                for index in 0..test_case.expected_selections.len() {
                    advance_groups(store, &lease, (&limits, scope))
                        .await
                        .expect("advance");
                    let claim = claim_next_run(store, &lease, &limits, Some(scope))
                        .await
                        .expect("claim")
                        .expect(test_case.description);
                    assert_eq!(
                        claim.request.selection,
                        BTreeSet::from([test_case.expected_selections[index].into()]),
                        "{} {backend:?}",
                        test_case.description
                    );
                    assert_eq!(slot_counts(store).await.get("warehouse"), Some(&1));
                    succeed(store, &claim).await;
                }
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("finish parent");
                assert!(
                    list_runs(store, scope, 10)
                        .await
                        .expect("runs")
                        .iter()
                        .all(|run| run.status == RunStatus::Success)
                );
                assert!(slot_counts(store).await.is_empty());
                close_test_store(fixture).await;
            });
        }
    }
}

#[test]
fn given_an_unserved_queued_group_when_dispatching_an_eligible_scope_then_unknown_child_limits_do_not_block_it()
 {
    let test_cases = [
        RunLifecycleTestCase {
            description: "SQLite dispatch excludes unserved prospective demand",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Success,
        },
        RunLifecycleTestCase {
            description: "PostgreSQL dispatch excludes unserved prospective demand",
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
            let actor = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let unserved = create_location(store, "inventory", "test-admin")
                .await
                .expect("unserved location");
            let served = create_location(store, "orders", "test-admin")
                .await
                .expect("served location");
            for location in [unserved, served] {
                let mut input = snapshot(location);
                input.documents.insert("jobs".into(), json!({"stage": {"description": "Build orders", "selection": ["orders"], "concurrency_groups": if location == unserved {vec!["inventory_limit"]} else {vec![]}}}));
                input.documents.insert("groups".into(), json!({"daily": {"description": "Group orders", "selection": ["orders"], "jobs": {"stage": []}}}));
                publish_snapshot(store, &input).await.expect("publish");
                request_job_run(
                    store,
                    &actor,
                    &JobRunRequest {
                        run_id: location,
                        location_id: location,
                        target: "development".into(),
                        job_key: "daily".into(),
                        reason: "Exercise launcher eligibility".into(),
                        priority: 1,
                    },
                )
                .await
                .expect("queue group");
            }
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::new();
            let eligible = BTreeSet::from([(served, "development".into())]);
            let dispatch = RunDispatch {
                scope: Some((served, "development")),
                eligible: Some(&eligible),
            };
            advance_groups(store, &lease, (&limits, dispatch))
                .await
                .expect("eligible parent advances");
            let claim = claim_next_run(store, &lease, &limits, dispatch)
                .await
                .expect("eligible dispatch")
                .expect("eligible child");
            assert_eq!(claim.request.location_id, served);
            succeed(store, &claim).await;
            advance_groups(store, &lease, (&limits, dispatch))
                .await
                .expect("finish eligible parent");
            assert!(
                list_runs(store, (served, "development"), 10)
                    .await
                    .expect("served runs")
                    .iter()
                    .all(|run| run.status == test_case.expected_status),
                "{}",
                test_case.description
            );
            assert!(
                list_runs(store, (unserved, "development"), 10)
                    .await
                    .expect("unserved runs")
                    .iter()
                    .all(|run| run.status == RunStatus::Queued)
            );
            assert!(slot_counts(store).await.is_empty());
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_capacity_and_multistage_cycles_when_replicas_admit_and_resume_groups_then_every_group_finishes_within_limits()
 {
    let test_cases = [
        GroupAdmissionTestCase {
            description: "capacity two permits both sides of a cycle",
            limits: &[("orders_limit", 2), ("inventory_limit", 2)],
            parents: &[
                AdmissionParent {
                    holds: &["orders_limit"],
                    children: &[&["inventory_limit"]],
                },
                AdmissionParent {
                    holds: &["inventory_limit"],
                    children: &[&["orders_limit"]],
                },
            ],
            expected_running: 2,
            reductions: &[&[("orders_limit", 1), ("inventory_limit", 1)]],
        },
        GroupAdmissionTestCase {
            description: "asymmetric capacity still has a safe completion order",
            limits: &[("orders_limit", 2), ("inventory_limit", 1)],
            parents: &[
                AdmissionParent {
                    holds: &["orders_limit"],
                    children: &[&["inventory_limit"]],
                },
                AdmissionParent {
                    holds: &["inventory_limit"],
                    children: &[&["orders_limit"]],
                },
            ],
            expected_running: 2,
            reductions: &[],
        },
        GroupAdmissionTestCase {
            description: "three-way cycle leaves one parent waiting",
            limits: &[
                ("orders_limit", 1),
                ("inventory_limit", 1),
                ("products_limit", 1),
            ],
            parents: &[
                AdmissionParent {
                    holds: &["orders_limit"],
                    children: &[&["inventory_limit"]],
                },
                AdmissionParent {
                    holds: &["inventory_limit"],
                    children: &[&["products_limit"]],
                },
                AdmissionParent {
                    holds: &["products_limit"],
                    children: &[&["orders_limit"]],
                },
            ],
            expected_running: 2,
            reductions: &[],
        },
        GroupAdmissionTestCase {
            description: "a free first stage does not conceal a later cycle",
            limits: &[("orders_limit", 1), ("inventory_limit", 1)],
            parents: &[
                AdmissionParent {
                    holds: &["orders_limit"],
                    children: &[&[], &["inventory_limit"]],
                },
                AdmissionParent {
                    holds: &["inventory_limit"],
                    children: &[&[], &["orders_limit"]],
                },
            ],
            expected_running: 1,
            reductions: &[],
        },
        GroupAdmissionTestCase {
            description: "a persistently blocked child lets its parent's peer drain shared limits",
            limits: &[
                ("orders_limit", 1),
                ("inventory_limit", 1),
                ("products_limit", 1),
            ],
            parents: &[
                AdmissionParent {
                    holds: &["orders_limit"],
                    children: &[&["inventory_limit", "products_limit"]],
                },
                AdmissionParent {
                    holds: &["inventory_limit"],
                    children: &[&["products_limit"]],
                },
            ],
            expected_running: 2,
            reductions: &[],
        },
        GroupAdmissionTestCase {
            description: "multiple parent limits do not hoist independent child limits",
            limits: &[
                ("orders_limit", 1),
                ("inventory_limit", 1),
                ("products_limit", 1),
                ("support_limit", 1),
            ],
            parents: &[
                AdmissionParent {
                    holds: &["orders_limit", "products_limit"],
                    children: &[&["inventory_limit"]],
                },
                AdmissionParent {
                    holds: &["inventory_limit"],
                    children: &[&["orders_limit", "products_limit"]],
                },
                AdmissionParent {
                    holds: &["support_limit"],
                    children: &[&["orders_limit"]],
                },
            ],
            expected_running: 2,
            reductions: &[],
        },
    ];
    for test_case in test_cases {
        for backend in [Backend::Sqlite, Backend::Postgres] {
            run(async {
                let Some(fixture) = open_test_store(backend).await else {
                    return;
                };
                let store = &fixture.store;
                migrate_store(store).await.expect("migrate");
                let replica = second_store(store).await;
                let location = create_location(store, "orders", "test-admin")
                    .await
                    .expect("location");
                let actor = AuditContext {
                    actor: "test-operator".into(),
                    granting_role: "control-store-owner".into(),
                    request_id: Uuid::now_v7(),
                    ip_address: None,
                };
                let mut input = snapshot(location);
                let mut jobs = serde_json::Map::new();
                let mut groups = serde_json::Map::new();
                let mut child_count = 0;
                for (index, parent) in test_case.parents.iter().enumerate() {
                    let mut members = serde_json::Map::new();
                    let mut selection = BTreeSet::new();
                    for (stage, needs) in parent.children.iter().enumerate() {
                        let name = format!("orders_job_{index}_{stage}");
                        let node =
                            ["orders", "customers", "inventory", "products"][child_count % 4];
                        child_count += 1;
                        selection.insert(node);
                        jobs.insert(name.clone(), json!({"description": "Build a synthetic stage", "selection": [node], "concurrency_groups": needs}));
                        let after: Vec<String> = (0..stage)
                            .map(|earlier| format!("orders_job_{index}_{earlier}"))
                            .collect();
                        members.insert(name, json!(after));
                    }
                    groups.insert(format!("orders_group_{index}"), json!({"description": "Complete a synthetic group", "selection": selection, "concurrency_groups": parent.holds, "jobs": members}));
                }
                input.documents.insert("jobs".into(), json!(jobs));
                input.documents.insert("groups".into(), json!(groups));
                let published = publish_snapshot(store, &input)
                    .await
                    .expect("publish groups");
                let lease = acquire_lease(store, Uuid::now_v7())
                    .await
                    .expect("elect")
                    .expect("leader");
                let limits: BTreeMap<String, NonZeroU32> = test_case
                    .limits
                    .iter()
                    .map(|(key, count)| {
                        (
                            (*key).into(),
                            NonZeroU32::new(*count).expect("positive capacity"),
                        )
                    })
                    .collect();
                let scope = (location, "development");
                for index in 0..test_case.parents.len() {
                    request_job_run(
                        store,
                        &actor,
                        &JobRunRequest {
                            // Higher-priority arrivals exercise safety, not only an older waiter's priority barrier.
                            run_id: Uuid::from_u128(1000 - index as u128),
                            location_id: location,
                            target: "development".into(),
                            job_key: format!("orders_group_{index}"),
                            reason: "Exercise capacity-aware admission".into(),
                            priority: index as i64 + 1,
                        },
                    )
                    .await
                    .expect("request group");
                    let (first, second) = tokio::join!(
                        advance_groups(store, &lease, (&limits, scope)),
                        advance_groups(&replica, &lease, (&limits, scope))
                    );
                    first.expect("first scheduler admits higher-priority arrival");
                    second.expect("replica cannot admit an unsafe arrival");
                }
                for _ in 0..test_case.parents.len() {
                    let (first, second) = tokio::join!(
                        advance_groups(store, &lease, (&limits, scope)),
                        advance_groups(&replica, &lease, (&limits, scope))
                    );
                    first.expect("first scheduler advance");
                    second.expect("second scheduler advance");
                }
                let initial = list_runs(store, scope, 100).await.expect("initial runs");
                assert_eq!(
                    initial
                        .iter()
                        .filter(|run| run.status == RunStatus::Running)
                        .count(),
                    test_case.expected_running,
                    "{} {backend:?}",
                    test_case.description
                );
                let before = slot_counts(store).await;
                for reduction in test_case.reductions {
                    let reduced = reduction
                        .iter()
                        .map(|(key, count)| {
                            (
                                (*key).into(),
                                NonZeroU32::new(*count).expect("positive reduction"),
                            )
                        })
                        .collect();
                    assert!(
                        claim_next_run(&replica, &lease, &reduced, Some(scope))
                            .await
                            .expect("unsafe parent state must not fail physical dispatch")
                            .is_none()
                    );
                    assert_eq!(
                        slot_counts(store).await,
                        before,
                        "blocked work cannot mutate claims"
                    );
                    let unrelated =
                        request((location, published.snapshot_id), 777, 1, &["products"]);
                    queue(store, &unrelated).await;
                    let claim = claim_next_run(&replica, &lease, &reduced, Some(scope))
                        .await
                        .expect("unrelated recovery progress")
                        .expect("unrelated work still dispatches");
                    assert_eq!(claim.request.run_id, unrelated.run_id);
                    succeed(store, &claim).await;
                }
                for iteration in 0..child_count {
                    let owner = [store, &replica][iteration % 2];
                    for _ in 0..test_case.parents.len() {
                        advance_groups(owner, &lease, (&limits, scope))
                            .await
                            .expect("resume group from durable state");
                    }
                    let claim = claim_next_run(owner, &lease, &limits, Some(scope))
                        .await
                        .expect("claim")
                        .expect(test_case.description);
                    // Re-evaluate while a physical child still holds its temporary claims.
                    advance_groups(&replica, &lease, (&limits, scope))
                        .await
                        .expect("advance during running claim");
                    let occupied = slot_counts(store).await;
                    assert!(
                        occupied
                            .iter()
                            .all(|(key, count)| *count <= i64::from(limits[key].get())),
                        "{} {occupied:?}",
                        test_case.description
                    );
                    succeed(owner, &claim).await;
                }
                advance_groups(store, &lease, (&limits, scope))
                    .await
                    .expect("aggregate final children");
                let finished = list_runs(&replica, scope, 100)
                    .await
                    .expect("finished groups");
                assert_eq!(
                    finished.len(),
                    test_case.parents.len() + child_count + test_case.reductions.len()
                );
                assert!(
                    finished.iter().all(|run| run.status == RunStatus::Success),
                    "{} {finished:?}",
                    test_case.description
                );
                assert!(
                    slot_counts(store).await.is_empty(),
                    "terminal parents release their own slots"
                );
                close_store(replica).await;
                close_test_store(fixture).await;
            });
        }
    }
}

#[test]
fn given_an_older_blocked_allow_group_when_an_independent_generation_is_queued_then_overlap_allow_still_runs()
 {
    let test_cases = [
        RunLifecycleTestCase {
            description: "SQLite preserves overlap allow across generations",
            backend: Backend::Sqlite,
            expected_status: RunStatus::Running,
        },
        RunLifecycleTestCase {
            description: "PostgreSQL preserves overlap allow across generations",
            backend: Backend::Postgres,
            expected_status: RunStatus::Running,
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
            let actor = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let mut input = snapshot(location);
            input.documents.insert(
                "jobs".into(),
                json!({"stage": {"description": "Build a pinned stage", "selection": ["orders"]}}),
            );
            input.documents.insert("groups".into(), json!({"daily": {"description": "Allow independent generations", "selection": ["orders"], "overlap": "allow", "concurrency_groups": ["orders_limit"], "jobs": {"stage": []}}}));
            let first = publish_snapshot(store, &input)
                .await
                .expect("first snapshot");
            let older = JobRunRequest {
                run_id: Uuid::from_u128(2000),
                location_id: location,
                target: "development".into(),
                job_key: "daily".into(),
                reason: "Older group".into(),
                priority: 1,
            };
            request_job_run(store, &actor, &older)
                .await
                .expect("older request");
            input.generation = 2;
            input.documents.get_mut("jobs").expect("jobs")["stage"]["selection"] =
                json!(["inventory"]);
            input.documents.get_mut("groups").expect("groups")["daily"]["selection"] =
                json!(["inventory"]);
            input.documents.get_mut("groups").expect("groups")["daily"]["concurrency_groups"] =
                json!(["inventory_limit"]);
            publish_snapshot(store, &input)
                .await
                .expect("second snapshot");
            let newer = JobRunRequest {
                run_id: Uuid::from_u128(1000),
                reason: "Independent replacement generation".into(),
                ..older.clone()
            };
            request_job_run(store, &actor, &newer)
                .await
                .expect("newer request");
            let mut physical = request((location, first.snapshot_id), 3000, 10, &["products"]);
            physical.concurrency_groups.insert("orders_limit".into());
            queue(store, &physical).await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::from([
                ("orders_limit".into(), NonZeroU32::new(1).expect("one")),
                ("inventory_limit".into(), NonZeroU32::new(1).expect("one")),
            ]);
            let scope = (location, "development");
            let blocker = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("physical claim")
                .expect("higher priority blocker");
            assert_eq!(blocker.request.run_id, physical.run_id);
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("independent group admission");
            let runs = list_runs(store, scope, 100).await.expect("runs");
            assert_eq!(
                runs.iter()
                    .find(|run| run.run_id == older.run_id)
                    .expect("older")
                    .status,
                RunStatus::Queued
            );
            assert_eq!(
                runs.iter()
                    .find(|run| run.run_id == newer.run_id)
                    .expect("newer")
                    .status,
                test_case.expected_status,
                "{}",
                test_case.description
            );
            let child = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("new child")
                .expect("allow generation can progress");
            assert_eq!(child.request.parent_run_id, Some(newer.run_id));
            assert_eq!(child.request.selection, ["inventory".to_owned()].into());
            succeed(store, &child).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("finish replacement parent");
            succeed(store, &blocker).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("old parent retries after blocker drains");
            let child = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("old child")
                .expect("older group makes progress");
            assert_eq!(child.request.parent_run_id, Some(older.run_id));
            succeed(store, &child).await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("complete all groups");
            assert!(
                list_runs(store, scope, 100)
                    .await
                    .expect("finished")
                    .iter()
                    .all(|run| run.status == RunStatus::Success)
            );
            assert!(slot_counts(store).await.is_empty());
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_a_large_healthy_project_with_blocked_groups_when_claiming_then_unrelated_work_has_a_bounded_write_lock()
 {
    let test_cases = [
        GroupQueueTestCase {
            description: "SQLite scans a full blocked group page without expanding unrelated bodies",
            backend: Backend::Sqlite,
            group_count: 105,
            node_count: 3000,
            expected_max_claim_millis: 5000,
        },
        GroupQueueTestCase {
            description: "PostgreSQL scans a full blocked group page without expanding unrelated bodies",
            backend: Backend::Postgres,
            group_count: 105,
            node_count: 3000,
            expected_max_claim_millis: 5000,
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
            let actor = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let mut input = snapshot(location);
            let template = input.nodes["orders"].clone();
            for index in 0..test_case.node_count {
                let mut node = template.clone();
                node.definition["description"] =
                    json!(format!("Orders {index} {}", "details ".repeat(128)));
                input.nodes.insert(format!("orders_{index}"), node);
            }
            let jobs = (0..test_case.group_count).map(|index| (format!("job_{index}"), json!({"description": "Build orders", "selection": ["orders"], "concurrency_groups": ["warehouse"]}))).collect::<serde_json::Map<_, _>>();
            let groups = (0..test_case.group_count).map(|index| (format!("group_{index}"), json!({"description": "Independent order group", "selection": ["orders"], "jobs": {format!("job_{index}"): []}}))).collect::<serde_json::Map<_, _>>();
            input.documents.insert("jobs".into(), json!(jobs));
            input.documents.insert("groups".into(), json!(groups));
            input.documents.insert(
                "columns".into(),
                json!({"orders": "unused display metadata ".repeat(100000)}),
            );
            let published = publish_snapshot(store, &input)
                .await
                .expect("publish large project");
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let limits = BTreeMap::from([("warehouse".into(), NonZeroU32::new(1).expect("one"))]);
            let scope = (location, "development");
            let mut physical = request((location, published.snapshot_id), 1, 10, &["products"]);
            physical.concurrency_groups.insert("warehouse".into());
            queue(store, &physical).await;
            let blocker = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim")
                .expect("blocker");
            for index in 0..test_case.group_count {
                request_job_run(
                    store,
                    &actor,
                    &JobRunRequest {
                        run_id: Uuid::from_u128(1000 + index as u128),
                        location_id: location,
                        target: "development".into(),
                        job_key: format!("group_{index}"),
                        reason: "Exercise blocked group queue".into(),
                        priority: 1,
                    },
                )
                .await
                .expect("request group");
            }
            let started = std::time::Instant::now();
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("admit independent parents");
            eprintln!(
                "{:?}: {} parent admissions in {:?}",
                test_case.backend,
                test_case.group_count,
                started.elapsed()
            );
            let unrelated = request((location, published.snapshot_id), 2, -1, &["customers"]);
            queue(store, &unrelated).await;
            let started = std::time::Instant::now();
            let claim = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("scan blocked children")
                .expect("unrelated work");
            let elapsed = started.elapsed();
            eprintln!(
                "{:?}: {} groups / {} nodes, claim transaction {:?}",
                test_case.backend, test_case.group_count, test_case.node_count, elapsed
            );
            assert_eq!(
                claim.request.run_id, unrelated.run_id,
                "{}",
                test_case.description
            );
            assert!(
                elapsed.as_millis() < test_case.expected_max_claim_millis,
                "{}: {elapsed:?}",
                test_case.description
            );
            succeed(store, &claim).await;
            succeed(store, &blocker).await;
            close_test_store(fixture).await;
        });
    }
}
