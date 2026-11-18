//! Pinned groups coordinate real transactions, inherited slots, cancellation and replay.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::main::{
    execution::advance_groups::advance_groups, execution::change_run::change_run,
    execution::claim_next_run::claim_next_run, execution::run_execution::run_execution,
    listings::list_runs::list_runs, requests::request_job_run::request_job_run,
};
use crate::runs::models::{
    JobRunRequest, NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunOutcome, RunStatus,
};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::helpers::read_recall_events;
use crate::warehouse_history::main::read_person_run_ignored_audits::read_person_run_ignored_audits;
use serde_json::json;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::runs::tests::claim_helpers::{
    publish_audit_results, slot_counts, succeed, succeed_ignoring_audit_failures,
};
use crate::runs::tests::helpers::snapshot;
use crate::runs::tests::test_types::{
    GroupMemberSettingsTestCase, RunLifecycleTestCase, TransactionFault,
};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

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
                "staging": {"description": "Stage customers", "selection": ["customers"], "pools": {"warehouse": {"amount": 1, "limit": 100}}},
                "marts": {"description": "Build orders", "selection": ["orders"], "pools": {"warehouse": {"amount": 1, "limit": 100}}}
            }));
            input.documents.insert("groups".into(), json!({"daily_orders": {
                "description": "Stage then build orders", "selection": ["customers", "orders"],
                "pools": {"warehouse": {"amount": 1, "limit": 100}}, "jobs": {"staging": [], "marts": ["staging"]}
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
                window: None,
                full_refresh: false,
                select: Vec::new(),
                exclude: Vec::new(),
            };
            assert!(
                request_job_run(store, (&context, None), &request)
                    .await
                    .expect("request group")
            );
            assert!(
                !request_job_run(store, (&context, None), &request)
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
                    error_message: None,
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
            input
                .nodes
                .get_mut("inventory")
                .expect("inventory")
                .definition["deps"] = json!(["customers"]);
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
            request_job_run(store, (&context, None), &standalone)
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
                "orders_job": {"description": "Build orders", "selection": ["orders"], "pools": {"inventory_limit": {"amount": 1, "limit": 100}}},
                "inventory_job": {"description": "Build inventory", "selection": ["customers"], "pools": {"orders_limit": {"amount": 1, "limit": 100}}}
            }));
            input.documents.insert("groups".into(), json!({
                "orders_group": {"description": "Orders group", "selection": ["orders"], "pools": {"orders_limit": {"amount": 1, "limit": 100}}, "jobs": {"orders_job": []}},
                "inventory_group": {"description": "Inventory group", "selection": ["customers"], "pools": {"inventory_limit": {"amount": 1, "limit": 100}}, "jobs": {"inventory_job": []}}
            }));
            publish_snapshot(store, &input)
                .await
                .expect("publish groups");
            for job_key in ["orders_group", "inventory_group"] {
                request_job_run(
                    store,
                    (&context, None),
                    &JobRunRequest {
                        run_id: Uuid::now_v7(),
                        location_id: location,
                        target: "development".into(),
                        job_key: job_key.into(),
                        reason: "Build independent groups".into(),
                        priority: 1,
                        window: None,
                        full_refresh: false,
                        select: Vec::new(),
                        exclude: Vec::new(),
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

#[test]
fn given_group_members_with_different_audit_settings_when_run_then_each_keeps_its_own_and_the_group_sums_ignored_failures()
 {
    let test_cases = [
        GroupMemberSettingsTestCase {
            description: "SQLite members keep their own settings",
            backend: Backend::Sqlite,
            members: [("staging", true, 2), ("marts", false, 0)],
            expected_group_status: RunStatus::Success,
            expected_group_ignored: 2,
        },
        GroupMemberSettingsTestCase {
            description: "PostgreSQL members keep their own settings",
            backend: Backend::Postgres,
            members: [("staging", false, 0), ("marts", true, 3)],
            expected_group_status: RunStatus::Success,
            expected_group_ignored: 3,
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
            let [
                (first_job, first_ignores, _),
                (second_job, second_ignores, _),
            ] = test_case.members;
            let mut input = snapshot(location);
            input.documents.insert(
                "jobs".into(),
                json!({
                    first_job: {"description": "Stage customers", "selection": ["customers"],
                        "settings": {"ignore_audit_failures": first_ignores, "concurrency": 2}},
                    second_job: {"description": "Build orders", "selection": ["orders"],
                        "settings": {"ignore_audit_failures": second_ignores}}
                }),
            );
            input.documents.insert(
                "groups".into(),
                json!({"daily_orders": {
                    "description": "Both members at once", "selection": ["customers", "orders"],
                    "jobs": {first_job: [], second_job: []}
                }}),
            );
            publish_snapshot(store, &input)
                .await
                .expect("publish group");
            let request = JobRunRequest {
                run_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                job_key: "daily_orders".into(),
                reason: "Build the group".into(),
                priority: 1,
                window: None,
                full_refresh: true,
                select: Vec::new(),
                exclude: Vec::new(),
            };
            request_job_run(store, (&context, None), &request)
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
                .expect("start virtual parent");
            let mut settings = BTreeMap::new();
            let mut members: BTreeMap<String, Uuid> = BTreeMap::new();
            for _ in test_case.members {
                let claim = claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim member")
                    .expect("member ready");
                let job = claim.request.job_key.clone().expect("member job");
                let (_, _, ignored) = test_case
                    .members
                    .into_iter()
                    .find(|(name, _, _)| *name == job)
                    .expect("declared member");
                let mut audits: Vec<(String, bool)> = (0..ignored)
                    .map(|index| (format!("{job}_error_{index}"), true))
                    .collect();
                audits.push((format!("{job}_warn"), false));
                publish_audit_results(store, &claim, &audits).await;
                members.insert(job.clone(), claim.request.run_id);
                settings.insert(job, claim.request.settings.clone());
                succeed_ignoring_audit_failures(store, &claim, ignored).await;
            }
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("finish parent");
            let administrator = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("administrator");
            let ignored_audits = async |run_id| {
                let mut failures: Vec<_> = read_person_run_ignored_audits(
                    store,
                    (&context, administrator),
                    (location, "development", run_id),
                    DefaultRole::None,
                )
                .await
                .expect("ignored audits")?
                .into_iter()
                .map(|failure| {
                    assert_eq!(
                        Some(failure.run_id),
                        failure.job_key.as_ref().map(|job| members[job]),
                        "each failure names the member run that let it through"
                    );
                    (
                        failure.job_key.expect("member job"),
                        failure.audit_name,
                        failure.binding_key,
                        failure.severity,
                        failure.violation_count,
                        failure.attached_target_name,
                        failure.attached_column_name,
                    )
                })
                .collect();
                failures.sort();
                Some(failures)
            };
            let expected = |member: Option<&str>| {
                test_case
                    .members
                    .iter()
                    .filter(|(name, _, _)| member.is_none_or(|member| member == *name))
                    .flat_map(|(name, _, ignored)| {
                        (0..*ignored).map(move |index| {
                            let audit = format!("{name}_error_{index}");
                            (
                                (*name).to_owned(),
                                audit.clone(),
                                Some(format!("orders.{audit}")),
                                "error".to_owned(),
                                Some(i64::from(index) + 2),
                                Some("orders".to_owned()),
                                Some("order_id".to_owned()),
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                ignored_audits(request.run_id).await,
                Some(expected(None)),
                "{}: the group lists every member's ignored failures and none of their passing or warning checks",
                test_case.description
            );
            for (job, run_id) in &members {
                assert_eq!(
                    ignored_audits(*run_id).await,
                    Some(expected(Some(job))),
                    "{}: a member lists only its own",
                    test_case.description
                );
            }
            assert_eq!(
                ignored_audits(Uuid::now_v7()).await,
                None,
                "{}: an unknown run is not found",
                test_case.description
            );

            assert_eq!(
                settings,
                BTreeMap::from([
                    (
                        first_job.to_owned(),
                        BTreeMap::from([
                            ("concurrency".to_owned(), json!(2)),
                            ("full_refresh".to_owned(), json!(true)),
                            ("ignore_audit_failures".to_owned(), json!(first_ignores)),
                        ]),
                    ),
                    (
                        second_job.to_owned(),
                        BTreeMap::from([
                            ("full_refresh".to_owned(), json!(true)),
                            ("ignore_audit_failures".to_owned(), json!(second_ignores)),
                        ]),
                    ),
                ]),
                "{}: each member runs with its own settings and the group's full refresh",
                test_case.description
            );
            let runs = list_runs(store, scope, 100).await.expect("runs");
            let parent = runs
                .iter()
                .find(|run| run.run_id == request.run_id)
                .expect("parent");
            assert_eq!(
                (
                    parent.status,
                    parent.ignored_audit_failures,
                    parent
                        .node_counts_json
                        .contains_key("audit_failures_ignored"),
                ),
                (
                    test_case.expected_group_status,
                    test_case.expected_group_ignored,
                    false,
                ),
                "{}: the group succeeds with its members' ignored failures summed",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
