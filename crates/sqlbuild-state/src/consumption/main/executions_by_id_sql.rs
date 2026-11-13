//! Read named executions from one consumption table.

use crate::consumption::constants::CONSUMPTION_COLUMN_TYPES;

/// Every row of the executions in `execution_ids`, which must be non-empty.
pub fn executions_by_id_sql(qualified_name: &str, execution_ids: &[&str]) -> String {
    let columns: Vec<&str> = CONSUMPTION_COLUMN_TYPES
        .iter()
        .map(|(column, _)| *column)
        .collect();
    let ids: Vec<String> = execution_ids
        .iter()
        .map(|id| format!("'{}'", id.replace('\'', "''")))
        .collect();
    format!(
        "SELECT {columns} FROM {qualified_name} WHERE execution_id IN ({ids})",
        columns = columns.join(", "),
        ids = ids.join(", "),
    )
}
