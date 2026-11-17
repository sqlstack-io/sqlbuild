//! Members that wait per node start with their group, and each node runs once the nodes it reads from other members are built.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::main::{
    advance_groups::advance_groups, claim_next_run::claim_next_run,
    read_node_gates::read_node_gates, request_job_run::request_job_run,
    run_execution::run_execution,
};
use crate::runs::models::{
    GateInput, JobRunRequest, NodeGate, NodeGatesRequest, NodeOutcome, RunChange, RunOutcome,
};
use crate::runs::tests::claim_helpers::{finish, report};
use crate::runs::tests::helpers::snapshot;
use crate::runs::tests::test_types::{CrossingGroupTestCase, NodeGateTestCase};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_members_waiting_per_node_when_inputs_build_fail_or_retry_then_each_node_gate_follows_its_inputs_and_policy()
 {
    let waiting_for_customers = || NodeGate::Waiting {
        waiting_for: vec![GateInput {
            job_key: "staging_job".into(),
            node_key: "customers".into(),
        }],
    };
    let skipped = || NodeGate::Skipped {
        cause: "customers".into(),
    };
    let test_cases = [Backend::Sqlite, Backend::Postgres]
        .into_iter()
        .flat_map(|backend| {
            [
                NodeGateTestCase {
                    description: "partial skips only the node that reads the failed input",
                    backend,
                    dependency_failure: json!({}),
                    staging_retries: 0,
                    expected_orders: skipped(),
                    expected_inventory: NodeGate::Open,
                },
                NodeGateTestCase {
                    description: "skip skips every node once any input failed",
                    backend,
                    dependency_failure: json!({"marts_job": "skip"}),
                    staging_retries: 0,
                    expected_orders: skipped(),
                    expected_inventory: skipped(),
                },
                NodeGateTestCase {
                    description: "run opens every node once its inputs ended",
                    backend,
                    dependency_failure: json!({"marts_job": "run"}),
                    staging_retries: 0,
                    expected_orders: NodeGate::Open,
                    expected_inventory: NodeGate::Open,
                },
                NodeGateTestCase {
                    description: "a retry of the failed input re-opens its gate",
                    backend,
                    dependency_failure: json!({}),
                    staging_retries: 1,
                    expected_orders: waiting_for_customers(),
                    expected_inventory: NodeGate::Open,
                },
            ]
        });
    let mut completed = 0;
    for test_case in test_cases {
        let description = format!("{:?}: {}", test_case.backend, test_case.description);
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
            input.nodes.get_mut("orders").expect("orders").definition["deps"] =
                json!(["customers"]);
            input
                .nodes
                .get_mut("inventory")
                .expect("inventory")
                .definition["deps"] = json!(["products"]);
            let staging_failure = (test_case.staging_retries > 0)
                .then(|| json!({"retries": test_case.staging_retries, "retry_delay_seconds": 0}));
            input.documents.insert(
                "jobs".into(),
                json!({
                    "staging_job": {"description": "Stage customers and products",
                        "selection": ["customers", "products"], "on_failure": staging_failure},
                    "marts_job": {"description": "Build orders and inventory",
                        "selection": ["inventory", "orders"]}
                }),
            );
            input.documents.insert("groups".into(), json!({"nightly_job_group": {
                "description": "Stage, then build marts node by node",
                "selection": ["customers", "inventory", "orders", "products"],
                "jobs": {"staging_job": [], "marts_job": ["staging_job"]},
                "wait_reasons": {"marts_job": [
                    {"upstream_job": "staging_job", "from_node": "customers", "to_node": "orders"},
                    {"upstream_job": "staging_job", "from_node": "products", "to_node": "inventory"}
                ]},
                "dependency_failure": test_case.dependency_failure,
                "node_waits": ["marts_job", "staging_job"]
            }}));
            publish_snapshot(store, &input).await.expect("publish");
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            request_job_run(
                store,
                (&context, None),
                &JobRunRequest {
                    run_id: Uuid::now_v7(),
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
            let mut claims = BTreeMap::new();
            while let Some(claim) = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim member")
            {
                claims.insert(claim.request.job_key.clone().expect("member"), claim);
            }
            assert_eq!(
                claims.keys().map(String::as_str).collect::<Vec<_>>(),
                ["marts_job", "staging_job"],
                "{description}: both members start with the group"
            );
            let (staging, marts) = (&claims["staging_job"], &claims["marts_job"]);
            let execution = run_execution(store, (marts.request.run_id, marts.claim_id))
                .await
                .expect("execution")
                .expect("claimed");
            assert_eq!(
                execution
                    .gated_nodes
                    .iter()
                    .map(|(node, inputs)| (node.as_str(), inputs[0].node_key.as_str()))
                    .collect::<Vec<_>>(),
                [("inventory", "products"), ("orders", "customers")],
                "{description}: each marts node waits for the staging node it reads"
            );
            let asked = NodeGatesRequest {
                nodes: ["inventory".to_owned(), "orders".to_owned()].into(),
            };
            let gates = async || {
                read_node_gates(store, (marts.request.run_id, marts.claim_id), &asked)
                    .await
                    .expect("gates")
                    .expect("active claim")
                    .nodes
            };
            let node = |node: &str, outcome: NodeOutcome| RunChange::NodeFinished {
                node_key: node.into(),
                outcome,
                rows_affected: None,
                error_code: None,
                skipped_because_of: None,
                error_message: None,
            };
            report(
                store,
                staging,
                vec![
                    RunChange::Started,
                    node("products", NodeOutcome::Success),
                    node("customers", NodeOutcome::Failed),
                ],
            )
            .await;
            assert_eq!(
                gates().await,
                BTreeMap::from([
                    ("inventory".into(), NodeGate::Open),
                    ("orders".into(), waiting_for_customers()),
                ]),
                "{description}: a built input opens its node while the member still runs, \
                 and a failed one waits for the member to end"
            );
            report(
                store,
                staging,
                vec![RunChange::Finished {
                    outcome: RunOutcome::Partial,
                    reason: "customers failed".into(),
                    exit_code: Some(1),
                }],
            )
            .await;
            advance_groups(store, &lease, (&limits, scope))
                .await
                .expect("react to the failure");
            assert_eq!(
                gates().await,
                BTreeMap::from([
                    ("inventory".into(), test_case.expected_inventory.clone()),
                    ("orders".into(), test_case.expected_orders.clone()),
                ]),
                "{description}"
            );
            if test_case.staging_retries > 0 {
                let retry = claim_next_run(store, &lease, &limits, Some(scope))
                    .await
                    .expect("claim retry")
                    .expect("retry ready");
                assert_eq!(retry.request.selection, ["customers".to_owned()].into());
                finish(store, &retry, (NodeOutcome::Success, RunOutcome::Success)).await;
                assert_eq!(
                    gates().await,
                    BTreeMap::from([
                        ("inventory".into(), NodeGate::Open),
                        ("orders".into(), NodeGate::Open),
                    ]),
                    "{description}: the retried input opens its node"
                );
            }
            completed += 1;
            close_test_store(fixture).await;
        });
    }
    assert_eq!(completed, 8, "every case runs on both backends");
}

#[test]
fn given_job_waits_that_cycle_over_acyclic_nodes_when_the_group_runs_then_every_member_starts_and_nodes_open_in_lineage_order()
 {
    let test_cases = [Backend::Sqlite, Backend::Postgres].map(|backend| CrossingGroupTestCase {
        description: "each build opens exactly the node that reads it",
        backend,
        expected_open_after: vec![
            ("start", vec!["customers"]),
            ("customers", vec!["customers", "orders"]),
            ("orders", vec!["customers", "inventory", "orders"]),
            (
                "inventory",
                vec!["customers", "inventory", "orders", "products"],
            ),
        ],
    });
    let mut completed = 0;
    for test_case in test_cases {
        let description = format!("{:?}: {}", test_case.backend, test_case.description);
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
            for (node, deps) in [
                ("orders", json!(["customers"])),
                ("inventory", json!(["orders"])),
                ("products", json!(["inventory"])),
            ] {
                input.nodes.get_mut(node).expect("node").definition["deps"] = deps;
            }
            input.documents.insert(
                "jobs".into(),
                json!({
                    "c_job": {"description": "Build customers and products",
                        "selection": ["customers", "products"]},
                    "b_job": {"description": "Build orders", "selection": ["orders"]},
                    "a_job": {"description": "Build inventory", "selection": ["inventory"]}
                }),
            );
            input.documents.insert("groups".into(), json!({"nightly_job_group": {
                "description": "Three jobs whose job waits cycle",
                "selection": ["customers", "inventory", "orders", "products"],
                "jobs": {"a_job": ["b_job"], "b_job": ["c_job"], "c_job": ["a_job"]},
                "wait_reasons": {
                    "a_job": [{"upstream_job": "b_job", "from_node": "orders", "to_node": "inventory"}],
                    "b_job": [{"upstream_job": "c_job", "from_node": "customers", "to_node": "orders"}],
                    "c_job": [{"upstream_job": "a_job", "from_node": "inventory", "to_node": "products"}]
                },
                "node_waits": ["a_job", "b_job", "c_job"]
            }}));
            publish_snapshot(store, &input).await.expect("publish");
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            request_job_run(
                store,
                (&context, None),
                &JobRunRequest {
                    run_id: Uuid::now_v7(),
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
            let mut claims = BTreeMap::new();
            while let Some(claim) = claim_next_run(store, &lease, &limits, Some(scope))
                .await
                .expect("claim member")
            {
                claims.insert(claim.request.job_key.clone().expect("member"), claim);
            }
            assert_eq!(
                claims.len(),
                3,
                "{description}: every member starts at once"
            );
            let owner = |node: &str| match node {
                "orders" => "b_job",
                "inventory" => "a_job",
                _ => "c_job",
            };
            for claim in claims.values() {
                report(store, claim, vec![RunChange::Started]).await;
            }
            for (built, expected_open) in &test_case.expected_open_after {
                if *built != "start" {
                    report(
                        store,
                        &claims[owner(built)],
                        vec![RunChange::NodeFinished {
                            node_key: (*built).into(),
                            outcome: NodeOutcome::Success,
                            rows_affected: None,
                            error_code: None,
                            skipped_because_of: None,
                            error_message: None,
                        }],
                    )
                    .await;
                }
                let mut open: Vec<String> = Vec::new();
                for claim in claims.values() {
                    let gates = read_node_gates(
                        store,
                        (claim.request.run_id, claim.claim_id),
                        &NodeGatesRequest {
                            nodes: claim.request.selection.clone(),
                        },
                    )
                    .await
                    .expect("gates")
                    .expect("active claim");
                    open.extend(
                        gates
                            .nodes
                            .into_iter()
                            .filter(|(_, gate)| *gate == NodeGate::Open)
                            .map(|(node, _)| node),
                    );
                }
                open.sort();
                assert_eq!(open, *expected_open, "{description}: after {built}");
            }
            completed += 1;
            close_test_store(fixture).await;
        });
    }
    assert_eq!(completed, 2, "every case runs on both backends");
}
