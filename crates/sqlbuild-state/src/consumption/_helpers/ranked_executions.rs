//! Select a consumption table's executions by their rank within their relation.

use crate::consumption::constants::RECORDED_AT_COLUMN;

/// `execution_id IN (...)` for executions whose newest-first rank per relation meets `rank_condition`.
pub(crate) fn ranked_executions(table: &str, rank_condition: &str) -> String {
    format!(
        "execution_id IN (SELECT execution_id FROM (SELECT execution_id, ROW_NUMBER() OVER \
(PARTITION BY target_database, target_schema, target_name ORDER BY started_at DESC, \
execution_id DESC) AS execution_rank FROM (SELECT execution_id, target_database, target_schema, \
target_name, MIN({RECORDED_AT_COLUMN}) AS started_at FROM {table} GROUP BY execution_id, \
target_database, target_schema, target_name) AS executions) AS ranked WHERE execution_rank \
{rank_condition})"
    )
}
