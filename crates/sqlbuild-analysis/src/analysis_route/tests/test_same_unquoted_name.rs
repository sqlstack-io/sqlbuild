use crate::analysis_route::main::same_unquoted_name::same_unquoted_name;
use crate::analysis_route::tests::test_types::SameUnquotedNameTestCase;

#[test]
fn given_two_unquoted_names_when_comparing_then_each_dialect_folds_them_as_it_resolves_names() {
    let test_cases = [
        SameUnquotedNameTestCase {
            description: "Snowflake folds unquoted names to upper case",
            dialect: Some("snowflake"),
            left: "marts",
            right: "MARTS",
            expected_same_name: true,
        },
        SameUnquotedNameTestCase {
            description: "PostgreSQL folds unquoted names to lower case",
            dialect: Some("postgres"),
            left: "Marts",
            right: "marts",
            expected_same_name: true,
        },
        SameUnquotedNameTestCase {
            description: "DuckDB compares names without case",
            dialect: Some("duckdb"),
            left: "marts",
            right: "MARTS",
            expected_same_name: true,
        },
        SameUnquotedNameTestCase {
            description: "ClickHouse names keep their case",
            dialect: Some("clickhouse"),
            left: "marts",
            right: "MARTS",
            expected_same_name: false,
        },
        SameUnquotedNameTestCase {
            description: "different names never match",
            dialect: Some("snowflake"),
            left: "marts",
            right: "staging",
            expected_same_name: false,
        },
        SameUnquotedNameTestCase {
            description: "a dialect without a native module compares exactly",
            dialect: None,
            left: "marts",
            right: "MARTS",
            expected_same_name: false,
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            same_unquoted_name(test_case.dialect, test_case.left, test_case.right),
            test_case.expected_same_name,
            "{}",
            test_case.description
        );
    }
}
