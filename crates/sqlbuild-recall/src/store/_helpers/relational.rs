//! Typed placeholders shared by native SQLite and control-plane transactions.

use crate::schema::_helpers::relational_tables::postgres_type;
use crate::schema::models::ColumnType;

pub(crate) fn placeholder(index: usize, kind: ColumnType, postgres: bool) -> String {
    if postgres {
        format!("CAST(${index} AS {})", postgres_type(kind))
    } else {
        format!("${index}")
    }
}
