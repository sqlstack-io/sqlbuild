//! `shared_dialect`: the grammar of SQLQuack's shared parser that reads a dialect's SQL.

use sqlquack_syntax::Dialect;

use crate::sql_tree::constants::{
    DUCKDB_DIALECTS, POSTGRES_DIALECTS, SNOWFLAKE_DIALECT, SQLITE_DIALECT,
};

/// The shared-parser grammar for `dialect`, or `None` when the shared parser reads none of it.
pub fn shared_dialect(dialect: &str) -> Option<Dialect> {
    if DUCKDB_DIALECTS.contains(&dialect) {
        return Some(Dialect::DuckDB);
    }
    if POSTGRES_DIALECTS.contains(&dialect) {
        return Some(Dialect::PostgreSQL);
    }
    match dialect {
        SNOWFLAKE_DIALECT => Some(Dialect::Snowflake),
        SQLITE_DIALECT => Some(Dialect::SQLite),
        _ => None,
    }
}
