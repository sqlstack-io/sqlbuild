//! Read the recent executions of every relation in one consumption table.

use crate::consumption::constants::{CONSUMPTION_COLUMN_TYPES, RECORDED_AT_COLUMN};

/// Every row of the newest `limit` executions per relation, timing each execution by its earliest row.
pub fn recent_executions_sql(qualified_name: &str, limit: usize) -> String {
    let columns: Vec<&str> = CONSUMPTION_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .collect();
    format!(
        "SELECT {columns} FROM {table} WHERE execution_id IN (SELECT execution_id FROM (\
SELECT execution_id, ROW_NUMBER() OVER (PARTITION BY target_database, target_schema, target_name \
ORDER BY started_at DESC, execution_id DESC) AS execution_rank FROM (SELECT execution_id, \
target_database, target_schema, target_name, MIN({RECORDED_AT_COLUMN}) AS started_at FROM {table} \
GROUP BY execution_id, target_database, target_schema, target_name) AS executions) AS ranked \
WHERE execution_rank <= {limit})",
        columns = columns.join(", "),
        table = qualified_name,
    )
}
