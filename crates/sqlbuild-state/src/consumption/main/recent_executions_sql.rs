//! Read the recent executions of every relation in one consumption table.

use crate::consumption::_helpers::ranked_executions::ranked_executions;
use crate::consumption::constants::CONSUMPTION_COLUMN_TYPES;

/// Every row of the newest `limit` executions per relation, timing each execution by its earliest row.
pub fn recent_executions_sql(qualified_name: &str, limit: usize) -> String {
    let columns: Vec<&str> = CONSUMPTION_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .collect();
    format!(
        "SELECT {columns} FROM {qualified_name} WHERE {executions}",
        columns = columns.join(", "),
        executions = ranked_executions(qualified_name, &format!("<= {limit}")),
    )
}
