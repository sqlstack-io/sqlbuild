use crate::sqlquack_analysis::tests::helpers::compile_lines;
use crate::sqlquack_analysis::tests::test_types::CompileLineageTestCase;

#[test]
fn given_a_compiled_model_when_tracing_compile_lineage_then_columns_name_their_resources() {
    let test_cases = [
        CompileLineageTestCase {
            description: "a direct and a computed column read the referenced model",
            dialect: "snowflake",
            sql: "SELECT o.id, o.amount * 2 AS doubled FROM ORDERS_REL o",
            expected_lines: Some(&[
                "id direct high [model:orders.ID]",
                "doubled expression high [model:orders.AMOUNT]",
            ]),
        },
        CompileLineageTestCase {
            description: "a quoted column keeps its exact spelling",
            dialect: "snowflake",
            sql: "SELECT e.\"listingId\" FROM EXPORTS_REL e",
            expected_lines: Some(&["listingId direct high [seed:listing_export.listingId]"]),
        },
        CompileLineageTestCase {
            description: "a union with a literal branch is an expression, not direct",
            dialect: "snowflake",
            sql: "SELECT id FROM ORDERS_REL UNION ALL SELECT 0",
            expected_lines: Some(&["id expression high [model:orders.ID]"]),
        },
        CompileLineageTestCase {
            description: "a column of no relation is unknown",
            dialect: "duckdb",
            sql: "SELECT missing FROM ORDERS_REL",
            expected_lines: Some(&["missing constant unknown []"]),
        },
        CompileLineageTestCase {
            description: "SQL SQLQuack cannot parse has no lineage",
            dialect: "snowflake",
            sql: "SELECT FROM WHERE (",
            expected_lines: None,
        },
    ];
    for test_case in test_cases {
        let expected: Option<Vec<String>> = test_case
            .expected_lines
            .map(|lines| lines.iter().map(|line| (*line).to_owned()).collect());
        assert_eq!(
            compile_lines(test_case.dialect, test_case.sql),
            Some(expected),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_another_dialect_when_tracing_compile_lineage_then_sqlquack_does_not_answer() {
    assert_eq!(compile_lines("bigquery", "SELECT 1 AS one"), None);
}
