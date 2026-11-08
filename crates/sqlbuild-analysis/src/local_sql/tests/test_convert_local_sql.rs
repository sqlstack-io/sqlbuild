use crate::local_sql::main::convert_local_sql::convert_local_sql;
use crate::local_sql::models::LocalSql;
use crate::local_sql::tests::test_types::LocalSqlTestCase;

const SCHEMA: &str = r#"{"__SQB_LOCAL__SOURCE__ORDERS": [["ID", "NUMBER(38,0)"], ["TOTAL", "NUMBER(10,2)"], ["PLACED_AT", "TIMESTAMP_NTZ(9)"], ["SHIPPED_AT", "TIMESTAMP_LTZ(9)"]]}"#;

#[test]
fn given_captured_snowflake_sql_when_converting_then_matches_the_command_line_default() {
    let test_cases = [
        LocalSqlTestCase {
            description: "unquoted local names resolve to the upper-case schema relation",
            sql: "select IFNULL(total, 0) as total from __sqb_local__source__orders",
            expected_columns: Some(vec![("TOTAL", "NUMBER(10,2)")]),
            expected_fragments: vec!["COALESCE(", "FROM \"__SQB_LOCAL__SOURCE__ORDERS\""],
        },
        LocalSqlTestCase {
            description: "a quoted date part and mixed timestamps convert under the built-in allowances",
            sql: "select id, timestampdiff('day', placed_at, shipped_at) as days from __sqb_local__source__orders",
            expected_columns: Some(vec![("ID", "NUMBER(38,0)"), ("DAYS", "NUMBER(9,0)")]),
            expected_fragments: vec!["date_diff('day'"],
        },
        LocalSqlTestCase {
            description: "a statement needing the unconfirmed UTC session assumption is refused",
            sql: "select convert_timezone('UTC', current_timestamp()) as now",
            expected_columns: None,
            expected_fragments: vec![
                "error[SQ0008]",
                "session_timezone_utc",
                "--> model 'orders':1:8",
            ],
        },
        LocalSqlTestCase {
            description: "a relation the schema does not declare is refused",
            sql: "select id from missing_orders",
            expected_columns: None,
            expected_fragments: vec!["error[SQ0004]", "`MISSING_ORDERS` has no known columns"],
        },
        LocalSqlTestCase {
            description: "more than one statement is refused",
            sql: "select 1; select 2",
            expected_columns: None,
            expected_fragments: vec!["expected one statement, got 2"],
        },
    ];

    for test_case in test_cases {
        let converted = convert_local_sql(test_case.sql, SCHEMA, "model 'orders'")
            .unwrap_or_else(|error| panic!("{}: {error}", test_case.description));

        let (columns, text) = match converted {
            LocalSql::Converted { sql, columns } => (Some(columns), sql),
            LocalSql::Refused(reason) => (None, reason),
        };
        let expected_columns: Option<Vec<(String, String)>> =
            test_case.expected_columns.map(|columns| {
                columns
                    .into_iter()
                    .map(|(name, data_type)| (name.to_owned(), data_type.to_owned()))
                    .collect()
            });
        assert_eq!(columns, expected_columns, "{}", test_case.description);
        for fragment in test_case.expected_fragments {
            assert!(
                text.contains(fragment),
                "{}: {fragment:?} not in {text}",
                test_case.description
            );
        }
    }
}

#[test]
fn given_a_schema_that_is_not_json_when_converting_then_the_request_fails() {
    let failure = convert_local_sql("select 1", "not json", "model 'orders'");

    assert!(failure.is_err());
}
