use sqlbuild_sqltext::sql_scan::main::dialect_lexical_syntax::dialect_lexical_syntax;

use crate::relation_markers::main::markers_as_names::markers_as_names;
use crate::relation_markers::tests::test_types::MarkersAsNamesTestCase;

#[test]
fn given_marker_calls_when_reading_them_as_names_then_code_calls_become_names_and_positions_hold() {
    let test_cases = [
        MarkersAsNamesTestCase {
            description: "each call becomes its indexed name, padded to the call's width",
            dialect: "snowflake",
            sql: r#"SELECT * FROM __ref("orders") o JOIN __source("raw") r ON TRUE"#,
            expected_sql: format!(
                "SELECT * FROM __sqb_0{} o JOIN __sqb_1{} r ON TRUE",
                " ".repeat(8),
                " ".repeat(8)
            ),
            expected_calls: &[("__ref", "orders"), ("__source", "raw")],
        },
        MarkersAsNamesTestCase {
            description: "calls in quoted text and comments stay; an upper-case call folds",
            dialect: "snowflake",
            sql: "SELECT '__ref(\"a\")' AS q -- __ref(\"b\")\nFROM __REF(\"Orders\") // __ref(\"c\")",
            expected_sql: format!(
                "SELECT '__ref(\"a\")' AS q -- __ref(\"b\")\nFROM __sqb_0{} // __ref(\"c\")",
                " ".repeat(8)
            ),
            expected_calls: &[("__ref", "Orders")],
        },
        MarkersAsNamesTestCase {
            description: "a call over three lines keeps its line breaks and its last line's width",
            dialect: "duckdb",
            sql: "FROM __ref(\n  \"orders\"\n) AS o",
            expected_sql: "FROM __sqb_0\n\n  AS o".to_owned(),
            expected_calls: &[("__ref", "orders")],
        },
        MarkersAsNamesTestCase {
            description: "two dbt names join; a non-ASCII name pads by characters, not bytes",
            dialect: "postgres",
            sql: r#"FROM __dbt_ref("shop", "customers") d JOIN __ref("café") c ON TRUE"#,
            expected_sql: format!(
                "FROM __sqb_0{} d JOIN __sqb_1{} c ON TRUE",
                " ".repeat(23),
                " ".repeat(6)
            ),
            expected_calls: &[("__dbt_ref", "shop__customers"), ("__ref", "café")],
        },
        MarkersAsNamesTestCase {
            description: "calls with a second name, or a non-literal argument, stay calls",
            dialect: "duckdb",
            sql: r#"FROM __ref("a", "b") JOIN __table_fn("orders", 2) ON TRUE"#,
            expected_sql: r#"FROM __ref("a", "b") JOIN __table_fn("orders", 2) ON TRUE"#.to_owned(),
            expected_calls: &[],
        },
    ];
    for test_case in test_cases {
        let (named, calls) =
            markers_as_names(test_case.sql, &dialect_lexical_syntax(test_case.dialect))
                .expect(test_case.description);
        let expected_calls: Vec<(String, String)> = test_case
            .expected_calls
            .iter()
            .map(|(function, name)| ((*function).to_owned(), (*name).to_owned()))
            .collect();
        assert_eq!(
            (named, calls),
            (test_case.expected_sql, expected_calls),
            "{}",
            test_case.description
        );
    }
}
