use crate::sql_tree::errors::SqlTreeError;
use crate::sql_tree::main::sql_tree_nodes::sql_tree_nodes;
use crate::sql_tree::tests::helpers::{facts, shape};
use crate::sql_tree::tests::test_types::{
    TreeFactsTestCase, TreeFailureTestCase, TreeShapeTestCase,
};

const CTE_AND_JOIN_SQL: &str =
    "WITH x AS (SELECT 1 AS id)\nSELECT *\n  FROM x JOIN (SELECT 2 AS id) s ON x.id = s.id";

const SNOWFLAKE_MARKER_SQL: &str = "WITH o AS (SELECT * FROM __ref(\"orders\"))\nSELECT o.*, c.name\nFROM o JOIN __ref(\"customers\") c ON o.id = c.id JOIN (SELECT 2 AS id) s ON s.id = o.id\nWHERE o.id IN (SELECT id FROM __source(\"raw_returns\"))";

const NAMES_AND_LITERALS_SQL: &str =
    "SELECT o.\"Id\" AS order_id, 'x''y' AS s, 42 + 1\nFROM db.sch.orders AS o WHERE o.v = N'z'";

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
            description: "a partitioned Snowflake table function marker reads with its OVER clause inside TABLE( )",
            dialect: "snowflake",
            sql: "SELECT s.a\nFROM p, __table_fn(\"f\")(p.x) OVER (PARTITION BY p.k) s JOIN (SELECT 2 AS k) q ON 1=1",
            expected_nodes: vec![
                ("select", 1, 1, None),
                ("join", 2, 56, Some("select")),
                ("select", 2, 61, Some("subquery")),
                ("subquery", 2, 61, Some("join")),
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
fn given_sql_when_building_a_rule_tree_then_nodes_carry_names_literals_and_source_extents() {
    let test_cases = [
        TreeFactsTestCase {
            description: "names keep quoting and qualifiers; a literal without its own span has no end",
            dialect: "duckdb",
            sql: NAMES_AND_LITERALS_SQL,
            expected_facts: &[
                "select 1:1 to 2:41 \"SELECT o.\\\"Id\\\" AS order_id, 'x''y' AS s, 42 + 1\\nFROM db.sch.orders AS o WHERE o.v = N'z'\"",
                "alias 1:8 name=order_id to 1:26 \"o.\\\"Id\\\" AS order_id\"",
                "column 1:8 name=Id quoted in=o to 1:14 \"o.\\\"Id\\\"\"",
                "alias 1:28 name=s to 1:39 \"'x''y' AS s\"",
                "literal 1:28 string=x'y",
                "add 1:41 to 1:47 \"42 + 1\"",
                "literal 1:41 number=42",
                "literal 1:41 number=1",
                "table 2:6 name=orders in=db.sch to 2:19 \"db.sch.orders\"",
                "eq 2:31 to 2:41 \"o.v = N'z'\"",
                "column 2:31 name=v in=o to 2:34 \"o.v\"",
                "literal 2:31 national_string=z",
            ],
        },
        TreeFactsTestCase {
            description: "native PostgreSQL records the same names and spans, a typed literal as a cast",
            dialect: "postgres",
            sql: "SELECT o.\"Id\" AS order_id\nFROM db.sch.orders AS o WHERE o.v = DATE '2024-01-02'",
            expected_facts: &[
                "select 1:1 to 2:54 \"SELECT o.\\\"Id\\\" AS order_id\\nFROM db.sch.orders AS o WHERE o.v = DATE '2024-01-02'\"",
                "alias 1:8 name=order_id to 1:26 \"o.\\\"Id\\\" AS order_id\"",
                "column 1:8 name=Id quoted in=o to 1:14 \"o.\\\"Id\\\"\"",
                "table 2:6 name=orders in=db.sch to 2:19 \"db.sch.orders\"",
                "eq 2:31 to 2:54 \"o.v = DATE '2024-01-02'\"",
                "column 2:31 name=v in=o to 2:34 \"o.v\"",
                "cast 2:37 to 2:54 \"DATE '2024-01-02'\"",
                "literal 2:37 string=2024-01-02",
            ],
        },
        TreeFactsTestCase {
            description: "a marker over three lines is named by its call and ends where the call ends",
            dialect: "duckdb",
            sql: "SELECT c.id\nFROM __ref(\n  \"customers\"\n) AS c\nJOIN orders o ON o.cid = c.id",
            expected_facts: &[
                "select 1:1 to 5:30 \"SELECT c.id\\nFROM __ref(\\n  \\\"customers\\\"\\n) AS c\\nJOIN orders o ON o.cid = c.id\"",
                "join 5:1 to 5:30 \"JOIN orders o ON o.cid = c.id\"",
                "column 1:8 name=id in=c to 1:12 \"c.id\"",
                "table 2:6 name=__ref(\n  \"customers\"\n) to 4:2 \"__ref(\\n  \\\"customers\\\"\\n)\"",
                "table 5:6 name=orders to 5:12 \"orders\"",
                "eq 5:18 to 5:30 \"o.cid = c.id\"",
                "column 5:18 name=cid in=o to 5:23 \"o.cid\"",
                "column 5:26 name=id in=c to 5:30 \"c.id\"",
            ],
        },
        TreeFactsTestCase {
            description: "a Snowflake table function marker read inside TABLE( ) covers only its source call",
            dialect: "snowflake",
            sql: "SELECT l.k\nFROM __table_fn(\"lines_for\")('B-1', __ref(\"orders\").k) AS l JOIN t ON t.k = l.k",
            expected_facts: &[
                "select 1:1 to 2:80 \"SELECT l.k\\nFROM __table_fn(\\\"lines_for\\\")('B-1', __ref(\\\"orders\\\").k) AS l JOIN t ON t.k = l.k\"",
                "join 2:61 to 2:80 \"JOIN t ON t.k = l.k\"",
                "column 1:8 name=k in=l to 1:11 \"l.k\"",
                "alias 2:6 name=l to 2:60 \"__table_fn(\\\"lines_for\\\")('B-1', __ref(\\\"orders\\\").k) AS l\"",
                "function 2:6",
                "function 2:6 to 2:55 \"__table_fn(\\\"lines_for\\\")('B-1', __ref(\\\"orders\\\").k)\"",
                "literal 2:6 string=B-1",
                "column 2:37 name=k in=__ref(\"orders\") to 2:54 \"__ref(\\\"orders\\\").k\"",
                "table 2:66 name=t to 2:67 \"t\"",
                "eq 2:71 to 2:80 \"t.k = l.k\"",
                "column 2:71 name=k in=t to 2:74 \"t.k\"",
                "column 2:77 name=k in=l to 2:80 \"l.k\"",
            ],
        },
        TreeFactsTestCase {
            description: "a CTE is named; non-ASCII text keeps character offsets; a two-name marker is its call",
            dialect: "snowflake",
            sql: "WITH \"Top\" AS (SELECT 'café' AS n FROM __dbt_ref(\"shop\", \"raw\"))\nSELECT x.n FROM \"Top\" x",
            expected_facts: &[
                "select 1:1 to 2:24 \"WITH \\\"Top\\\" AS (SELECT 'café' AS n FROM __dbt_ref(\\\"shop\\\", \\\"raw\\\"))\\nSELECT x.n FROM \\\"Top\\\" x\"",
                "cte 1:6 name=Top quoted to 1:65 \"\\\"Top\\\" AS (SELECT 'café' AS n FROM __dbt_ref(\\\"shop\\\", \\\"raw\\\"))\"",
                "column 2:8 name=n in=x to 2:11 \"x.n\"",
                "table 2:17 name=Top quoted to 2:22 \"\\\"Top\\\"\"",
                "select 1:16 to 1:64 \"SELECT 'café' AS n FROM __dbt_ref(\\\"shop\\\", \\\"raw\\\")\"",
                "alias 1:23 name=n to 1:34 \"'café' AS n\"",
                "literal 1:23 string=café",
                "table 1:40 name=__dbt_ref(\"shop\", \"raw\") to 1:64 \"__dbt_ref(\\\"shop\\\", \\\"raw\\\")\"",
            ],
        },
    ];
    for test_case in test_cases {
        let nodes = sql_tree_nodes(test_case.sql, test_case.dialect);
        assert_eq!(
            nodes.as_deref().map(|nodes| facts(test_case.sql, nodes)),
            Ok(test_case
                .expected_facts
                .iter()
                .map(|fact| (*fact).to_owned())
                .collect()),
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
