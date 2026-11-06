use crate::sql_macros::main::sql_macro_calls::sql_macro_calls;
use crate::sql_macros::tests::helpers::{backslash_rules, escape_prefix_rules, raw_prefix_rules};
use crate::sql_macros::tests::test_types::LexicalRulesCallsTestCase;

#[test]
fn given_adapter_lexical_rules_when_finding_sql_macro_calls_then_strings_end_where_the_adapter_ends_them()
 {
    let test_cases = [
        LexicalRulesCallsTestCase {
            description: "an E-string's escaped quote on DuckDB and Postgres",
            syntax: escape_prefix_rules(),
            sql: "SELECT E'it\\'s' AS a, @sql.order_total(1) AS b",
            expected_calls: &["order_total"],
        },
        LexicalRulesCallsTestCase {
            description: "a plain string ending in a backslash on DuckDB and Postgres",
            syntax: escape_prefix_rules(),
            sql: "SELECT 'C:\\' AS path, @sql.order_total(1) AS b",
            expected_calls: &["order_total"],
        },
        LexicalRulesCallsTestCase {
            description: "a backslash-escaped quote on Snowflake",
            syntax: backslash_rules(),
            sql: "SELECT 'it\\'s' AS a, @sql.order_total(1) AS b",
            expected_calls: &["order_total"],
        },
        LexicalRulesCallsTestCase {
            description: "a quote inside a Snowflake // comment",
            syntax: backslash_rules(),
            sql: "SELECT @sql.order_total(1) AS b // the customer's total\nFROM orders",
            expected_calls: &["order_total"],
        },
        LexicalRulesCallsTestCase {
            description: "a raw string ending in a backslash under raw-string prefix rules",
            syntax: raw_prefix_rules(),
            sql: "SELECT r'C:\\' AS path, @sql.order_total(1) AS b",
            expected_calls: &["order_total"],
        },
    ];

    for test_case in test_cases {
        let calls = sql_macro_calls(test_case.sql, Some(&test_case.syntax))
            .unwrap_or_else(|error| panic!("{}: {error:?}", test_case.description));
        let names: Vec<&str> = calls.iter().map(|call| call.name.as_str()).collect();
        assert_eq!(names, test_case.expected_calls, "{}", test_case.description);
    }
}
