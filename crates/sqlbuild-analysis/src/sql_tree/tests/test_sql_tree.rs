use crate::sql_tree::errors::SqlTreeError;
use crate::sql_tree::main::sql_tree_nodes::sql_tree_nodes;
use crate::sql_tree::tests::helpers::shape;
use crate::sql_tree::tests::test_types::{TreeFailureTestCase, TreeShapeTestCase};

const CTE_AND_JOIN_SQL: &str =
    "WITH x AS (SELECT 1 AS id)\nSELECT *\n  FROM x JOIN (SELECT 2 AS id) s ON x.id = s.id";

const SNOWFLAKE_MARKER_SQL: &str = "WITH o AS (SELECT * FROM __ref(\"orders\"))\nSELECT o.*, c.name\nFROM o JOIN __ref(\"customers\") c ON o.id = c.id JOIN (SELECT 2 AS id) s ON s.id = o.id\nWHERE o.id IN (SELECT id FROM __source(\"raw_returns\"))";

#[test]
fn given_sql_when_building_a_rule_tree_then_clauses_own_their_queries_at_their_source_positions() {
    let test_cases = [
        TreeShapeTestCase {
            description: "native PostgreSQL records every clause, star included",
            dialect: "postgres",
            sql: CTE_AND_JOIN_SQL,
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("cte", 1, 6, Some("select")),
                ("select", 1, 12, Some("cte")),
                ("star", 2, 8, Some("select")),
                ("join", 3, 10, Some("select")),
                ("subquery", 3, 15, Some("join")),
                ("select", 3, 16, Some("subquery")),
            ],
        },
        TreeShapeTestCase {
            description: "the shared DuckDB parser records a bare star at its own position and a subquery's select at its parent's",
            dialect: "duckdb",
            sql: CTE_AND_JOIN_SQL,
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("cte", 1, 6, Some("select")),
                ("select", 1, 12, Some("cte")),
                ("star", 2, 8, Some("select")),
                ("join", 3, 10, Some("select")),
                ("select", 3, 15, Some("subquery")),
                ("subquery", 3, 15, Some("join")),
            ],
        },
        TreeShapeTestCase {
            description: "a qualified star starts at its qualifier; a scalar subquery's select at the subquery",
            dialect: "snowflake",
            sql: "SELECT o.*,\n  (SELECT MAX(id) FROM orders) AS top_id\nFROM orders o",
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("star", 1, 8, Some("select")),
                ("select", 2, 3, Some("subquery")),
                ("subquery", 2, 3, Some("alias")),
            ],
        },
        TreeShapeTestCase {
            description: "Snowflake reads relation markers as tables; nodes after them keep their columns",
            dialect: "snowflake",
            sql: SNOWFLAKE_MARKER_SQL,
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("cte", 1, 6, Some("select")),
                ("select", 1, 12, Some("cte")),
                ("star", 1, 19, Some("select")),
                ("star", 2, 8, Some("select")),
                ("join", 3, 8, Some("select")),
                ("join", 3, 49, Some("select")),
                ("select", 3, 54, Some("subquery")),
                ("subquery", 3, 54, Some("join")),
                ("select", 4, 7, Some("in")),
            ],
        },
        TreeShapeTestCase {
            description: "a Snowflake table function marker reads as SQLBuild renders it, inside TABLE( ), at its columns",
            dialect: "snowflake",
            sql: "SELECT l.*\nFROM __table_fn(\"lines_for\")('B-1', __ref(\"orders\").k) AS l JOIN (SELECT 1 AS k) s ON s.k = l.k",
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("star", 1, 8, Some("select")),
                ("join", 2, 61, Some("select")),
                ("select", 2, 66, Some("subquery")),
                ("subquery", 2, 66, Some("join")),
            ],
        },
        TreeShapeTestCase {
            description: "a Snowflake table function marker closing a subquery keeps the columns after it",
            dialect: "snowflake",
            sql: "SELECT 1\nFROM (SELECT * FROM __table_fn(\"f\")(1)) t JOIN (SELECT 2 AS k) s ON s.k = t.k",
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("select", 2, 6, Some("subquery")),
                ("subquery", 2, 6, Some("select")),
                ("star", 2, 14, Some("select")),
                ("join", 2, 43, Some("select")),
                ("select", 2, 48, Some("subquery")),
                ("subquery", 2, 48, Some("join")),
            ],
        },
        TreeShapeTestCase {
            description: "a Snowflake table function marker ending its line keeps its arguments' columns",
            dialect: "snowflake",
            sql: "SELECT 1\nFROM __table_fn(\"f\")((SELECT 9 AS z))\nJOIN (SELECT 2 AS k) s ON 1=1",
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("select", 2, 22, Some("subquery")),
                ("subquery", 2, 22, Some("function")),
                ("join", 3, 1, Some("select")),
                ("select", 3, 6, Some("subquery")),
                ("subquery", 3, 6, Some("join")),
            ],
        },
        TreeShapeTestCase {
            description: "a Snowflake table function marker already inside TABLE( ) is not wrapped twice",
            dialect: "snowflake",
            sql: "SELECT 1\nFROM TABLE(__table_fn(\"f\")(1)) t JOIN (SELECT 2 AS k) s ON s.k = t.k",
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("join", 2, 34, Some("select")),
                ("select", 2, 39, Some("subquery")),
                ("subquery", 2, 39, Some("join")),
            ],
        },
        TreeShapeTestCase {
            description: "a comma-separated FROM item after an explicit join is not a join",
            dialect: "duckdb",
            sql: "SELECT 1\nFROM a JOIN b ON a.id = b.id, (SELECT 2) c",
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("join", 2, 8, Some("select")),
                ("select", 2, 31, Some("subquery")),
                ("subquery", 2, 31, Some("select")),
            ],
        },
    ];
    for test_case in test_cases {
        let nodes = sql_tree_nodes(test_case.sql, test_case.dialect);
        assert_eq!(
            nodes.as_deref().map(shape),
            Ok(test_case.expected_nodes),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_sql_without_a_full_tree_when_building_a_rule_tree_then_the_reason_is_reported() {
    let test_cases = [
        TreeFailureTestCase {
            description: "a dialect SQLQuack does not parse",
            dialect: "bigquery",
            sql: "SELECT 1".to_owned(),
            expected_error: |error| matches!(error, SqlTreeError::UnavailableDialect(name) if name == "bigquery"),
        },
        TreeFailureTestCase {
            description: "a PostgreSQL syntax error",
            dialect: "postgres",
            sql: "SELECT FROM WHERE".to_owned(),
            expected_error: |error| matches!(error, SqlTreeError::Parse(_)),
        },
        TreeFailureTestCase {
            description: "PostgreSQL's stack depth limit is unanalysable, not a syntax error",
            dialect: "postgres",
            sql: format!("SELECT {}1{}", "(".repeat(10_000), ")".repeat(10_000)),
            expected_error: |error| matches!(error, SqlTreeError::Unanalysable(_)),
        },
        TreeFailureTestCase {
            description: "the shared parser's nesting limit is unanalysable, not a syntax error",
            dialect: "duckdb",
            sql: format!("SELECT {}1{}", "(".repeat(1_000), ")".repeat(1_000)),
            expected_error: |error| matches!(error, SqlTreeError::Unanalysable(_)),
        },
        TreeFailureTestCase {
            description: "a shared-parser syntax error",
            dialect: "sqlite",
            sql: "SELECT FROM WHERE (".to_owned(),
            expected_error: |error| matches!(error, SqlTreeError::Parse(_)),
        },
    ];
    for test_case in test_cases {
        let error = sql_tree_nodes(&test_case.sql, test_case.dialect).err();
        assert!(
            error.as_ref().is_some_and(test_case.expected_error),
            "{}: {error:?}",
            test_case.description
        );
    }
}
