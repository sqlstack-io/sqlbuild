use crate::sqlquack_analysis::tests::helpers::{compile_findings, compile_lines, compile_types};
use crate::sqlquack_analysis::tests::test_types::{
    CompileFindingsTestCase, CompileLineageTestCase, CompileTypesTestCase,
};

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
            description: "a column its one relation lacks is read from it, untraced",
            dialect: "duckdb",
            sql: "SELECT missing FROM ORDERS_REL",
            expected_lines: &["missing direct medium [model:orders.missing]"],
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

#[test]
fn given_a_compiled_model_when_resolving_its_names_then_unresolved_references_are_found() {
    let test_cases = [
        CompileFindingsTestCase {
            description: "a typo in a model column is an unknown column of its relation",
            dialect: "snowflake",
            sql: "SELECT o.amonut FROM ORDERS_REL o",
            expected_findings: &["E201 Unknown column 'amonut' in table 'orders_rel' @9"],
        },
        CompileFindingsTestCase {
            description: "a quoted catalog column read with its quotes resolves",
            dialect: "snowflake",
            sql: "SELECT \"listingId\" FROM EXPORTS_REL",
            expected_findings: &[],
        },
        CompileFindingsTestCase {
            description: "an unqualified column both relations offer is ambiguous on DuckDB",
            dialect: "duckdb",
            sql: "SELECT id FROM ORDERS_REL a JOIN ORDERS_REL b ON TRUE",
            expected_findings: &[
                "E221 Ambiguous unqualified column 'id' found in 2 referenced tables @7",
            ],
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            compile_findings(test_case.dialect, test_case.sql),
            test_case.expected_findings,
            "{}",
            test_case.description
        );
    }
}
