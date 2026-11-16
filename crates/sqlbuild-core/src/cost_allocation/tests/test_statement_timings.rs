use crate::cost_allocation::main::statement_timings_sql::statement_timings_sql;

#[test]
fn given_query_ids_when_rendering_timings_read_then_ids_are_quoted_and_database_qualified() {
    let query_ids: Vec<String> = vec!["01a-1".to_owned(), "it's".to_owned()];

    let sql: String = statement_timings_sql(Some("analytics\"db"), &query_ids, 720);

    assert_eq!(
        sql,
        "SELECT
  COUNT(*),
  COALESCE(SUM(COMPILATION_TIME), 0),
  COALESCE(SUM(EXECUTION_TIME), 0)
FROM TABLE(\"ANALYTICS\"\"DB\".INFORMATION_SCHEMA.QUERY_HISTORY(
  END_TIME_RANGE_START => DATEADD('second', -720, CURRENT_TIMESTAMP()),
  RESULT_LIMIT => 10000
))
WHERE QUERY_ID IN ('01a-1', 'it''s')"
    );
}
