use serde_json::json;

use crate::engine::tests::helpers::{CROSS_JOIN_CODE, CROSS_JOIN_MODEL_PATH, finalize_cross_joins};
use crate::engine::tests::test_types::{RelationExceptionTestCase, StaleRelationExceptionTestCase};

#[test]
fn given_relation_level_exceptions_when_finalizing_then_exactly_the_named_joins_are_excepted()
-> Result<(), String> {
    let test_cases = [
        RelationExceptionTestCase {
            description: "a relation exception covers only that relation",
            exceptions: json!([{"rule": CROSS_JOIN_CODE, "path": CROSS_JOIN_MODEL_PATH, "relation": "order_policy", "reason": "Every order meets every policy."}]),
            expected_messages: &[
                "CROSS JOIN with `customers` in CTE `checks` creates an explicit cartesian product",
            ],
        },
        RelationExceptionTestCase {
            description: "a CTE-scoped exception covers the join in that CTE only",
            exceptions: json!([{"rule": CROSS_JOIN_CODE, "path": CROSS_JOIN_MODEL_PATH, "relation": "customers", "cte": "checks", "reason": "Deliberate."}]),
            expected_messages: &[
                "CROSS JOIN with `order_policy` creates an explicit cartesian product",
            ],
        },
        RelationExceptionTestCase {
            description: "relation names match case-insensitively",
            exceptions: json!([
                {"rule": CROSS_JOIN_CODE, "path": CROSS_JOIN_MODEL_PATH, "relation": "Order_Policy", "reason": "Every order meets every policy."},
                {"rule": CROSS_JOIN_CODE, "path": CROSS_JOIN_MODEL_PATH, "relation": "customers", "reason": "Deliberate."}
            ]),
            expected_messages: &[],
        },
    ];

    for test_case in test_cases {
        let kept = finalize_cross_joins(test_case.exceptions.clone())?;

        let messages: Vec<&str> = kept.iter().map(|fault| fault.message.as_str()).collect();
        assert_eq!(
            messages, test_case.expected_messages,
            "{}",
            test_case.description
        );
    }
    Ok(())
}

#[test]
fn given_relation_level_exceptions_matching_nothing_when_finalizing_then_they_are_stale() {
    let test_cases = [
        StaleRelationExceptionTestCase {
            description: "a relation exception that matches nothing is stale",
            exceptions: json!([{"rule": CROSS_JOIN_CODE, "path": CROSS_JOIN_MODEL_PATH, "relation": "shipments", "reason": "Gone."}]),
            expected_error: "stale rule exception suppresses no finding: SQLCLEAN038 at models/orders.sql (relation shipments)",
        },
        StaleRelationExceptionTestCase {
            description: "a CTE-scoped exception for another CTE is stale",
            exceptions: json!([{"rule": CROSS_JOIN_CODE, "path": CROSS_JOIN_MODEL_PATH, "relation": "customers", "cte": "other", "reason": "Gone."}]),
            expected_error: "stale rule exception suppresses no finding: SQLCLEAN038 at models/orders.sql (relation customers in CTE other)",
        },
    ];

    for test_case in test_cases {
        let error: String = finalize_cross_joins(test_case.exceptions.clone())
            .err()
            .unwrap_or_default();

        assert!(
            error.contains(test_case.expected_error),
            "{}: {error}",
            test_case.description
        );
    }
}
