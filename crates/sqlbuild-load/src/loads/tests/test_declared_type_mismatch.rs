use sqlload::column_types::models::Warehouse;

use crate::loads::main::declared_type_mismatch::declared_type_mismatch;
use crate::loads::tests::test_types::DeclaredTypeTestCase;

#[test]
fn given_a_declared_type_when_comparing_by_name_then_reports_the_type_sqlload_creates() {
    let test_cases = [
        DeclaredTypeTestCase {
            description: "DuckDB stores an integer as BIGINT",
            warehouse: Warehouse::DuckDb,
            declared: "INTEGER",
            expected_created: Some("BIGINT"),
        },
        DeclaredTypeTestCase {
            description: "DuckDB TIMESTAMP is a timestamp without a time zone",
            warehouse: Warehouse::DuckDb,
            declared: "TIMESTAMP",
            expected_created: None,
        },
        DeclaredTypeTestCase {
            description: "Postgres timestamp is a timestamp without a time zone",
            warehouse: Warehouse::Postgres,
            declared: "timestamp",
            expected_created: None,
        },
        DeclaredTypeTestCase {
            description: "a type SQLLoad refuses is left to SQLLoad's own error",
            warehouse: Warehouse::Snowflake,
            declared: "TIMESTAMP",
            expected_created: None,
        },
    ];
    for test_case in test_cases {
        let created = declared_type_mismatch(
            &[("id".to_owned(), test_case.declared.to_owned())],
            test_case.warehouse,
            |declared, created| declared.eq_ignore_ascii_case(created),
        )
        .map(|mismatch| mismatch.created);

        assert_eq!(
            created.as_deref(),
            test_case.expected_created,
            "{}",
            test_case.description
        );
    }
}
