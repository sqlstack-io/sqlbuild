use crate::semantic_validation::_helpers::diagnostics::map_diagnostics;
use crate::semantic_validation::_helpers::truth_tests::{rejects_truth_tests, truth_tests};
use crate::semantic_validation::tests::test_types::{
    LocatedTruthTestDiagnosticTestCase, TruthTestDialectTestCase, TruthTestScanTestCase,
};
use polyglot_sql::{DialectType, ValidationResult};

#[test]
fn given_authored_sql_when_scanning_truth_tests_then_finds_only_keyword_sequences() {
    let test_cases = [
        TruthTestScanTestCase {
            description: "every truth value and negation is found",
            sql: "SELECT a IS TRUE, b IS NOT TRUE, c is false, d Is Not False FROM t",
            expected_tests: &[
                (9, 16, false, true),
                (20, 31, true, true),
                (35, 43, false, false),
                (47, 59, true, false),
            ],
        },
        TruthTestScanTestCase {
            description: "comments between keywords keep the sequence",
            sql: "SELECT 1 FROM t WHERE flag IS /* note */ NOT -- why\n TRUE",
            expected_tests: &[(27, 57, true, true)],
        },
        TruthTestScanTestCase {
            description: "NULL, DISTINCT FROM and other IS predicates are ignored",
            sql: "SELECT 1 FROM t WHERE a IS NULL AND b IS NOT NULL AND c IS DISTINCT FROM TRUE",
            expected_tests: &[],
        },
        TruthTestScanTestCase {
            description: "quoted text, quoted identifiers and comments are ignored",
            sql: "SELECT 'x IS TRUE', \"IS\" AS y, z IS \"TRUE\" FROM t -- a IS TRUE\n/* b IS FALSE */",
            expected_tests: &[],
        },
        TruthTestScanTestCase {
            description: "a word ending in IS is not the IS keyword",
            sql: "SELECT axis TRUE, this NOT FALSE FROM t",
            expected_tests: &[],
        },
    ];
    for test_case in test_cases {
        let found: Vec<(usize, usize, bool, bool)> = truth_tests(test_case.sql)
            .iter()
            .map(|test| (test.start, test.end, test.negated, test.value))
            .collect();
        assert_eq!(found, test_case.expected_tests, "{}", test_case.description);
    }
}

#[test]
fn given_dialect_when_checking_truth_tests_then_only_rejecting_warehouses_report() {
    let test_cases = [
        TruthTestDialectTestCase {
            description: "Snowflake rejects truth-value tests",
            dialect: DialectType::Snowflake,
            expected_rejected: true,
        },
        TruthTestDialectTestCase {
            description: "SQL Server rejects truth-value tests",
            dialect: DialectType::TSQL,
            expected_rejected: true,
        },
        TruthTestDialectTestCase {
            description: "DuckDB accepts truth-value tests",
            dialect: DialectType::DuckDB,
            expected_rejected: false,
        },
        TruthTestDialectTestCase {
            description: "PostgreSQL accepts truth-value tests",
            dialect: DialectType::PostgreSQL,
            expected_rejected: false,
        },
        TruthTestDialectTestCase {
            description: "BigQuery accepts truth-value tests",
            dialect: DialectType::BigQuery,
            expected_rejected: false,
        },
        TruthTestDialectTestCase {
            description: "Databricks accepts truth-value tests",
            dialect: DialectType::Databricks,
            expected_rejected: false,
        },
    ];
    for test_case in test_cases {
        let result = map_diagnostics(
            "SELECT 1 FROM t WHERE flag IS NOT TRUE",
            test_case.dialect,
            ValidationResult {
                valid: true,
                errors: Vec::new(),
            },
        )
        .expect(test_case.description);
        assert_eq!(
            rejects_truth_tests(test_case.dialect),
            test_case.expected_rejected,
            "{}",
            test_case.description
        );
        assert_eq!(
            result.valid, !test_case.expected_rejected,
            "{}",
            test_case.description
        );
        assert_eq!(
            result
                .errors
                .iter()
                .map(|error| error.code.as_str())
                .collect::<Vec<_>>(),
            if test_case.expected_rejected {
                vec!["B103"]
            } else {
                Vec::new()
            },
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_rejected_truth_test_when_mapping_diagnostics_then_returns_located_b103()
-> Result<(), String> {
    let test_cases = [
        LocatedTruthTestDiagnosticTestCase {
            description: "Snowflake points at IS after a multibyte literal",
            dialect: DialectType::Snowflake,
            sql: "SELECT 'é' AS label\nFROM orders\nWHERE is_test_order IS NOT TRUE",
            expected_message: "'IS NOT TRUE' is not supported for dialect Snowflake; \
                               write NOT COALESCE(<expr>, FALSE) or <expr> IS DISTINCT FROM TRUE",
            expected_location: (Some(3), Some(21)),
            expected_span: (Some(52), Some(63)),
        },
        LocatedTruthTestDiagnosticTestCase {
            description: "Snowflake IS FALSE suggests the TRUE default",
            dialect: DialectType::Snowflake,
            sql: "SELECT 1 FROM orders WHERE is_test_order is false",
            expected_message: "'IS FALSE' is not supported for dialect Snowflake; \
                               write NOT COALESCE(<expr>, TRUE)",
            expected_location: (Some(1), Some(42)),
            expected_span: (Some(41), Some(49)),
        },
        LocatedTruthTestDiagnosticTestCase {
            description: "SQL Server suggests a BIT comparison",
            dialect: DialectType::TSQL,
            sql: "SELECT 1 FROM orders WHERE is_test_order IS TRUE",
            expected_message: "'IS TRUE' is not supported for dialect TSQL; \
                               write COALESCE(<expr>, 0) = 1",
            expected_location: (Some(1), Some(42)),
            expected_span: (Some(41), Some(48)),
        },
    ];
    for test_case in &test_cases {
        let result = map_diagnostics(
            test_case.sql,
            test_case.dialect,
            ValidationResult {
                valid: true,
                errors: Vec::new(),
            },
        )?;
        let [error] = result.errors.as_slice() else {
            return Err(format!("{}: {:?}", test_case.description, result.errors));
        };

        assert!(!result.valid, "{}", test_case.description);
        assert_eq!(error.code, "B103", "{}", test_case.description);
        assert_eq!(
            error.message, test_case.expected_message,
            "{}",
            test_case.description
        );
        assert_eq!(
            (error.line, error.column),
            test_case.expected_location,
            "{}",
            test_case.description
        );
        assert_eq!(
            (error.start, error.end),
            test_case.expected_span,
            "{}",
            test_case.description
        );
    }
    Ok(())
}
