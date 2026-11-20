//! A rerun of an ad-hoc build keeps the jobs its selection pinned, narrowed to the nodes it reruns.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::models::Action;
use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::credentials::tests::role_helpers::{denial, publish_support_role, role_fixture};
use crate::runs::_helpers::requests::adhoc::permission_decisions;
use crate::runs::_helpers::requests::permissions::authorize_execution;
use crate::runs::main::execution::change_run::change_run;
use crate::runs::main::requests::request_rerun::request_rerun;
use crate::runs::models::{
    PinnedJob, RerunRequest, RunAuthority, RunChange, RunChangeRequest, RunRequest,
};
use crate::runs::tests::helpers::{adhoc_fixture, json_column, queue, request, rows, snapshot};
use crate::runs::tests::test_types::{
    PinnedPermissionTestCase, PinnedRerunTestCase, PinnedWriteSetTestCase,
};
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::Parameter;
use crate::store::models::TransactionMode;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

const JOB_DENIED: &str = "missing job.run permission for this target: applying job nightly_job's settings needs job.run, as running the job does";

#[test]
fn given_an_adhoc_build_that_pinned_jobs_when_rerun_then_the_rerun_keeps_them_narrowed_to_its_nodes()
 {
    let test_cases = [
        PinnedRerunTestCase {
            description: "a rerun of every node keeps every pin whole",
            source_id: 1,
            nodes: &[],
            expected_jobs: &[
                ("nightly", &["inventory", "orders"]),
                ("weekly", &["inventory"]),
            ],
        },
        PinnedRerunTestCase {
            description: "a rerun of orders keeps weekly named, with none of its nodes",
            source_id: 2,
            nodes: &["orders"],
            expected_jobs: &[("nightly", &["orders"]), ("weekly", &[])],
        },
    ];
    let keys =
        |keys: &[&str]| -> BTreeSet<String> { keys.iter().map(|key| (*key).to_owned()).collect() };
    let job_settings = BTreeMap::from([
        (
            "nightly",
            BTreeMap::from([("full_refresh".to_owned(), json!(true))]),
        ),
        (
            "weekly",
            BTreeMap::from([("vars".to_owned(), json!({"region": "north"}))]),
        ),
    ]);
    let pinned = |jobs: &[(&str, &[&str])]| -> BTreeMap<String, PinnedJob> {
        jobs.iter()
            .map(|(name, nodes)| {
                let job = PinnedJob {
                    selection: keys(nodes),
                    settings: job_settings[*name].clone(),
                };
                ((*name).to_owned(), job)
            })
            .collect()
    };
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
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let owner = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            for test_case in &test_cases {
                let mut source: RunRequest = request(
                    (location, published.snapshot_id),
                    test_case.source_id,
                    0,
                    &["inventory", "orders"],
                );
                source.jobs = pinned(&[
                    ("nightly", &["inventory", "orders"]),
                    ("weekly", &["inventory"]),
                ]);
                queue(store, &source).await;
                change_run(
                    store,
                    RunAuthority::Operator(&owner),
                    &RunChangeRequest {
                        event_id: Uuid::now_v7(),
                        run_id: source.run_id,
                        claim_id: None,
                        change: RunChange::CancelRequested {
                            actor: owner.actor.clone(),
                            reason: "Stop before it starts".into(),
                        },
                    },
                )
                .await
                .expect("finish the source run");
                let rerun = RerunRequest {
                    run_id: Uuid::now_v7(),
                    source_run_id: source.run_id,
                    from_failure: false,
                    reason: "Rerun the pinned build".into(),
                    priority: 0,
                    nodes: test_case
                        .nodes
                        .iter()
                        .map(|node| (*node).to_owned())
                        .collect(),
                };
                request_rerun(store, (&owner, None), &rerun)
                    .await
                    .expect("rerun");
                let stored = rows(
                    store,
                    "SELECT request_json FROM sqb_runs WHERE run_id=$1",
                    vec![Parameter::Id(rerun.run_id)],
                )
                .await;
                let frozen: RunRequest =
                    serde_json::from_value(json_column(&stored[0]["request_json"]))
                        .expect("frozen");
                assert_eq!(
                    frozen.jobs,
                    pinned(test_case.expected_jobs),
                    "{backend:?}: {}",
                    test_case.description
                );
            }
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_an_adhoc_build_that_pinned_jobs_when_launched_then_it_needs_the_permissions_running_those_jobs_needs()
 {
    let test_cases = [
        PinnedPermissionTestCase {
            description: "a job's cursor needs run.rewrite_history",
            generation: 2,
            permissions: &["run.adhoc", "job.run"],
            jobs: &[(
                "nightly_job",
                &["orders"],
                r#"{"start_cursor": "2026-01-01"}"#,
            )],
            job_settings: r#"{"start_cursor": "2026-01-01"}"#,
            expected_permissions: r#"[["run.adhoc", true, null], ["job.run", true, null],
                ["run.rewrite_history", false, "missing run.rewrite_history permission for this target"]]"#,
            expected_launch: Err("missing run.rewrite_history permission for this target"),
        },
        PinnedPermissionTestCase {
            description: "a job's full refresh of its nodes needs run.rewrite_history",
            generation: 3,
            permissions: &["run.adhoc", "job.run"],
            jobs: &[("nightly_job", &["orders"], r#"{"full_refresh": true}"#)],
            job_settings: "{}",
            expected_permissions: r#"[["run.adhoc", true, null], ["job.run", true, null],
                ["run.rewrite_history", false, "missing run.rewrite_history permission for this target"]]"#,
            expected_launch: Err("missing run.rewrite_history permission for this target"),
        },
        PinnedPermissionTestCase {
            description: "a job's reload needs run.load on the sources it loads",
            generation: 4,
            permissions: &["run.adhoc", "job.run"],
            jobs: &[("nightly_job", &["orders"], r#"{"reload": true}"#)],
            job_settings: r#"{"reload": true}"#,
            expected_permissions: r#"[["run.adhoc", true, null], ["job.run", true, null],
                ["run.load", false, "missing run.load permission for this target"]]"#,
            expected_launch: Err("missing run.load permission for this target"),
        },
        PinnedPermissionTestCase {
            description: "a job's defer_to, refused when typed, needs job.run",
            generation: 5,
            permissions: &["run.adhoc"],
            jobs: &[("nightly_job", &["orders"], r#"{"defer_to": "production"}"#)],
            job_settings: r#"{"defer_to": "production"}"#,
            expected_permissions: r#"[["run.adhoc", true, null], ["job.run", false, "missing job.run permission for this target: applying job nightly_job's settings needs job.run, as running the job does"]]"#,
            expected_launch: Err(JOB_DENIED),
        },
        PinnedPermissionTestCase {
            description: "a job's defer_to applies with job.run, as for the job's own runs",
            generation: 6,
            permissions: &["run.adhoc", "job.run"],
            jobs: &[("nightly_job", &["orders"], r#"{"defer_to": "production"}"#)],
            job_settings: r#"{"defer_to": "production"}"#,
            expected_permissions: r#"[["run.adhoc", true, null], ["job.run", true, null]]"#,
            expected_launch: Ok(()),
        },
        PinnedPermissionTestCase {
            description: "a job's vars and warehouse need job.run, not run.vars or run.warehouse",
            generation: 7,
            permissions: &["run.adhoc", "job.run"],
            jobs: &[(
                "nightly_job",
                &["orders"],
                r#"{"vars": {"amount": "5"}, "warehouse": "REPORTS_XS"}"#,
            )],
            job_settings: r#"{"vars": {"amount": "5"}, "warehouse": "REPORTS_XS"}"#,
            expected_permissions: r#"[["run.adhoc", true, null], ["job.run", true, null]]"#,
            expected_launch: Ok(()),
        },
        PinnedPermissionTestCase {
            description: "a pinned job that keeps no nodes and gives no build-wide setting needs nothing",
            generation: 8,
            permissions: &["run.adhoc"],
            jobs: &[("nightly_job", &[], r#"{"full_refresh": true}"#)],
            job_settings: "{}",
            expected_permissions: r#"[["run.adhoc", true, null]]"#,
            expected_launch: Ok(()),
        },
        PinnedPermissionTestCase {
            description: "a pinned job without settings needs only run.adhoc",
            generation: 9,
            permissions: &["run.adhoc"],
            jobs: &[("nightly_job", &["orders"], "{}")],
            job_settings: "{}",
            expected_permissions: r#"[["run.adhoc", true, null]]"#,
            expected_launch: Ok(()),
        },
        PinnedPermissionTestCase {
            description: "a job's whole-run warehouse needs job.run though the job keeps none of the nodes",
            generation: 10,
            permissions: &["run.adhoc"],
            jobs: &[("nightly_job", &[], r#"{"warehouse": "REPORTS_XS"}"#)],
            job_settings: r#"{"warehouse": "REPORTS_XS"}"#,
            expected_permissions: r#"[["run.adhoc", true, null], ["job.run", false, "missing job.run permission for this target: applying job nightly_job's settings needs job.run, as running the job does"]]"#,
            expected_launch: Err(JOB_DENIED),
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
            for test_case in &test_cases {
                let nodes = ["orders", "source:shop.orders"];
                let grants = json!([{"nodes": ["tag:orders"], "permissions": test_case.permissions,
                    "resolved_nodes": {"tag:orders": nodes}}]);
                publish_support_role(store, roles.location_id, (test_case.generation, grants))
                    .await;
                let mut launched: RunRequest =
                    request((roles.location_id, Uuid::nil()), 1, 0, &["orders"]);
                launched.write_set = nodes.iter().map(|node| (*node).to_owned()).collect();
                launched.jobs = test_case
                    .jobs
                    .iter()
                    .map(|(name, nodes, settings)| {
                        let job = PinnedJob {
                            selection: nodes.iter().map(|node| (*node).to_owned()).collect(),
                            settings: serde_json::from_str(settings).expect("settings"),
                        };
                        ((*name).to_owned(), job)
                    })
                    .collect();
                launched.job_settings =
                    serde_json::from_str(test_case.job_settings).expect("job settings");
                let transaction = begin(store, TransactionMode::Read).await.expect("begin");
                let (transaction, decided) =
                    permission_decisions(transaction, roles.member_id, &launched)
                        .await
                        .expect("decide");
                commit(transaction).await.expect("commit");
                let decided: Vec<Value> = decided
                    .iter()
                    .map(|permission| {
                        json!([permission.action, permission.allowed, permission.reason])
                    })
                    .collect();
                let expected: Vec<Value> =
                    serde_json::from_str(test_case.expected_permissions).expect("expected");
                assert_eq!(decided, expected, "{backend:?}: {}", test_case.description);
                let transaction = begin(store, TransactionMode::Write).await.expect("begin");
                let launch = authorize_execution(
                    transaction,
                    (&roles.context, roles.member_id),
                    (&launched, Action::RunAdhoc),
                )
                .await
                .map(|_| ())
                .map_err(denial);
                assert_eq!(
                    launch.as_ref().map_err(String::as_str).copied(),
                    test_case.expected_launch,
                    "{backend:?}: {}",
                    test_case.description
                );
            }
            close_test_store(fixture).await;
        });
    }
}

#[test]
fn given_an_adhoc_build_whose_pinned_job_sets_load_sources_when_rerun_then_its_write_set_follows_the_job()
 {
    let test_cases = [
        PinnedWriteSetTestCase {
            description: "a job's load_sources = false still holds on the rerun",
            source_id: 1,
            job_settings: r#"{"load_sources": false}"#,
            expected_write_set: &["model:customers"],
        },
        PinnedWriteSetTestCase {
            description: "a job that leaves loading alone reruns with the sources the model reads",
            source_id: 2,
            job_settings: "{}",
            expected_write_set: &[
                "model:customers",
                "source:shop.accounts",
                "source:shop.customers",
            ],
        },
    ];
    for backend in [Backend::Sqlite, Backend::Postgres] {
        run(async {
            let Some(fixture) = open_test_store(backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let owner = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let (location, snapshot_id, _) = adhoc_fixture(store, &owner).await;
            for test_case in &test_cases {
                let mut source: RunRequest = request(
                    (location, snapshot_id),
                    test_case.source_id,
                    0,
                    &["model:customers"],
                );
                let job_settings: BTreeMap<String, Value> =
                    serde_json::from_str(test_case.job_settings).expect("job settings");
                source.jobs = BTreeMap::from([(
                    "customers_job".to_owned(),
                    PinnedJob {
                        selection: BTreeSet::from(["model:customers".to_owned()]),
                        settings: job_settings.clone(),
                    },
                )]);
                source.job_settings = job_settings;
                queue(store, &source).await;
                change_run(
                    store,
                    RunAuthority::Operator(&owner),
                    &RunChangeRequest {
                        event_id: Uuid::now_v7(),
                        run_id: source.run_id,
                        claim_id: None,
                        change: RunChange::CancelRequested {
                            actor: owner.actor.clone(),
                            reason: "Stop before it starts".into(),
                        },
                    },
                )
                .await
                .expect("finish the source run");
                let rerun = RerunRequest {
                    run_id: Uuid::now_v7(),
                    source_run_id: source.run_id,
                    from_failure: false,
                    reason: "Rerun customers".into(),
                    priority: 0,
                    nodes: Vec::new(),
                };
                request_rerun(store, (&owner, None), &rerun)
                    .await
                    .expect("rerun");
                let stored = rows(
                    store,
                    "SELECT request_json FROM sqb_runs WHERE run_id=$1",
                    vec![Parameter::Id(rerun.run_id)],
                )
                .await;
                let frozen: RunRequest =
                    serde_json::from_value(json_column(&stored[0]["request_json"]))
                        .expect("frozen");
                assert_eq!(
                    frozen
                        .write_set
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<&str>>(),
                    test_case.expected_write_set,
                    "{backend:?}: {}",
                    test_case.description
                );
            }
            close_test_store(fixture).await;
        });
    }
}
