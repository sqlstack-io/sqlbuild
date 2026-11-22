//! An expansion never opens or extends a comment where it lands.

use crate::sql_scan::main::dialect_lexical_syntax::dialect_lexical_syntax;
use crate::sql_scan::models::LexicalSyntax;
use crate::sql_scan::tests::boundary_helpers::{generated_failures, spliced};
use crate::sql_scan::tests::test_types::{BoundaryTestCase, GeneratedBoundaryTestCase};

#[test]
fn given_hostile_expansions_in_every_position_when_bounded_then_only_changed_code_changes() {
    let test_cases = [
        GeneratedBoundaryTestCase {
            description: "generic",
            syntax: dialect_lexical_syntax("duckdb"),
            expected_failures: &[],
        },
        GeneratedBoundaryTestCase {
            description: "BigQuery",
            syntax: dialect_lexical_syntax("bigquery"),
            expected_failures: &[],
        },
        GeneratedBoundaryTestCase {
            description: "Snowflake",
            syntax: dialect_lexical_syntax("snowflake"),
            expected_failures: &[],
        },
        GeneratedBoundaryTestCase {
            description: "ClickHouse",
            syntax: dialect_lexical_syntax("clickhouse"),
            expected_failures: &[],
        },
        GeneratedBoundaryTestCase {
            description: "the ClickHouse adapter's prefixes",
            syntax: LexicalSyntax {
                line_comment_prefixes: vec!["--".into(), "#".into(), "//".into()],
                ..dialect_lexical_syntax("clickhouse")
            },
            expected_failures: &[],
        },
        GeneratedBoundaryTestCase {
            description: "Databricks",
            syntax: dialect_lexical_syntax("databricks"),
            expected_failures: &[],
        },
        GeneratedBoundaryTestCase {
            description: "T-SQL",
            syntax: dialect_lexical_syntax("tsql"),
            expected_failures: &[],
        },
    ];
    for test_case in test_cases {
        let failures: Vec<String> = generated_failures(&test_case.syntax);
        assert_eq!(
            failures.iter().take(12).collect::<Vec<_>>(),
            test_case.expected_failures.iter().collect::<Vec<_>>(),
            "{}: {} failures",
            test_case.description,
            failures.len()
        );
    }
}

#[test]
fn given_a_boundary_when_bounded_then_only_a_space_or_newline_is_added() {
    let test_cases = [
        BoundaryTestCase {
            description: "a negative after a minus",
            template: "select -{} as x, 7 as y",
            values: &["-1"],
            dialect: "duckdb",
            expected_sql: "select - -1 as x, 7 as y",
        },
        BoundaryTestCase {
            description: "a trailing comment before code",
            template: "select {} as x, 7 as y",
            values: &["1 -- one"],
            dialect: "duckdb",
            expected_sql: "select 1 -- one\n as x, 7 as y",
        },
        BoundaryTestCase {
            description: "an empty expansion between two halves",
            template: "select a/{}*2",
            values: &[""],
            dialect: "duckdb",
            expected_sql: "select a/ *2",
        },
        BoundaryTestCase {
            description: "a join",
            template: "select {}_count",
            values: &["orders"],
            dialect: "duckdb",
            expected_sql: "select orders_count",
        },
        BoundaryTestCase {
            description: "sites inside a string and a comment stay as they are",
            template: "select '-{}' -- -{}",
            values: &["-1", "-1"],
            dialect: "duckdb",
            expected_sql: "select '--1' -- --1",
        },
        BoundaryTestCase {
            description: "a slash pair is a comment on Snowflake",
            template: "select a/{}",
            values: &["/b"],
            dialect: "snowflake",
            expected_sql: "select a/ /b",
        },
        BoundaryTestCase {
            description: "a slash pair is not a comment on PostgreSQL",
            template: "select a/{}",
            values: &["/b"],
            dialect: "postgres",
            expected_sql: "select a//b",
        },
        BoundaryTestCase {
            description: "an unclosed block comment is left alone",
            template: "select {} as x",
            values: &["1 /* one"],
            dialect: "duckdb",
            expected_sql: "select 1 /* one as x",
        },
    ];
    for test_case in test_cases {
        let syntax: LexicalSyntax = dialect_lexical_syntax(test_case.dialect);
        let (_, bounded) = spliced(test_case.template, test_case.values, &syntax);
        assert_eq!(bounded, test_case.expected_sql, "{}", test_case.description);
    }
}
