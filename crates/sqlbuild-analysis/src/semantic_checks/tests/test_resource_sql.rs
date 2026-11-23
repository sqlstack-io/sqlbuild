use std::sync::Arc;

use crate::semantic_checks::_helpers::resource_sql::diagnostics::missing_column_diagnostic;
use crate::semantic_checks::_helpers::resource_sql::fixtures::prepared_test_sql;
use crate::semantic_checks::_helpers::resource_sql::resources::missing_column_audits;
use crate::semantic_checks::models::AuditSqlInput;
use crate::semantic_checks::tests::helpers::missing_column_request;
use crate::semantic_checks::tests::test_types::{
    MissingColumnAuditTestCase, PreparedTestSqlTestCase,
};

#[test]
fn given_sql_test_bodies_when_preparing_then_empty_fixtures_open_as_python_rewrites_them() {
    let test_cases = [
        PreparedTestSqlTestCase {
            description: "an empty fixture CTE reads its own relation",
            sql_body: "WITH __ref__customers AS (SELECT * FROM __empty_fixture()) SELECT 1",
            expected_sql: "WITH __ref__customers AS (SELECT * FROM customers) SELECT 1",
            expected_relations: &["customers"],
        },
        PreparedTestSqlTestCase {
            description: "other empty fixtures open and reference calls are relations",
            sql_body: "SELECT * FROM __EMPTY_FIXTURE ( ) JOIN __ref('orders') AS o ON TRUE",
            expected_sql: "SELECT * FROM __sqlbuild_empty_fixture JOIN __ref('orders') AS o ON TRUE",
            expected_relations: &["orders"],
        },
        PreparedTestSqlTestCase {
            description: "a fixture name inside a longer word is not a CTE",
            sql_body: "WITH x__ref__a AS (SELECT * FROM __empty_fixture()) SELECT 1",
            expected_sql: "WITH x__ref__a AS (SELECT * FROM __sqlbuild_empty_fixture) SELECT 1",
            expected_relations: &[],
        },
    ];

    for test_case in test_cases {
        let (sql, relations) = prepared_test_sql(test_case.sql_body);

        assert_eq!(
            (
                sql.as_str(),
                relations.iter().map(String::as_str).collect::<Vec<_>>()
            ),
            (
                test_case.expected_sql,
                test_case.expected_relations.to_vec()
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_built_in_audit_on_missing_column_when_checking_then_reports_once_at_its_declaration() {
    let test_cases = [
        MissingColumnAuditTestCase {
            description: "not_null on a missing column points at its MODEL declaration",
            column: "shipped_at",
            definition: "not_null",
            built_in: true,
            expected_finding: Some((
                "audit 'not_null' on model 'orders' column 'shipped_at': \
                 Unknown column 'shipped_at' in table 'orders'",
                1,
                96,
                104,
            )),
        },
        MissingColumnAuditTestCase {
            description: "unique after the column's first audit points at its own name",
            column: "shipped_at",
            definition: "unique",
            built_in: true,
            expected_finding: Some((
                "audit 'unique' on model 'orders' column 'shipped_at': \
                 Unknown column 'shipped_at' in table 'orders'",
                1,
                106,
                112,
            )),
        },
        MissingColumnAuditTestCase {
            description: "a column the model outputs is bound as resource SQL",
            column: "order_id",
            definition: "unique",
            built_in: true,
            expected_finding: None,
        },
        MissingColumnAuditTestCase {
            description: "a project audit overriding a built-in keeps its own file",
            column: "shipped_at",
            definition: "not_null",
            built_in: false,
            expected_finding: None,
        },
    ];

    for test_case in test_cases {
        let request = missing_column_request(AuditSqlInput {
            name: test_case.definition.to_owned(),
            definition_name: test_case.definition.to_owned(),
            attached_target: Some(("model".to_owned(), "orders".to_owned())),
            attached_column: Some(test_case.column.to_owned()),
            built_in: test_case.built_in,
            path: format!("sql/audits/generic/{}.sql", test_case.definition),
            contents: Arc::from(""),
            bodies: Vec::new(),
            opts_out: false,
            relations: vec!["orders".to_owned()],
        });

        let found: Option<(String, i64, i64, i64)> =
            missing_column_audits(&request).first().map(|missing| {
                let diagnostic = missing_column_diagnostic(missing).expect(test_case.description);
                assert_eq!(diagnostic.code, "B002", "{}", test_case.description);
                assert_eq!(
                    diagnostic.path, "sql/models/orders.sql",
                    "{}",
                    test_case.description
                );
                (
                    diagnostic.message,
                    diagnostic.span.line,
                    diagnostic.span.column,
                    diagnostic.span.end_column.unwrap_or_default(),
                )
            });
        assert_eq!(
            found.as_ref().map(|(message, line, column, end)| (
                message.as_str(),
                *line,
                *column,
                *end
            )),
            test_case.expected_finding,
            "{}",
            test_case.description
        );
    }
}
