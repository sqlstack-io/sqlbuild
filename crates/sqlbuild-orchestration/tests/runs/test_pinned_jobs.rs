//! A rerun of an ad-hoc build keeps the jobs its selection pinned, narrowed to the nodes it reruns.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::Action;
use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::_helpers::requests::permissions::execution_scopes;
use crate::runs::main::execution::change_run::change_run;
use crate::runs::main::requests::request_rerun::request_rerun;
use crate::runs::models::{
    PinnedJob, RerunRequest, RunAuthority, RunChange, RunChangeRequest, RunRequest,
};
use crate::runs::tests::helpers::{json_column, queue, request, rows, snapshot};
use crate::runs::tests::test_types::{PinnedPermissionTestCase, PinnedRerunTestCase};
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::Parameter;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

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
fn given_an_adhoc_build_that_pinned_jobs_when_launched_then_a_job_full_refresh_needs_run_rewrite_history()
 {
    let test_cases = [
        PinnedPermissionTestCase {
            description: "a pinned job that fully refreshes its nodes rewrites history, as running the job does",
            jobs: &[("nightly", &["orders"], r#"{"full_refresh": true}"#)],
            expected_actions: &[Action::RunAdhoc, Action::RunRewriteHistory],
        },
        PinnedPermissionTestCase {
            description: "a pinned job left with none of its nodes rewrites nothing",
            jobs: &[("nightly", &[], r#"{"full_refresh": true}"#)],
            expected_actions: &[Action::RunAdhoc],
        },
        PinnedPermissionTestCase {
            description: "a pinned job's vars and warehouse need no run.vars or run.warehouse",
            jobs: &[(
                "nightly",
                &["orders"],
                r#"{"vars": {"amount": "5"}, "warehouse": "REPORTS_XS"}"#,
            )],
            expected_actions: &[Action::RunAdhoc],
        },
    ];
    for test_case in test_cases {
        let mut launched: RunRequest = request((Uuid::nil(), Uuid::nil()), 1, 0, &["orders"]);
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
        let actions: Vec<Action> =
            execution_scopes(&launched, std::slice::from_ref(&launched), Action::RunAdhoc)
                .into_iter()
                .map(|scope| scope.action)
                .collect();
        assert_eq!(
            actions, test_case.expected_actions,
            "{}",
            test_case.description
        );
    }
}
