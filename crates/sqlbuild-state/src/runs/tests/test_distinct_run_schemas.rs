use crate::runs::main::distinct_run_schemas::distinct_run_schemas;
use crate::runs::models::RunSchema;
use crate::runs::tests::helpers::{folded, run_schemas};
use crate::runs::tests::test_types::DistinctRunSchemasTestCase;

#[test]
fn given_spellings_of_schemas_when_deduplicating_then_each_physical_schema_is_written_once() {
    let test_cases = [
        DistinctRunSchemasTestCase {
            description: "case spellings of one unquoted name are one schema",
            schemas: &[(Some("ANALYTICS"), "MARTS"), (Some("ANALYTICS"), "marts")],
            default_database: Some("ANALYTICS"),
            expected_schemas: &[(Some("ANALYTICS"), "MARTS")],
        },
        DistinctRunSchemasTestCase {
            description: "an unnamed database is the target's",
            schemas: &[(None, "MARTS"), (Some("analytics"), "MARTS")],
            default_database: Some("ANALYTICS"),
            expected_schemas: &[(Some("ANALYTICS"), "MARTS")],
        },
        DistinctRunSchemasTestCase {
            description: "the same schema in two databases is two schemas",
            schemas: &[(Some("ANALYTICS"), "MARTS"), (Some("RAW"), "MARTS")],
            default_database: Some("ANALYTICS"),
            expected_schemas: &[(Some("ANALYTICS"), "MARTS"), (Some("RAW"), "MARTS")],
        },
        DistinctRunSchemasTestCase {
            description: "without a target database an unnamed database stays apart",
            schemas: &[(None, "marts"), (Some("demo"), "marts")],
            default_database: None,
            expected_schemas: &[(None, "marts"), (Some("demo"), "marts")],
        },
    ];

    for test_case in test_cases {
        let distinct: Vec<RunSchema> = distinct_run_schemas(
            run_schemas(test_case.schemas),
            test_case.default_database,
            &folded,
        );

        assert_eq!(
            distinct,
            run_schemas(test_case.expected_schemas),
            "{}",
            test_case.description
        );
    }
}
