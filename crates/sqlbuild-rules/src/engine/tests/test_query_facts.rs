use serde_json::json;

use crate::engine::tests::helpers;
use crate::engine::tests::test_types::QueryFactsTestCase;

const IMPORT_RULES: &[&str] = &["SQBRMODEL101", "SQBRMODEL102"];
const ENUM_RULE: &[&str] = &["SQBRDECLARATION101"];
const NUMERIC_RULE: &[&str] = &["SQBRDECLARATION102"];
const OUTPUT_TYPE_RULE: &[&str] = &["SQBRCONTRACT105"];
const STATUS: &[&str] = &["status"];

#[test]
fn given_dialect_sql_when_reading_query_facts_then_findings_match_the_authored_sql()
-> Result<(), String> {
    let test_cases = [
        QueryFactsTestCase {
            description: "a Snowflake // comment holds no dependency call",
            dialect: "snowflake",
            query_sql: "WITH imported AS (\n  SELECT * FROM __ref(\"orders\")\n  // replaced __ref(\"orders_v1\")\n)\nSELECT order_id FROM imported",
            select: IMPORT_RULES,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[],
        },
        QueryFactsTestCase {
            description: "an apostrophe in a Snowflake // comment opens no string",
            dialect: "snowflake",
            query_sql: "WITH imported AS (\n  // don't read the archive\n  SELECT * FROM __ref(\"orders\")\n)\nSELECT order_id FROM imported",
            select: IMPORT_RULES,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[],
        },
        QueryFactsTestCase {
            description: "an apostrophe in a BigQuery # comment opens no string, reported unavailable until SQLBuild parses the dialect",
            dialect: "bigquery",
            query_sql: "WITH imported AS (\n  # don't read the archive\n  SELECT * FROM __ref(\"orders\")\n)\nSELECT order_id FROM imported",
            select: IMPORT_RULES,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[("L007", 1, 1, "cannot parse bigquery SQL for built-in rules")],
        },
        QueryFactsTestCase {
            description: "a DuckDB FROM-first import CTE star is exempt without a SELECT keyword",
            dialect: "duckdb",
            query_sql: "WITH imported AS (FROM __ref(\"orders\"))\nSELECT order_id FROM imported",
            select: IMPORT_RULES,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[],
        },
        QueryFactsTestCase {
            description: "a terminal DuckDB FROM-first star is anchored on its FROM keyword",
            dialect: "duckdb",
            query_sql: "WITH final AS (SELECT order_id FROM __ref(\"orders\"))\nFROM final",
            select: IMPORT_RULES,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[("SQBRMODEL102", 2, 1, "SELECT * is allowed only")],
        },
        QueryFactsTestCase {
            description: "a FROM-first subquery does not take the outer star's position",
            dialect: "duckdb",
            query_sql: "WITH imported AS (SELECT * FROM __ref(\"orders\"))\nSELECT * FROM (FROM imported)",
            select: &["SQBRMODEL102"],
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[("SQBRMODEL102", 2, 8, "SELECT * is allowed only")],
        },
        QueryFactsTestCase {
            description: "a SELECT subquery does not take the outer star's position",
            dialect: "duckdb",
            query_sql: "WITH imported AS (SELECT * FROM __ref(\"orders\"))\nSELECT * FROM (SELECT * FROM imported)",
            select: &["SQBRMODEL102"],
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[("SQBRMODEL102", 2, 8, "SELECT * is allowed only")],
        },
        QueryFactsTestCase {
            description: "stars in plain set-operation branches are each reported",
            dialect: "duckdb",
            query_sql: "WITH imported AS (SELECT * FROM __ref(\"orders\"))\nSELECT * FROM (SELECT order_id FROM imported)\nUNION ALL\nSELECT * FROM (SELECT order_id FROM imported) AS s",
            select: &["SQBRMODEL102"],
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[
                ("SQBRMODEL102", 2, 8, "SELECT * is allowed only"),
                ("SQBRMODEL102", 4, 8, "SELECT * is allowed only"),
            ],
        },
        QueryFactsTestCase {
            description: "stars over dynamic pivots in every set-operation branch are exempt",
            dialect: "snowflake",
            query_sql: "SELECT * FROM orders PIVOT(MAX(amount) FOR category IN (ANY))\nUNION ALL\nSELECT * FROM orders PIVOT(MAX(amount) FOR category IN (ANY))",
            select: &["SQBRMODEL102"],
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[],
        },
        QueryFactsTestCase {
            description: "only the plain star branch beside a dynamic-pivot branch is reported",
            dialect: "snowflake",
            query_sql: "SELECT * FROM orders PIVOT(MAX(amount) FOR category IN (ANY))\nUNION ALL\nSELECT * FROM (SELECT order_id FROM orders)",
            select: &["SQBRMODEL102"],
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[("SQBRMODEL102", 3, 8, "SELECT * is allowed only")],
        },
        QueryFactsTestCase {
            description: "a comparison of two literals keeps its source position",
            dialect: "duckdb",
            query_sql: "SELECT order_id FROM orders WHERE 3 < 7",
            select: NUMERIC_RULE,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[("SQBRDECLARATION102", 1, 35, "3 < 7")],
        },
        QueryFactsTestCase {
            description: "DuckDB literal comparisons on separate lines each keep their own position",
            dialect: "duckdb",
            query_sql: "SELECT order_id FROM orders\nWHERE 3 < 7\n  AND 5 > 2",
            select: NUMERIC_RULE,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[
                ("SQBRDECLARATION102", 2, 7, "3 < 7"),
                ("SQBRDECLARATION102", 3, 7, "5 > 2"),
            ],
        },
        QueryFactsTestCase {
            description: "Snowflake literal comparisons on separate lines each keep their own position",
            dialect: "snowflake",
            query_sql: "SELECT order_id FROM orders\nWHERE 3 < 7\n  AND 5 > 2",
            select: NUMERIC_RULE,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[
                ("SQBRDECLARATION102", 2, 7, "3 < 7"),
                ("SQBRDECLARATION102", 3, 7, "5 > 2"),
            ],
        },
        QueryFactsTestCase {
            description: "unaliased later union branches keep their cast types",
            dialect: "duckdb",
            query_sql: "SELECT CAST(1 AS INTEGER) AS customer_id, CAST('books' AS VARCHAR) AS category, CAST(10.25 AS DECIMAL(12,2)) AS amount\nUNION ALL\nSELECT CAST(1 AS INTEGER), CAST('games' AS VARCHAR), CAST(20.50 AS DECIMAL(12,2))\nUNION ALL\nSELECT CAST(2 AS INTEGER), CAST('books' AS VARCHAR), CAST(7.00 AS DECIMAL(12,2))",
            select: OUTPUT_TYPE_RULE,
            enum_columns: &[],
            columns: json!([{"name": "customer_id", "type": "INTEGER", "type_proven": true}, {"name": "category", "type": "VARCHAR", "type_proven": true}, {"name": "amount", "type": "DECIMAL(12,2)", "type_proven": true}]),
            expected_faults: &[],
        },
        QueryFactsTestCase {
            description: "an unaliased later union branch reports its own position",
            dialect: "duckdb",
            query_sql: "SELECT CAST(1 AS INTEGER) AS amount\nUNION ALL\nSELECT CAST(2 AS BIGINT)",
            select: OUTPUT_TYPE_RULE,
            enum_columns: &[],
            columns: json!([{"name": "amount", "type": "INTEGER", "type_proven": true}]),
            expected_faults: &[(
                "SQBRCONTRACT105",
                3,
                8,
                "which is not its declared contract type",
            )],
        },
        QueryFactsTestCase {
            description: "nested parenthesized set operations keep each branch position",
            dialect: "duckdb",
            query_sql: "(SELECT CAST(1 AS INTEGER) AS amount)\nUNION ALL\n((SELECT 2) EXCEPT (SELECT CAST(3 AS INTEGER)))",
            select: OUTPUT_TYPE_RULE,
            enum_columns: &[],
            columns: json!([{"name": "amount", "type": "INTEGER", "type_proven": true}]),
            expected_faults: &[(
                "SQBRCONTRACT105",
                3,
                10,
                "calculated without an outer explicit cast",
            )],
        },
        QueryFactsTestCase {
            description: "an unaliased branch after a subquery-filtered branch keeps its position",
            dialect: "duckdb",
            query_sql: "SELECT CAST(order_id AS INTEGER) AS amount FROM orders WHERE order_id IN (SELECT order_id FROM returns)\nUNION ALL BY NAME\nSELECT CAST(4 AS SMALLINT) AS amount",
            select: OUTPUT_TYPE_RULE,
            enum_columns: &[],
            columns: json!([{"name": "amount", "type": "INTEGER", "type_proven": true}]),
            expected_faults: &[(
                "SQBRCONTRACT105",
                3,
                8,
                "which is not its declared contract type",
            )],
        },
        QueryFactsTestCase {
            description: "unaliased union branches inside a final CTE keep their positions",
            dialect: "duckdb",
            query_sql: "WITH final AS (\n  SELECT CAST(1 AS INTEGER) AS amount\n  UNION ALL\n  SELECT 5 + 1\n)\nSELECT * FROM final",
            select: OUTPUT_TYPE_RULE,
            enum_columns: &[],
            columns: json!([{"name": "amount", "type": "INTEGER", "type_proven": true}]),
            expected_faults: &[(
                "SQBRCONTRACT105",
                4,
                10,
                "calculated without an outer explicit cast",
            )],
        },
        QueryFactsTestCase {
            description: "a BigQuery double-quoted string compared with an enum column, reported unavailable until SQLBuild parses the dialect",
            dialect: "bigquery",
            query_sql: "SELECT order_id FROM orders WHERE status = \"active\"",
            select: ENUM_RULE,
            enum_columns: STATUS,
            columns: json!([]),
            expected_faults: &[("L007", 1, 1, "cannot parse bigquery SQL for built-in rules")],
        },
        QueryFactsTestCase {
            description: "a BigQuery triple-quoted string compared with an enum column, reported unavailable until SQLBuild parses the dialect",
            dialect: "bigquery",
            query_sql: "SELECT order_id FROM orders WHERE status = '''active'''",
            select: ENUM_RULE,
            enum_columns: STATUS,
            columns: json!([]),
            expected_faults: &[("L007", 1, 1, "cannot parse bigquery SQL for built-in rules")],
        },
        QueryFactsTestCase {
            description: "a Postgres escape string compared with an enum column",
            dialect: "postgres",
            query_sql: "SELECT order_id FROM orders WHERE status = E'active'",
            select: ENUM_RULE,
            enum_columns: STATUS,
            columns: json!([]),
            expected_faults: &[("SQBRDECLARATION101", 1, 35, "")],
        },
        QueryFactsTestCase {
            description: "an interval unit is not an enum column",
            dialect: "duckdb",
            query_sql: "SELECT order_id FROM orders WHERE created_at > NOW() - INTERVAL 1 DAY",
            select: ENUM_RULE,
            enum_columns: &["day"],
            columns: json!([]),
            expected_faults: &[],
        },
        QueryFactsTestCase {
            description: "a DuckDB == comparison keeps its source position",
            dialect: "duckdb",
            query_sql: "SELECT order_id FROM orders WHERE amount == 42",
            select: NUMERIC_RULE,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[("SQBRDECLARATION102", 1, 35, "amount == 42")],
        },
        QueryFactsTestCase {
            description: "a parenthesized T-SQL TOP argument is not projection, reported unavailable until SQLBuild parses the dialect",
            dialect: "mssql",
            query_sql: "SELECT TOP (5) amount + 1 AS amount FROM orders",
            select: OUTPUT_TYPE_RULE,
            enum_columns: &[],
            columns: json!([{"name": "amount", "type": "INTEGER", "type_proven": true}]),
            expected_faults: &[("L007", 1, 1, "cannot parse mssql SQL for built-in rules")],
        },
        QueryFactsTestCase {
            description: "a T-SQL TOP PERCENT clause is not projection, reported unavailable until SQLBuild parses the dialect",
            dialect: "mssql",
            query_sql: "SELECT TOP 5 PERCENT amount + 1 AS amount FROM orders",
            select: OUTPUT_TYPE_RULE,
            enum_columns: &[],
            columns: json!([{"name": "amount", "type": "INTEGER", "type_proven": true}]),
            expected_faults: &[("L007", 1, 1, "cannot parse mssql SQL for built-in rules")],
        },
        QueryFactsTestCase {
            description: "a placeholder in relation position still parses",
            dialect: "duckdb",
            query_sql: "SELECT order_id FROM @@@orders_table WHERE amount > 7",
            select: NUMERIC_RULE,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[("SQBRDECLARATION102", 1, 44, "amount > 7")],
        },
        QueryFactsTestCase {
            description: "a placeholder name with non-ASCII word characters is replaced whole",
            dialect: "duckdb",
            query_sql: "SELECT order_id FROM orders WHERE amount > @@@seuil_minimé AND amount < 9",
            select: NUMERIC_RULE,
            enum_columns: &[],
            columns: json!([]),
            expected_faults: &[("SQBRDECLARATION102", 1, 64, "amount < 9")],
        },
    ];
    for test_case in test_cases {
        let faults = helpers::query_facts_faults(&test_case)?;
        let observed: Vec<(&str, u64, u64, &str)> = faults
            .iter()
            .enumerate()
            .map(|(index, (code, line, column, message))| {
                let shown = test_case
                    .expected_faults
                    .get(index)
                    .map(|expected| expected.3)
                    .filter(|fragment| message.contains(fragment))
                    .unwrap_or(message.as_str());
                (code.as_str(), *line, *column, shown)
            })
            .collect();
        assert_eq!(
            observed,
            test_case.expected_faults.to_vec(),
            "{}",
            test_case.description
        );
    }
    Ok(())
}
