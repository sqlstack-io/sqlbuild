//! `statement_timings_sql`: compile and execution time of known Snowflake queries.

use crate::cost_allocation::_helpers::history_function::query_history_function;
use crate::cost_allocation::constants::QUERY_HISTORY_RESULT_LIMIT;

/// One query-history read summing compile and execution milliseconds of `query_ids` that ended in the last `lookback_seconds`.
#[must_use]
pub fn statement_timings_sql(
    database: Option<&str>,
    query_ids: &[String],
    lookback_seconds: u64,
) -> String {
    let identifiers: String = query_ids
        .iter()
        .map(|query_id| format!("'{}'", query_id.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "SELECT
  COUNT(*),
  COALESCE(SUM(COMPILATION_TIME), 0),
  COALESCE(SUM(EXECUTION_TIME), 0)
FROM TABLE({}(
  END_TIME_RANGE_START => DATEADD('second', -{lookback_seconds}, CURRENT_TIMESTAMP()),
  RESULT_LIMIT => {QUERY_HISTORY_RESULT_LIMIT}
))
WHERE QUERY_ID IN ({identifiers})",
        query_history_function(database),
    )
}
