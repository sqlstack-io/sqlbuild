//! `query_history_sql`: the Snowflake `QUERY_HISTORY` read of one end-time window.

use crate::cost_allocation::_helpers::history_function::query_history_function;
use crate::cost_allocation::_helpers::query_text::query_text_expression;
use crate::cost_allocation::models::QueryHistoryWindow;

/// Query-history SQL for queries ending after `start`, and before `end` when it is bounded.
pub fn query_history_sql(window: &QueryHistoryWindow<'_>) -> String {
    let end_range: String = match window.end {
        Some(end) => format!("\n  END_TIME_RANGE_END => TO_TIMESTAMP_LTZ('{end}'),"),
        None => String::new(),
    };
    format!(
        "SELECT
  QUERY_ID,
  QUERY_TAG,
  WAREHOUSE_NAME,
  WAREHOUSE_SIZE,
  WAREHOUSE_TYPE,
  CLUSTER_NUMBER,
  START_TIME,
  END_TIME,
  EXECUTION_TIME,
  BYTES_SCANNED,
  EXECUTION_STATUS,
  QUERY_TYPE,
  {},
  CURRENT_TIMESTAMP()
FROM TABLE({}(
  END_TIME_RANGE_START => TO_TIMESTAMP_LTZ('{}'),{end_range}
  RESULT_LIMIT => {}
))
ORDER BY START_TIME, QUERY_ID",
        query_text_expression(window.text_run_id),
        query_history_function(window.database),
        window.start,
        window.result_limit,
    )
}
