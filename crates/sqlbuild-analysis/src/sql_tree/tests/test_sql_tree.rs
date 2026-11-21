use crate::sql_tree::errors::SqlTreeError;
use crate::sql_tree::main::sql_tree_nodes::sql_tree_nodes;
use crate::sql_tree::tests::helpers::{
    clauses, expression_facts, facts, shape, statement_children,
};
use crate::sql_tree::tests::test_types::{
    TreeChildrenTestCase, TreeClausesTestCase, TreeFactsTestCase, TreeFailureTestCase,
    TreeShapeTestCase,
};

const CTE_AND_JOIN_SQL: &str =
    "WITH x AS (SELECT 1 AS id)\nSELECT *\n  FROM x JOIN (SELECT 2 AS id) s ON x.id = s.id";

const SNOWFLAKE_MARKER_SQL: &str = "WITH o AS (SELECT * FROM __ref(\"orders\"))\nSELECT o.*, c.name\nFROM o JOIN __ref(\"customers\") c ON o.id = c.id JOIN (SELECT 2 AS id) s ON s.id = o.id\nWHERE o.id IN (SELECT id FROM __source(\"raw_returns\"))";

const CLAUSES_SQL: &str = "SELECT o.order_id, o.total > 2, COUNT(*) AS c\nFROM orders o\nWHERE o.is_paid\nGROUP BY o.order_id, o.region\nHAVING COUNT(*) > 1\nQUALIFY ROW_NUMBER() OVER (PARTITION BY o.order_id ORDER BY o.placed_at) = 1\nORDER BY o.order_id\nLIMIT 5";

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
                ("star", 2, 8, Some("projection")),
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
                ("star", 2, 8, Some("projection")),
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
                ("star", 1, 8, Some("projection")),
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
                ("star", 1, 19, Some("projection")),
                ("star", 2, 8, Some("projection")),
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
                ("star", 1, 8, Some("projection")),
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
                ("subquery", 2, 6, Some("from")),
                ("star", 2, 14, Some("projection")),
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
                ("subquery", 2, 31, Some("from")),
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
            nodes
                .as_deref()
                .map(|nodes| expression_facts(test_case.sql, nodes)),
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

#[test]
fn given_a_query_when_building_a_rule_tree_then_each_clause_owns_its_expressions() {
    let every_clause = || {
        vec![
            ("projection", vec!["column 1:8", "gt 1:20", "alias 1:33"]),
            ("from", vec!["table 2:6"]),
            ("where", vec!["column 3:7"]),
            ("group_by", vec!["column 4:10", "column 4:22"]),
            ("having", vec!["gt 5:8"]),
            ("qualify", vec!["eq 6:9"]),
            ("order_by", vec!["column 7:10"]),
            ("limit", vec!["literal 1:1"]),
            ("partition_by", vec!["column 6:41"]),
            ("window_order_by", vec!["column 6:61"]),
        ]
    };
    let test_cases = [
        TreeClausesTestCase {
            description: "Snowflake: every clause, and a window's partition and order keys apart",
            dialect: "snowflake",
            sql: CLAUSES_SQL,
            expected_clauses: every_clause(),
        },
        TreeClausesTestCase {
            description: "DuckDB: every clause, and a window's partition and order keys apart",
            dialect: "duckdb",
            sql: CLAUSES_SQL,
            expected_clauses: every_clause(),
        },
        TreeClausesTestCase {
            description: "a set operation's ORDER BY and LIMIT belong to the whole query",
            dialect: "duckdb",
            sql: "SELECT a FROM t\nUNION ALL\nSELECT a FROM u\nORDER BY a\nLIMIT 3",
            expected_clauses: vec![
                ("order_by", vec!["column 4:10"]),
                ("limit", vec!["literal 1:1"]),
                ("projection", vec!["column 1:8"]),
                ("from", vec!["table 1:15"]),
                ("projection", vec!["column 3:8"]),
                ("from", vec!["table 3:15"]),
            ],
        },
        TreeClausesTestCase {
            description: "a comma-separated source after an explicit join belongs to FROM",
            dialect: "snowflake",
            sql: "SELECT 1\nFROM a JOIN b ON a.k = b.k, c",
            expected_clauses: vec![
                ("projection", vec!["literal 1:1"]),
                ("from", vec!["table 2:6", "table 2:29"]),
            ],
        },
        TreeClausesTestCase {
            description: "SQLite: an UPDATE's WHERE, ORDER BY and LIMIT own their expressions",
            dialect: "sqlite",
            sql: "UPDATE t SET a = 1\nWHERE b = 2\nORDER BY c\nLIMIT 3",
            expected_clauses: vec![
                ("where", vec!["eq 2:7"]),
                ("order_by", vec!["column 3:10"]),
                ("limit", vec!["literal 1:1"]),
            ],
        },
        TreeClausesTestCase {
            description: "Snowflake: an UPDATE's WHERE owns its predicate",
            dialect: "snowflake",
            sql: "UPDATE t SET a = 1\nFROM u\nWHERE t.b = u.b",
            expected_clauses: vec![("where", vec!["eq 3:7"])],
        },
        TreeClausesTestCase {
            description: "DuckDB: an UPDATE's WHERE owns its predicate",
            dialect: "duckdb",
            sql: "UPDATE t SET a = 1\nWHERE b = 2",
            expected_clauses: vec![("where", vec!["eq 2:7"])],
        },
        TreeClausesTestCase {
            description: "a DELETE's WHERE owns its predicate",
            dialect: "snowflake",
            sql: "DELETE FROM t\nWHERE t.a = 1",
            expected_clauses: vec![("where", vec!["eq 2:7"])],
        },
    ];

    for test_case in test_cases {
        let nodes = sql_tree_nodes(test_case.sql, test_case.dialect).expect(test_case.description);
        let found: Vec<(&str, Vec<String>)> = clauses(&nodes);
        let described: Vec<(&str, Vec<&str>)> = found
            .iter()
            .map(|(kind, children)| (*kind, children.iter().map(String::as_str).collect()))
            .collect();

        assert_eq!(
            described, test_case.expected_clauses,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_clause_without_its_own_span_when_building_a_rule_tree_then_it_covers_its_children() {
    let test_cases = [
        TreeFactsTestCase {
            description: "Snowflake: each clause runs from its first child's start to its last child's end",
            dialect: "snowflake",
            sql: "SELECT a, b\nFROM t\nWHERE a > 1",
            expected_facts: &[
                "select 1:1 to 3:12 \"SELECT a, b\\nFROM t\\nWHERE a > 1\"",
                "projection 1:8 to 1:12 \"a, b\"",
                "from 2:6 to 2:7 \"t\"",
                "where 3:7 to 3:12 \"a > 1\"",
                "column 1:8 name=a to 1:9 \"a\"",
                "column 1:11 name=b to 1:12 \"b\"",
                "table 2:6 name=t to 2:7 \"t\"",
                "gt 3:7 to 3:12 \"a > 1\"",
                "column 3:7 name=a to 3:8 \"a\"",
                "literal 3:7 number=1",
            ],
        },
        TreeFactsTestCase {
            description: "DuckDB: a clause whose children have no span has none, and reports its statement's start",
            dialect: "duckdb",
            sql: "SELECT a\nFROM t\nLIMIT 5",
            expected_facts: &[
                "select 1:1 to 3:8 \"SELECT a\\nFROM t\\nLIMIT 5\"",
                "projection 1:8 to 1:9 \"a\"",
                "from 2:6 to 2:7 \"t\"",
                "limit 1:1",
                "column 1:8 name=a to 1:9 \"a\"",
                "table 2:6 name=t to 2:7 \"t\"",
                "literal 1:1 number=5",
            ],
        },
    ];

    for test_case in test_cases {
        let nodes = sql_tree_nodes(test_case.sql, test_case.dialect).expect(test_case.description);

        assert_eq!(
            facts(test_case.sql, &nodes),
            test_case.expected_facts,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_query_when_building_a_rule_tree_then_its_clauses_come_in_the_order_written() {
    const EVERY_CLAUSE_SQL: &str = "WITH x AS (SELECT 1 AS id)\nSELECT x.id\nFROM x\nJOIN y ON y.id = x.id\nLEFT JOIN z ON z.id = x.id\nWHERE x.id > 0\nGROUP BY x.id\nHAVING COUNT(*) > 0\nQUALIFY ROW_NUMBER() OVER (ORDER BY x.id) = 1\nORDER BY x.id\nLIMIT 3";
    const WRITTEN: &[&str] = &[
        "cte",
        "projection",
        "from",
        "join",
        "join",
        "where",
        "group_by",
        "having",
        "qualify",
        "order_by",
        "limit",
    ];
    let test_cases = [
        TreeChildrenTestCase {
            description: "Snowflake: CTEs, the select list, FROM, its joins, then the other clauses",
            dialect: "snowflake",
            sql: EVERY_CLAUSE_SQL,
            expected_children: WRITTEN,
        },
        TreeChildrenTestCase {
            description: "DuckDB: CTEs, the select list, FROM, its joins, then the other clauses",
            dialect: "duckdb",
            sql: EVERY_CLAUSE_SQL,
            expected_children: WRITTEN,
        },
    ];

    for test_case in test_cases {
        let nodes = sql_tree_nodes(test_case.sql, test_case.dialect).expect(test_case.description);

        assert_eq!(
            statement_children(&nodes),
            test_case.expected_children,
            "{}",
            test_case.description
        );
    }
}
