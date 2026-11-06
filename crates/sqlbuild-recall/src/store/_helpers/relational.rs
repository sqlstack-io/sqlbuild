//! Typed placeholders shared by native SQLite and control-plane transactions.

use crate::schema::models::ColumnType;

pub(crate) fn placeholder(index: usize, kind: ColumnType, postgres: bool) -> String {
    if postgres {
        format!("CAST(${index} AS {})", kind.postgres_type())
    } else {
        format!("${index}")
    }
}
