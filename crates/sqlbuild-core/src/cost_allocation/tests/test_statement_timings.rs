use crate::cost_allocation::main::statement_timings_sql::statement_timings_sql;
use crate::cost_allocation::tests::test_types::StatementTimingsSqlTestCase;

#[test]
fn given_query_ids_when_rendering_timings_read_then_ids_are_quoted_and_database_qualified() {
    let test_cases = [
        StatementTimingsSqlTestCase {
            description: "a quote in an id is doubled and the database is quoted upper case",
            database: Some("analytics\"db"),
            query_ids: vec!["01a-1", "it's"],
            expected_sql: "SELECT
  COUNT(*),
  COALESCE(SUM(COMPILATION_TIME), 0),
  COALESCE(SUM(EXECUTION_TIME), 0)
FROM TABLE(\"ANALYTICS\"\"DB\".INFORMATION_SCHEMA.QUERY_HISTORY(
  END_TIME_RANGE_START => DATEADD('second', -720, CURRENT_TIMESTAMP()),
  RESULT_LIMIT => 10000
))
WHERE QUERY_ID IN ('01a-1', 'it''s')",
        },
        StatementTimingsSqlTestCase {
            description: "without a database the session's information schema is read",
            database: None,
            query_ids: vec!["q1"],
            expected_sql: "SELECT
  COUNT(*),
  COALESCE(SUM(COMPILATION_TIME), 0),
  COALESCE(SUM(EXECUTION_TIME), 0)
FROM TABLE(INFORMATION_SCHEMA.QUERY_HISTORY(
  END_TIME_RANGE_START => DATEADD('second', -720, CURRENT_TIMESTAMP()),
  RESULT_LIMIT => 10000
))
WHERE QUERY_ID IN ('q1')",
        },
    ];

    for test_case in test_cases {
        let query_ids: Vec<String> = test_case
            .query_ids
            .iter()
            .map(|query_id| (*query_id).to_owned())
            .collect();
        assert_eq!(
            statement_timings_sql(test_case.database, &query_ids, 720),
            test_case.expected_sql,
            "{}",
            test_case.description
        );
    }
}
