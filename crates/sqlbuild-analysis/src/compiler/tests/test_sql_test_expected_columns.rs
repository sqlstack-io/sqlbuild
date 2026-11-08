use crate::compiler::_helpers::sql_tests::expected_columns::expected_columns;
use crate::compiler::_helpers::sql_tests::rendering::sql_test_parser;
use crate::compiler::tests::test_types::ExpectedColumnsTestCase;

#[test]
fn given_expected_cte_bodies_when_reading_columns_then_authored_names_or_none_are_returned() {
    let test_cases = [
        ExpectedColumnsTestCase {
            description: "aliases and bare or qualified columns",
            dialect: "duckdb",
            sql: "SELECT 1 AS order_id, o.customer_id, amount FROM orders AS o",
            expected_columns: Some(&["order_id", "customer_id", "amount"]),
        },
        ExpectedColumnsTestCase {
            description: "quoted identifiers keep authored spelling",
            dialect: "snowflake",
            sql: "SELECT 1 AS \"Order Id\", 'paid' AS Status",
            expected_columns: Some(&["\"Order Id\"", "Status"]),
        },
        ExpectedColumnsTestCase {
            description: "bracketed identifiers keep authored spelling",
            dialect: "tsql",
            sql: "SELECT 1 AS [Order Id], 2 AS amount",
            expected_columns: Some(&["[Order Id]", "amount"]),
        },
        ExpectedColumnsTestCase {
            description: "set operations use the first branch",
            dialect: "duckdb",
            sql: "SELECT 1 AS order_id, 10 AS amount UNION ALL SELECT 2, 20",
            expected_columns: Some(&["order_id", "amount"]),
        },
        ExpectedColumnsTestCase {
            description: "star over aliased values uses the alias list",
            dialect: "duckdb",
            sql: "SELECT * FROM (VALUES (1, 'paid'), (2, 'open')) AS t(order_id, status)",
            expected_columns: Some(&["order_id", "status"]),
        },
        ExpectedColumnsTestCase {
            description: "leading WITH uses the final projection",
            dialect: "duckdb",
            sql: "WITH picked AS (SELECT 1 AS a, 2 AS b) SELECT b FROM picked",
            expected_columns: Some(&["b"]),
        },
        ExpectedColumnsTestCase {
            description: "non-ASCII text before an identifier keeps its spelling",
            dialect: "duckdb",
            sql: "SELECT '订单' AS label, 1 AS \"数量\"",
            expected_columns: Some(&["label", "\"数量\""]),
        },
        ExpectedColumnsTestCase {
            description: "star from another relation is not explicit",
            dialect: "duckdb",
            sql: "SELECT * FROM orders",
            expected_columns: None,
        },
        ExpectedColumnsTestCase {
            description: "star with exclusions is not explicit",
            dialect: "duckdb",
            sql: "SELECT * EXCLUDE (amount) FROM (VALUES (1, 2)) AS t(order_id, amount)",
            expected_columns: None,
        },
        ExpectedColumnsTestCase {
            description: "unaliased expressions are not explicit",
            dialect: "duckdb",
            sql: "SELECT 1 AS order_id, amount + 1 FROM orders",
            expected_columns: None,
        },
        ExpectedColumnsTestCase {
            description: "duplicate names keep full-row comparison",
            dialect: "duckdb",
            sql: "SELECT 1 AS order_id, 2 AS ORDER_ID",
            expected_columns: None,
        },
        ExpectedColumnsTestCase {
            description: "unparsable SQL is not explicit",
            dialect: "duckdb",
            sql: "SELECT FROM WHERE",
            expected_columns: None,
        },
    ];

    for test_case in test_cases
        .into_iter()
        .filter(|test_case| sql_test_parser(Some(test_case.dialect)).is_some())
    {
        let parser = sql_test_parser(Some(test_case.dialect)).expect("native parser");
        let actual: Option<Vec<String>> = expected_columns(test_case.sql, parser);
        let actual_columns: Option<Vec<&str>> = actual
            .as_ref()
            .map(|columns| columns.iter().map(String::as_str).collect());
        assert_eq!(
            actual_columns.as_deref(),
            test_case.expected_columns,
            "{}",
            test_case.description
        );
    }
}
