//! Every set of members that wait on each other is reported in one pass, and reads of seeds or shared nodes add the hint.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::constants::SHARED_READ_HINT;
use crate::declarations::main::group_cycle_message::group_cycle_message;
use crate::declarations::main::group_wait_graph::group_wait_graph;
use crate::declarations::tests::test_types::{GroupWaitGraphTestCase, Pairs};

fn map(pairs: Pairs) -> BTreeMap<String, BTreeSet<String>> {
    pairs
        .iter()
        .map(|(key, values)| {
            (
                (*key).to_owned(),
                values.iter().map(|value| (*value).to_owned()).collect(),
            )
        })
        .collect()
}

#[test]
fn given_group_members_and_node_lineage_when_deriving_waits_then_every_cycle_is_reported_with_its_reasons()
 {
    let two_cycles = "group dependencies contain a cycle in job group nightly: customers_job waits for \
         orders_job (model:customers reads model:stg_orders); orders_job waits for customers_job \
         (model:orders reads model:customers)\n\
         group dependencies contain a cycle in job group nightly: inventory_job waits for \
         shipments_job (model:inventory reads model:shipments); shipments_job waits for \
         inventory_job (model:shipments reads model:stg_inventory)";
    let seed_hub = format!(
        "group dependencies contain a cycle in job group nightly: customers_job waits for \
         seeds_and_orders_job (model:customers reads seed:regions); seeds_and_orders_job waits for \
         customers_job (model:orders reads model:customers)\n{SHARED_READ_HINT}"
    );
    let test_cases = [
        GroupWaitGraphTestCase {
            description: "two independent cycles are both reported, and a member that only waits on one is not",
            selections: &[
                ("customers_job", &["model:customers"]),
                ("finance_job", &["model:revenue"]),
                ("inventory_job", &["model:inventory", "model:stg_inventory"]),
                ("orders_job", &["model:orders", "model:stg_orders"]),
                ("shipments_job", &["model:shipments"]),
            ],
            deps: &[
                ("model:customers", &["model:stg_orders"]),
                ("model:inventory", &["model:shipments"]),
                ("model:orders", &["model:customers"]),
                ("model:revenue", &["model:orders"]),
                ("model:shipments", &["model:stg_inventory"]),
            ],
            expected_edges: &[
                ("customers_job", &["orders_job"]),
                ("finance_job", &["customers_job", "orders_job"]),
                ("inventory_job", &["shipments_job"]),
                ("orders_job", &["customers_job"]),
                ("shipments_job", &["inventory_job"]),
            ],
            expected_message: Some(two_cycles),
        },
        GroupWaitGraphTestCase {
            description: "a cycle through a seed one member builds for everyone adds the hint",
            selections: &[
                ("customers_job", &["model:customers"]),
                ("seeds_and_orders_job", &["seed:regions", "model:orders"]),
            ],
            deps: &[
                ("model:customers", &["seed:regions"]),
                ("model:orders", &["model:customers"]),
            ],
            expected_edges: &[
                ("customers_job", &["seeds_and_orders_job"]),
                ("seeds_and_orders_job", &["customers_job"]),
            ],
            expected_message: Some(seed_hub.leak()),
        },
        GroupWaitGraphTestCase {
            description: "members that both build a shared function never wait on each other for it",
            selections: &[
                (
                    "customers_job",
                    &["function:normalize_email", "model:customers"],
                ),
                ("orders_job", &["function:normalize_email", "model:orders"]),
            ],
            deps: &[
                (
                    "model:customers",
                    &["function:normalize_email", "model:orders"],
                ),
                ("model:orders", &["function:normalize_email"]),
            ],
            expected_edges: &[("customers_job", &["orders_job"]), ("orders_job", &[])],
            expected_message: None,
        },
    ];
    for test_case in test_cases {
        let waits = group_wait_graph(&map(test_case.selections), &map(test_case.deps));
        assert_eq!(
            waits.edges,
            map(test_case.expected_edges),
            "{}",
            test_case.description
        );
        assert_eq!(
            group_cycle_message("nightly", &waits.cycles).as_deref(),
            test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_large_group_on_a_long_chain_when_deriving_waits_then_one_cycle_is_reported_within_the_time_limit()
 {
    const MEMBERS: usize = 50;
    const NODES_PER_MEMBER: usize = 200;
    let node = |index: usize| format!("model:m{index}");
    let last = MEMBERS * NODES_PER_MEMBER - 1;
    let mut selections: BTreeMap<String, BTreeSet<String>> = (0..MEMBERS)
        .map(|member| {
            let first = member * NODES_PER_MEMBER;
            (
                format!("job_{member:03}"),
                (first..first + NODES_PER_MEMBER).map(node).collect(),
            )
        })
        .collect();
    // An acyclic chain of 10,000 nodes: each reads the one before it, and the first job also builds
    // a tail that reads the last node, so every member reaches every other through lineage.
    selections
        .get_mut("job_000")
        .expect("first member")
        .insert("model:tail".to_owned());
    let mut deps: BTreeMap<String, BTreeSet<String>> = (1..=last)
        .map(|index| (node(index), BTreeSet::from([node(index - 1)])))
        .collect();
    deps.insert("model:tail".to_owned(), BTreeSet::from([node(last)]));

    let started = std::time::Instant::now();
    let waits = group_wait_graph(&selections, &deps);
    let elapsed = started.elapsed();

    assert_eq!(waits.cycles.len(), 1);
    assert_eq!(waits.edges["job_000"].len(), MEMBERS - 1);
    assert_eq!(waits.edges[&format!("job_{:03}", MEMBERS - 1)].len(), MEMBERS - 1);
    assert!(
        elapsed < std::time::Duration::from_secs(2),
        "deriving waits for {MEMBERS} members over {} nodes took {elapsed:?}",
        last + 2
    );
}
