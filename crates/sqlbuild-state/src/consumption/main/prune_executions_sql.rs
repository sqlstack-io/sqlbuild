//! Prune old executions from one consumption table.

use crate::consumption::constants::RECORDED_AT_COLUMN;

/// Delete every row of executions ranked beyond the newest `retain` per relation, whole executions at a time.
pub fn prune_executions_sql(qualified_name: &str, retain: i64) -> String {
    format!(
        "DELETE FROM {table} WHERE execution_id IN (SELECT execution_id FROM (SELECT execution_id, \
ROW_NUMBER() OVER (PARTITION BY target_database, target_schema, target_name ORDER BY started_at DESC, \
execution_id DESC) AS execution_rank FROM (SELECT execution_id, target_database, target_schema, \
target_name, MIN({RECORDED_AT_COLUMN}) AS started_at FROM {table} GROUP BY execution_id, \
target_database, target_schema, target_name) AS executions) AS ranked WHERE execution_rank > {retain})",
        table = qualified_name,
    )
}
