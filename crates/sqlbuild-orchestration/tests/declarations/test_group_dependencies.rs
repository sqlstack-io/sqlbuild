//! A group member waits for the members building the first nodes it reaches upstream.

use std::collections::{BTreeMap, BTreeSet};

use crate::declarations::tests::helpers::waits;
use crate::declarations::tests::test_types::{GroupCycleTestCase, GroupWaitsTestCase};

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
            expected_waits: vec![
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
            expected_waits: vec![
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
            expected_waits: vec![("load_job", vec![]), ("customers_job", vec!["load_job"])],
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
            expected_waits: vec![
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
            expected_waits: vec![
                ("load_job", vec![]),
                ("orders_staging_job", vec!["load_job"]),
                ("inventory_staging_job", vec!["load_job"]),
            ],
        },
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
            expected_waits: vec![
                ("a_job", vec![]),
                ("b_job", vec![]),
                ("c_job", vec![]),
                ("functions_job", vec![]),
                ("staging_job", vec!["a_job"]),
                ("marts_job", vec!["staging_job"]),
            ],
        },
    ];
    for test_case in test_cases {
        let actual = waits(&test_case.lineage, &test_case.jobs).expect(test_case.description);
        assert_eq!(
            actual,
            test_case
                .expected_waits
                .iter()
                .map(|(name, deps)| {
                    (
                        (*name).to_owned(),
                        deps.iter().map(|dep| (*dep).to_owned()).collect(),
                    )
                })
                .collect::<BTreeMap<String, BTreeSet<String>>>(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_members_reading_each_other_when_deriving_group_waits_then_the_cycle_names_both_waits() {
    let test_cases = [GroupCycleTestCase {
        description: "two members reading each other's nodes",
        lineage: vec![
            ("model:stg_orders", vec!["model:customers"]),
            ("model:customers", vec!["model:stg_orders"]),
        ],
        jobs: vec![
            ("orders_job", vec!["model:stg_orders"]),
            ("customers_job", vec!["model:customers"]),
        ],
        expected_error: "invalid control-plane data: group dependencies contain a cycle in job group \
                         orders_job_group: customers_job waits for orders_job (model:customers reads \
                         model:stg_orders); orders_job waits for customers_job (model:stg_orders reads \
                         model:customers)",
    }];
    for test_case in test_cases {
        let error = waits(&test_case.lineage, &test_case.jobs).expect_err("a cycle is rejected");
        assert_eq!(
            error.to_string(),
            test_case.expected_error,
            "{}",
            test_case.description
        );
    }
}
