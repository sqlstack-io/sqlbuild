//! `rules_tree_parser`: the parser that reads a dialect's SQL for rules.

use sqlquack_syntax::Dialect;

use crate::sql_tree::constants::{
    DUCKDB_DIALECTS, POSTGRES_DIALECTS, SNOWFLAKE_DIALECT, SQLITE_DIALECT,
};
use crate::sql_tree::models::TreeParser;

/// The parser for `dialect`, or `None`; add a dialect (one line) only once its parser passes the depth probe with an empty allowlist.
pub(crate) fn rules_tree_parser(dialect: &str) -> Option<TreeParser> {
    if POSTGRES_DIALECTS.contains(&dialect) {
        return Some(TreeParser::Postgres);
    }
    if DUCKDB_DIALECTS.contains(&dialect) {
        return Some(TreeParser::Shared(Dialect::DuckDB));
    }
    match dialect {
        SNOWFLAKE_DIALECT => Some(TreeParser::Shared(Dialect::Snowflake)),
        SQLITE_DIALECT => Some(TreeParser::Shared(Dialect::SQLite)),
        _ => None,
    }
}
