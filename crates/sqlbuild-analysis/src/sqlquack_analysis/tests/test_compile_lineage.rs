use crate::sqlquack_analysis::tests::helpers::{compile_lines, compile_types};
use crate::sqlquack_analysis::tests::test_types::{CompileLineageTestCase, CompileTypesTestCase};

#[test]
fn given_a_compiled_model_when_tracing_compile_lineage_then_columns_name_their_resources() {
    let test_cases = [
        CompileLineageTestCase {
            description: "a direct and a computed column read the referenced model",
            dialect: "snowflake",
            sql: "SELECT o.id, o.amount * 2 AS doubled FROM ORDERS_REL o",
            expected_lines: &[
                "id direct high [model:orders.ID]",
                "doubled expression high [model:orders.AMOUNT]",
            ],
        },
        CompileLineageTestCase {
            description: "a quoted column keeps its exact spelling",
            dialect: "snowflake",
            sql: "SELECT e.\"listingId\" FROM EXPORTS_REL e",
            expected_lines: &["listingId direct high [seed:listing_export.listingId]"],
        },
        CompileLineageTestCase {
            description: "a union with a literal branch is an expression, not direct",
            dialect: "snowflake",
            sql: "SELECT id FROM ORDERS_REL UNION ALL SELECT 0",
            expected_lines: &["id expression high [model:orders.ID]"],
        },
        CompileLineageTestCase {
            description: "a column of no relation is unknown",
            dialect: "duckdb",
            sql: "SELECT missing FROM ORDERS_REL",
            expected_lines: &["missing constant unknown []"],
        },
        CompileLineageTestCase {
            description: "SQL SQLQuack cannot parse has no lineage",
            dialect: "snowflake",
            sql: "SELECT FROM WHERE (",
            expected_lines: &["unread"],
        },
        CompileLineageTestCase {
            description: "another dialect gets no answer from SQLQuack",
            dialect: "bigquery",
            sql: "SELECT 1 AS one",
            expected_lines: &["no answer"],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            compile_lines(test_case.dialect, test_case.sql),
            test_case.expected_lines,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_compiled_model_when_typing_outputs_then_the_catalog_and_its_functions_decide() {
    let test_cases = [
        CompileTypesTestCase {
            description: "stored columns keep their catalog type and nullability; unsized text is TEXT",
            sql: "SELECT o.id, o.amount FROM ORDERS_REL o",
            expected_lines: &["id NUMBER(38,0) non_null", "amount TEXT nullable"],
        },
        CompileTypesTestCase {
            description: "a relation known only in part types the columns it lists",
            sql: "SELECT e.kind, e.other FROM EVENTS_REL e",
            expected_lines: &["kind TEXT unknown", "other - unknown"],
        },
        CompileTypesTestCase {
            description: "a lower-case catalog name the SQL quotes is that quoted name",
            sql: "SELECT e.\"user\" AS username FROM EVENTS_REL e",
            expected_lines: &["username VARCHAR(16777216) unknown"],
        },
        CompileTypesTestCase {
            description: "the adapter's function return types and nullability rules apply",
            sql: "SELECT UPPER(o.amount) AS shouted, LISTAGG(o.amount, ',') AS listed \
                  FROM ORDERS_REL o",
            expected_lines: &["shouted TEXT nullable", "listed TEXT unknown"],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            compile_types(test_case.sql),
            test_case.expected_lines,
            "{}",
            test_case.description
        );
    }
}
