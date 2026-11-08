use crate::explicit_references::_helpers::relations::{matched, project_relations};
use crate::explicit_references::main::check_explicit_references::check_explicit_references;
use crate::explicit_references::tests::helpers::{clickhouse_request, relation, request};
use crate::explicit_references::tests::test_types::{HardCodedSqlTestCase, MatchTestCase};

#[test]
fn given_written_relations_when_matching_project_relations_then_matches_python() {
    let test_cases = [
        MatchTestCase {
            description: "an unqualified name matches case-insensitively",
            written: ("CUSTOMERS", None, None),
            expected_match: Some(("model", "customers", None)),
        },
        MatchTestCase {
            description: "a conflicting schema does not match",
            written: ("customers", Some("other"), None),
            expected_match: None,
        },
        MatchTestCase {
            description: "an empty source table falls back to the source name",
            written: ("raw_orders", Some("LANDING"), None),
            expected_match: Some(("source", "raw_orders", None)),
        },
        MatchTestCase {
            description: "names match by casefold where lowercase differs",
            written: ("STRASSE", Some("LANDING"), None),
            expected_match: Some(("source", "street_addresses", None)),
        },
        MatchTestCase {
            description: "an expression source is no relation",
            written: ("computed", None, None),
            expected_match: None,
        },
        MatchTestCase {
            description: "a stripped, unquoted old name inherits the destination namespace",
            written: ("legacy_customers", Some("main"), Some("memory")),
            expected_match: Some(("model", "customers", Some("customers"))),
        },
        MatchTestCase {
            description: "a schema-qualified old name keeps its schema",
            written: ("old_orders", Some("archive"), None),
            expected_match: Some(("model", "orders", Some("orders"))),
        },
        MatchTestCase {
            description: "an old name equal to another model is that model, not an old name",
            written: ("customers", None, None),
            expected_match: Some(("model", "customers", None)),
        },
    ];
    let index = project_relations(&request());

    for test_case in test_cases {
        let (name, schema, database) = test_case.written;
        let found = matched(&index, &relation(name, schema, database)).map(|found| {
            (
                found.target.0.as_str(),
                found.target.1.as_str(),
                found.compatibility_for.as_deref(),
            )
        });
        assert_eq!(found, test_case.expected_match, "{}", test_case.description);
    }
}

#[test]
fn given_clickhouse_python_sql_when_checking_hard_coded_names_then_each_statement_reads_as_polyglot_read_it()
 {
    let test_cases = [
        HardCodedSqlTestCase {
            description: "TRUNCATE names a model",
            statements: &["TRUNCATE TABLE main.orders"],
            expected_p008_count: 1,
        },
        HardCodedSqlTestCase {
            description: "ALTER ... DELETE names a model",
            statements: &["ALTER TABLE main.orders DELETE WHERE id = 1"],
            expected_p008_count: 1,
        },
        HardCodedSqlTestCase {
            description: "a SELECT ... INTO OUTFILE names a model",
            statements: &["SELECT * FROM main.customers INTO OUTFILE 'x.csv'"],
            expected_p008_count: 1,
        },
        HardCodedSqlTestCase {
            description: "a temporary table hides the model of the same name",
            statements: &[
                "CREATE TEMPORARY TABLE customers AS SELECT 1 AS id",
                "SELECT * FROM customers",
            ],
            expected_p008_count: 0,
        },
        HardCodedSqlTestCase {
            description: "OPTIMIZE names no relation, as Polyglot reads it",
            statements: &["OPTIMIZE TABLE main.orders FINAL"],
            expected_p008_count: 0,
        },
        HardCodedSqlTestCase {
            description: "an ALTER with an UPDATE names no relation, as Polyglot reads it",
            statements: &["ALTER TABLE main.orders UPDATE a = 1 WHERE 1"],
            expected_p008_count: 0,
        },
    ];
    for test_case in test_cases {
        let report =
            check_explicit_references(&clickhouse_request(test_case.statements)).expect("checked");
        let found: usize = report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "P008")
            .count();
        assert_eq!(
            found, test_case.expected_p008_count,
            "{}",
            test_case.description
        );
    }
}
