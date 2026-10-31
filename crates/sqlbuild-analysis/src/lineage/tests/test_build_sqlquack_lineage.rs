use crate::lineage::tests::helpers::sqlquack_outcomes;
use crate::lineage::tests::test_types::SqlQuackLineageTestCase;

/// Each case is a difference from polyglot's lineage that SQLQuack's analysis fixes.
#[test]
fn given_models_when_building_sqlquack_lineage_then_each_column_reads_what_its_value_reads() {
    let test_cases = [
        SqlQuackLineageTestCase {
            description: "a value computed through a CTE keeps its derivation, not the outer select's",
            dialect: "snowflake",
            models: &[(
                "order_totals",
                "WITH t AS (SELECT o.amount * 2 AS doubled FROM __ref('orders') o) SELECT doubled FROM t",
            )],
            expected_lines: &["doubled expression high unknown [model:orders.amount]"],
            expected_has_star: false,
        },
        SqlQuackLineageTestCase {
            description: "a literal does not take the lineage of an upstream column of the same name",
            dialect: "snowflake",
            models: &[(
                "tagged_orders",
                "WITH t AS (SELECT 'web'::VARCHAR AS customer_id, o.order_id FROM __ref('orders') o) \
                 SELECT customer_id, order_id FROM t",
            )],
            expected_lines: &[
                "customer_id constant high unknown []",
                "order_id direct high unknown [model:orders.order_id]",
            ],
            expected_has_star: false,
        },
        SqlQuackLineageTestCase {
            description: "a quoted column the schema lists quoted matches its exact spelling",
            dialect: "snowflake",
            models: &[(
                "listing_snapshot",
                "SELECT e.\"listingId\", e.status FROM __ref('listing_export') AS e",
            )],
            expected_lines: &[
                "listingId direct high unknown [model:listing_export.listingId]",
                "status direct high unknown [model:listing_export.status]",
            ],
            expected_has_star: false,
        },
        SqlQuackLineageTestCase {
            description: "EXCEPT outputs read only the left branch",
            dialect: "snowflake",
            models: &[(
                "orphan_customers",
                "SELECT customer_id FROM __ref('orders') MINUS SELECT customer_id FROM __ref('customers')",
            )],
            expected_lines: &["customer_id direct high unknown [model:orders.customer_id]"],
            expected_has_star: false,
        },
        SqlQuackLineageTestCase {
            description: "a table function's columns come from the function",
            dialect: "snowflake",
            models: &[(
                "recent_orders",
                "SELECT r.order_id FROM TABLE(__table_fn('orders_since')('2026-01-01')) AS r",
            )],
            expected_lines: &["order_id direct high unknown [table_fn:orders_since.order_id]"],
            expected_has_star: false,
        },
        SqlQuackLineageTestCase {
            description: "a model the schema does not know is read by the outputs its lineage found",
            dialect: "duckdb",
            models: &[
                (
                    "order_amounts",
                    "SELECT order_id, amount AS order_amount FROM __ref('orders')",
                ),
                ("order_report", "SELECT * FROM __ref('order_amounts')"),
            ],
            expected_lines: &[
                "order_id direct high unknown [model:order_amounts.order_id]",
                "order_amount direct high unknown [model:order_amounts.order_amount]",
            ],
            expected_has_star: false,
        },
    ];
    for test_case in test_cases {
        let mut outcomes = sqlquack_outcomes(test_case.dialect, test_case.models);
        let (status, lines, has_star) = outcomes.pop().expect("one outcome per model");
        assert_eq!(status, "built", "{}", test_case.description);
        assert_eq!(lines, test_case.expected_lines, "{}", test_case.description);
        assert_eq!(
            has_star, test_case.expected_has_star,
            "{}",
            test_case.description
        );
    }
}
