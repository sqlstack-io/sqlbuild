//! Statements adding columns a state table gained after it was first created, each only if missing.

use crate::state_tables::models::TableColumn;

/// One `ALTER TABLE … ADD COLUMN IF NOT EXISTS` per column; added columns are always nullable.
pub fn add_columns_sql(qualified_name: &str, columns: &[TableColumn]) -> Vec<String> {
    columns
        .iter()
        .map(|column| {
            format!(
                "ALTER TABLE {qualified_name} ADD COLUMN IF NOT EXISTS {} {}",
                column.name, column.rendered_type
            )
        })
        .collect()
}
