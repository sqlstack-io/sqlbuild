//! One insert contract for standalone SQLite Recall and transactional control stores.

use std::collections::HashSet;

use crate::records::models::{RecallRow, RecallValue};
use crate::schema::_helpers::rendering::sqlite_identifier;
use crate::schema::main::table_columns::table_columns;
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::ColumnType;
use crate::store::_helpers::relational::placeholder;
use crate::store::models::RecallStatement;

/// Ignore fields absent from an older additive schema; ingestion time is assigned by the store.
#[must_use]
pub fn relational_insert<'a>(
    (row, columns): (&'a RecallRow, &HashSet<String>),
    ingested_at: &'a RecallValue,
    postgres: bool,
) -> RecallStatement<'a> {
    let metadata = table_columns(row.table);
    let mut names = Vec::new();
    let mut values = Vec::new();
    for (name, value) in &row.values {
        if let Some(column) = metadata.iter().find(|column| column.name == *name)
            && columns.contains(*name)
            && *name != "ingested_at"
        {
            names.push(sqlite_identifier(name));
            values.push((value, column.column_type));
        }
    }
    names.push("ingested_at".into());
    values.push((ingested_at, ColumnType::Timestamp));
    let placeholders: Vec<String> = values
        .iter()
        .enumerate()
        .map(|(index, (_, kind))| placeholder(index + 1, *kind, postgres))
        .collect();
    RecallStatement {
        sql: format!(
            "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT DO NOTHING",
            sqlite_identifier(table_spec(row.table).name),
            names.join(", "),
            placeholders.join(", ")
        ),
        values,
    }
}
