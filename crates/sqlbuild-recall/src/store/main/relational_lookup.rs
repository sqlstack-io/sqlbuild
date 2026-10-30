//! Duplicate detection uses the same stable keys as physical unique constraints.

use crate::records::models::{RecallRow, RecallValue};
use crate::schema::_helpers::rendering::sqlite_identifier;
use crate::schema::main::table_columns::table_columns;
use crate::schema::main::table_spec::table_spec;
use crate::schema::models::Table;
use crate::store::_helpers::relational::placeholder;
use crate::store::models::RecallStatement;

/// Control event lookups return their durable cursor too; standalone stores have no cursor column.
#[must_use]
pub fn relational_lookup(row: &RecallRow, postgres: bool, control: bool) -> RecallStatement<'_> {
    let spec = table_spec(row.table);
    let mut keys = spec.key.to_vec();
    if postgres && row.table != Table::Artifacts && !keys.contains(&spec.expiry_column) {
        keys.push(spec.expiry_column);
    }
    let metadata = table_columns(row.table);
    let mut values = Vec::new();
    let mut conditions = Vec::new();
    for key in keys {
        let kind = metadata
            .iter()
            .find(|column| column.name == key)
            .expect("Recall key columns belong to their schema")
            .column_type;
        values.push((row.value(key).unwrap_or(&RecallValue::Null), kind));
        conditions.push(format!(
            "{} = {}",
            sqlite_identifier(key),
            placeholder(values.len(), kind, postgres)
        ));
    }
    let cursor = if control && row.table == Table::Events {
        ", storage_id"
    } else {
        ""
    };
    RecallStatement {
        sql: format!(
            "SELECT content_hash{cursor} FROM {} WHERE {}",
            sqlite_identifier(spec.name),
            conditions.join(" AND ")
        ),
        values,
    }
}
