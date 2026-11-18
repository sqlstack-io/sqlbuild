//! A group member waits for the members building the first nodes it reaches upstream.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use uuid::Uuid;

use crate::declarations::main::group_dependencies::group_dependencies;
use crate::declarations::models::JobDefinition;
use crate::declarations::tests::test_types::{GroupWaitsTestCase, NodeWaitCycleTestCase};
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::errors::ControlError;

fn input(lineage: &[(&str, Vec<&str>)]) -> SnapshotInput {
    SnapshotInput {
        location_id: Uuid::nil(),
        target: "development".into(),
        generation: 1,
        git_revision: None,
        git_dirty: false,
        git_author: None,
        image_id: None,
        environment_hash: "orders-environment".into(),
        sqb_version: "1.0.0".into(),
        nodes: lineage
            .iter()
            .map(|(key, deps)| {
                (
                    (*key).to_owned(),
                    SnapshotNode {
                        kind: "model".into(),
                        definition: json!({"deps": deps}),
                        raw_sql: None,
                        compiled_sql: None,
                        python_source: None,
                    },
                )
            })
            .collect(),
        documents: BTreeMap::new(),
    }
}

fn jobs(selections: &[(&str, Vec<&str>)]) -> BTreeMap<String, JobDefinition> {
    selections
        .iter()
        .map(|(name, selection)| {
            let definition =
                json!({"description": format!("Build {name}"), "selection": selection});
            (
                (*name).to_owned(),
                serde_json::from_value(definition).expect("job definition"),
            )
        })
        .collect()
}

fn waits(
    lineage: &[(&str, Vec<&str>)],
    selections: &[(&str, Vec<&str>)],
) -> Result<BTreeMap<String, BTreeSet<String>>, ControlError> {
    let jobs = jobs(selections);
    let members: BTreeSet<String> = jobs.keys().cloned().collect();
    group_dependencies(
        ("orders_job_group", &input(lineage)),
        &jobs,
        (&members, &BTreeSet::new()),
    )
    .map(|order| order.jobs)
}

#[test]
fn given_member_lineage_when_deriving_group_waits_then_each_member_waits_for_the_first_owners_upstream()
 {
    let test_cases = [
        GroupWaitsTestCase {
            description: "a chain through a member's nodes waits only for that member",
            lineage: vec![
                ("model:raw_orders", vec![]),
                ("model:stg_orders", vec!["model:raw_orders"]),
                ("model:orders", vec!["model:stg_orders"]),
            ],
            jobs: vec![
                ("load_job", vec!["model:raw_orders"]),
                ("staging_job", vec!["model:stg_orders"]),
                ("marts_job", vec!["model:orders"]),
            ],
            expected: vec![
                ("load_job", vec![]),
                ("staging_job", vec!["load_job"]),
                ("marts_job", vec!["staging_job"]),
            ],
        },
        GroupWaitsTestCase {
            description: "a shared function behind another member's node adds none of its builders",
            lineage: vec![
                ("function:normalise_sku", vec![]),
                ("model:stg_inventory", vec!["function:normalise_sku"]),
                ("model:stg_orders", vec!["function:normalise_sku"]),
                ("model:stg_customers", vec!["function:normalise_sku"]),
                ("model:inventory", vec!["model:stg_inventory"]),
            ],
            jobs: vec![
                (
                    "inventory_staging_job",
                    vec!["function:normalise_sku", "model:stg_inventory"],
                ),
                (
                    "orders_staging_job",
                    vec!["function:normalise_sku", "model:stg_orders"],
                ),
                (
                    "customers_staging_job",
                    vec!["function:normalise_sku", "model:stg_customers"],
                ),
                ("inventory_job", vec!["model:inventory"]),
            ],
            expected: vec![
                ("inventory_staging_job", vec![]),
                ("orders_staging_job", vec![]),
                ("customers_staging_job", vec![]),
                ("inventory_job", vec!["inventory_staging_job"]),
            ],
        },
        GroupWaitsTestCase {
            description: "a path through nodes no member builds still waits for the owner beyond",
            lineage: vec![
                ("model:raw_customers", vec![]),
                ("model:customer_view", vec!["model:raw_customers"]),
                ("model:customer_lookup", vec!["model:customer_view"]),
                ("model:customers", vec!["model:customer_lookup"]),
            ],
            jobs: vec![
                ("load_job", vec!["model:raw_customers"]),
                ("customers_job", vec!["model:customers"]),
            ],
            expected: vec![("load_job", vec![]), ("customers_job", vec!["load_job"])],
        },
        GroupWaitsTestCase {
            description: "a shared node reached first waits for every member building it",
            lineage: vec![
                ("function:normalise_sku", vec![]),
                ("model:inventory", vec!["function:normalise_sku"]),
            ],
            jobs: vec![
                ("orders_staging_job", vec!["function:normalise_sku"]),
                ("inventory_staging_job", vec!["function:normalise_sku"]),
                ("inventory_job", vec!["model:inventory"]),
            ],
            expected: vec![
                ("orders_staging_job", vec![]),
                ("inventory_staging_job", vec![]),
                (
                    "inventory_job",
                    vec!["inventory_staging_job", "orders_staging_job"],
                ),
            ],
        },
        GroupWaitsTestCase {
            description: "a member keeps walking past a shared node it builds itself",
            lineage: vec![
                ("model:raw_orders", vec![]),
                ("function:normalise_sku", vec!["model:raw_orders"]),
                ("model:stg_inventory", vec!["function:normalise_sku"]),
            ],
            jobs: vec![
                ("load_job", vec!["model:raw_orders"]),
                ("orders_staging_job", vec!["function:normalise_sku"]),
                (
                    "inventory_staging_job",
                    vec!["function:normalise_sku", "model:stg_inventory"],
                ),
            ],
            expected: vec![
                ("load_job", vec![]),
                ("orders_staging_job", vec!["load_job"]),
                ("inventory_staging_job", vec!["load_job"]),
            ],
        },
        widely_shared_test_case(),
    ];
    for test_case in test_cases {
        let expected: BTreeMap<String, BTreeSet<String>> = test_case
            .expected
            .iter()
            .map(|(name, deps)| {
                (
                    (*name).to_owned(),
                    deps.iter().map(|dep| (*dep).to_owned()).collect(),
                )
            })
            .collect();
        let actual = waits(&test_case.lineage, &test_case.jobs).expect(test_case.description);
        assert_eq!(actual, expected, "{}", test_case.description);
    }
}

fn widely_shared_test_case() -> GroupWaitsTestCase {
    GroupWaitsTestCase {
        description: "a member reading another's node past a widely shared function waits once",
        lineage: vec![
            ("function:normalise_sku", vec![]),
            ("model:orders_a", vec!["function:normalise_sku"]),
            ("model:orders_b", vec!["function:normalise_sku"]),
            ("model:orders_c", vec!["function:normalise_sku"]),
            ("model:stg_orders", vec!["model:orders_a"]),
            ("model:orders", vec!["model:stg_orders"]),
        ],
        jobs: vec![
            ("a_job", vec!["function:normalise_sku", "model:orders_a"]),
            ("b_job", vec!["function:normalise_sku", "model:orders_b"]),
            ("c_job", vec!["function:normalise_sku", "model:orders_c"]),
            ("staging_job", vec!["model:stg_orders"]),
            ("marts_job", vec!["model:orders"]),
            ("functions_job", vec!["function:normalise_sku"]),
        ],
        expected: vec![
            ("a_job", vec![]),
            ("b_job", vec![]),
            ("c_job", vec![]),
            ("functions_job", vec![]),
            ("staging_job", vec!["a_job"]),
            ("marts_job", vec!["staging_job"]),
        ],
    }
}

#[test]
fn given_members_reading_each_other_when_deriving_group_waits_then_the_cycle_names_both_waits() {
    let lineage = [
        ("model:stg_orders", vec!["model:customers"]),
        ("model:customers", vec!["model:stg_orders"]),
    ];
    let selections = [
        ("orders_job", vec!["model:stg_orders"]),
        ("customers_job", vec!["model:customers"]),
    ];
    let error = waits(&lineage, &selections).expect_err("a cycle is rejected");
    assert_eq!(
        error.to_string(),
        "invalid control-plane data: group dependencies contain a cycle in job group \
         orders_job_group: customers_job waits for orders_job (model:customers reads \
         model:stg_orders); orders_job waits for customers_job (model:stg_orders reads \
         model:customers)"
    );
}

#[test]
fn given_members_waiting_per_node_when_checking_cycles_then_only_node_cycles_are_refused_with_their_nodes()
 {
    let crossing = vec![
        ("model:c_root", vec![]),
        ("model:b_mart", vec!["model:c_root"]),
        ("model:a_mart", vec!["model:b_mart"]),
        ("model:c_report", vec!["model:a_mart"]),
    ];
    let crossing_jobs = vec![
        ("a_job", vec!["model:a_mart"]),
        ("b_job", vec!["model:b_mart"]),
        ("c_job", vec!["model:c_root", "model:c_report"]),
    ];
    let test_cases = [
        NodeWaitCycleTestCase {
            description: "a member reading a pooled member's node, also through a third member, waits for its whole job",
            lineage: vec![
                ("model:raw_orders", vec![]),
                ("model:stg_orders", vec!["model:raw_orders"]),
                ("model:orders", vec!["model:stg_orders"]),
            ],
            jobs: vec![
                ("load_job", vec!["model:raw_orders"]),
                ("staging_job", vec!["model:stg_orders"]),
                ("marts_job", vec!["model:orders"]),
            ],
            node_waits: vec!["load_job", "staging_job", "marts_job"],
            pooled: vec!["load_job", "marts_job"],
            expected_outcome: Ok(vec![("marts_job", vec!["load_job"])]),
        },
        NodeWaitCycleTestCase {
            description: "pooled members whose nodes cross both ways cannot run one at a time",
            lineage: crossing.clone(),
            jobs: crossing_jobs.clone(),
            node_waits: vec!["a_job", "b_job", "c_job"],
            pooled: vec!["a_job", "c_job"],
            expected_outcome: Err(
                "model:a_mart (a_job) waits for model:b_mart (b_job); model:b_mart (b_job) waits \
                 for model:c_root (c_job); model:c_root (c_job) waits for the end of job a_job; \
                 the end of job a_job waits for model:a_mart (a_job) (members that share a pool \
                 run one at a time, so one that reads another's nodes waits for that whole job)",
            ),
        },
        NodeWaitCycleTestCase {
            description: "job waits that cycle over acyclic nodes publish when every member waits per node",
            lineage: crossing.clone(),
            jobs: crossing_jobs.clone(),
            node_waits: vec!["a_job", "b_job", "c_job"],
            pooled: vec![],
            expected_outcome: Ok(vec![]),
        },
        NodeWaitCycleTestCase {
            description: "the same group waiting for jobs is refused, naming the nodes",
            lineage: crossing.clone(),
            jobs: crossing_jobs.clone(),
            node_waits: vec![],
            pooled: vec![],
            expected_outcome: Err(
                "a_job waits for b_job (model:a_mart reads model:b_mart); b_job waits for c_job \
                 (model:b_mart reads model:c_root); c_job waits for a_job (model:c_report reads \
                 model:a_mart)",
            ),
        },
        NodeWaitCycleTestCase {
            description: "one member waiting for whole jobs closes the loop through its own nodes",
            lineage: crossing.clone(),
            jobs: crossing_jobs.clone(),
            node_waits: vec!["a_job", "b_job"],
            pooled: vec![],
            expected_outcome: Err(
                "model:a_mart (a_job) waits for model:b_mart (b_job); model:b_mart (b_job) waits \
                 for model:c_root (c_job); model:c_root (c_job) waits for the end of job a_job; \
                 the end of job a_job waits for model:a_mart (a_job)",
            ),
        },
        NodeWaitCycleTestCase {
            description: "nodes that read each other across members are refused",
            lineage: vec![
                ("model:stg_orders", vec!["model:customers"]),
                ("model:customers", vec!["model:stg_orders"]),
            ],
            jobs: vec![
                ("orders_job", vec!["model:stg_orders"]),
                ("customers_job", vec!["model:customers"]),
            ],
            node_waits: vec!["orders_job", "customers_job"],
            pooled: vec![],
            expected_outcome: Err(
                "model:customers (customers_job) waits for model:stg_orders (orders_job); \
                 model:stg_orders (orders_job) waits for model:customers (customers_job)",
            ),
        },
        NodeWaitCycleTestCase {
            description: "a Python task's input holds its whole member back, so reading the member's own output is refused",
            lineage: vec![
                ("model:a_mart", vec![]),
                ("model:c_report", vec!["model:a_mart"]),
                ("task:a_export", vec!["model:c_report"]),
            ],
            jobs: vec![
                ("a_job", vec!["model:a_mart", "task:a_export"]),
                ("c_job", vec!["model:c_report"]),
            ],
            node_waits: vec!["a_job", "c_job"],
            pooled: vec![],
            expected_outcome: Err(
                "model:a_mart (a_job) waits for the start of job a_job; the start of job a_job \
                 waits for model:c_report (c_job); model:c_report (c_job) waits for model:a_mart \
                 (a_job)",
            ),
        },
    ];
    for test_case in test_cases {
        let mut jobs = jobs(&test_case.jobs);
        for member in &test_case.pooled {
            jobs.get_mut(*member).expect("pooled member").pools =
                serde_json::from_value(json!({"warehouse": {"amount": 1, "limit": 1}}))
                    .expect("pool");
        }
        let members: BTreeSet<String> = jobs.keys().cloned().collect();
        let node_waits: BTreeSet<String> = test_case
            .node_waits
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        let actual = group_dependencies(
            ("orders_job_group", &input(&test_case.lineage)),
            &jobs,
            (&members, &node_waits),
        )
        .map(|order| {
            order
                .pool_waits
                .into_iter()
                .map(|(member, jobs)| (member, jobs.into_iter().collect::<Vec<_>>()))
                .collect::<Vec<_>>()
        })
        .map_err(|error| error.to_string());
        assert_eq!(
            actual,
            test_case
                .expected_outcome
                .map(|waits| {
                    waits
                        .into_iter()
                        .map(|(member, jobs)| {
                            (
                                member.to_owned(),
                                jobs.into_iter().map(str::to_owned).collect(),
                            )
                        })
                        .collect()
                })
                .map_err(|cycle| format!(
                    "invalid control-plane data: group dependencies contain a cycle in job group \
                     orders_job_group: {cycle}"
                )),
            "{}",
            test_case.description
        );
    }
}
