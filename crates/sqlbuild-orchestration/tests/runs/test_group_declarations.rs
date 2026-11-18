use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use uuid::Uuid;

use crate::declarations::main::group_wait_reasons::group_wait_reasons;
use crate::declarations::main::snapshot_definitions::snapshot_definitions;
use crate::declarations::models::JobDefinition;
use crate::runs::tests::helpers::snapshot;
use crate::runs::tests::test_types::{DeclarationCheckTestCase, WaitReasonTestCase};
use crate::store::errors::ControlError;

#[test]
fn given_group_members_when_publishing_then_each_wait_has_its_node_level_reasons() {
    let test_cases = [WaitReasonTestCase {
        description: "through a node no member selects, and never through another member's node",
        deps: &[
            ("products", &["orders"]),
            ("customers", &["products"]),
            ("inventory", &["customers"]),
        ],
        jobs: json!({
            "load_orders": {"description": "Orders", "selection": ["orders"]},
            "build_customers": {"description": "Customers", "selection": ["customers"]},
            "build_inventory": {"description": "Inventory", "selection": ["inventory"]},
        }),
        waits: json!({
            "load_orders": [],
            "build_customers": ["load_orders"],
            "build_inventory": ["build_customers", "load_orders"],
        }),
        expected_reasons: json!({
            "build_customers": [
                {"upstream_job": "load_orders", "from_node": "orders", "to_node": "customers"}],
            "build_inventory": [
                {"upstream_job": "build_customers", "from_node": "customers", "to_node": "inventory"}],
        }),
    }];
    for test_case in test_cases {
        let mut input = snapshot(Uuid::nil());
        for (node, upstream) in test_case.deps {
            input.nodes.get_mut(*node).expect("node").definition["deps"] = json!(upstream);
        }
        let jobs: BTreeMap<String, JobDefinition> =
            serde_json::from_value(test_case.jobs).expect("jobs");
        let waits: BTreeMap<String, BTreeSet<String>> =
            serde_json::from_value(test_case.waits).expect("waits");
        assert_eq!(
            json!(group_wait_reasons(&input, &jobs, &waits)),
            test_case.expected_reasons,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_limits_or_reasons_when_publishing_then_the_snapshot_is_refused() {
    let test_cases = [
        DeclarationCheckTestCase {
            description: "an amount above its pool's limit",
            jobs: json!({"orders": {"description": "Orders", "selection": ["orders"],
                "pools": {"warehouse": {"amount": 2, "limit": 1}}}}),
            groups: json!({}),
            expected_error: "orders takes more slots of pool warehouse than its limit",
        },
        DeclarationCheckTestCase {
            description: "one group with two limits",
            jobs: json!({
                "orders": {"description": "Orders", "selection": ["orders"],
                    "pools": {"warehouse": {"amount": 1, "limit": 1}}},
                "customers": {"description": "Customers", "selection": ["customers"],
                    "pools": {"warehouse": {"amount": 1, "limit": 2}}},
            }),
            groups: json!({}),
            expected_error: "pool warehouse has more than one limit",
        },
        DeclarationCheckTestCase {
            description: "a wait reason naming a node outside the snapshot",
            jobs: json!({
                "orders": {"description": "Orders", "selection": ["orders"]},
                "customers": {"description": "Customers", "selection": ["customers"]},
            }),
            groups: json!({"daily": {"description": "Daily", "selection": ["customers", "orders"],
                "jobs": {"orders": [], "customers": []},
                "wait_reasons": {"customers": [
                    {"upstream_job": "orders", "from_node": "returns", "to_node": "customers"}]}}}),
            expected_error: "group daily has wait reasons outside its members' waits or the snapshot",
        },
    ];
    for test_case in test_cases {
        let mut input = snapshot(Uuid::nil());
        input.documents.insert("jobs".into(), test_case.jobs);
        input.documents.insert("groups".into(), test_case.groups);
        let refused = snapshot_definitions(&input).map(|_| ());
        assert!(
            matches!(&refused, Err(ControlError::Invalid(message)) if message == test_case.expected_error),
            "{}: {refused:?}",
            test_case.description
        );
    }
}
