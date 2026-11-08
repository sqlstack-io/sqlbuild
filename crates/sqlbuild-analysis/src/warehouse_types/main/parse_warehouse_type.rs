//! `parse_warehouse_type`: a warehouse type name as SQLQuack's parser for its dialect reads it.

use serde_json::Value;
use sqlquack_syntax::tokens::models::Dialect;

use crate::warehouse_types::constants::{BIGQUERY_DIALECT, POSTGRES_DIALECT, SNOWFLAKE_DIALECT};
use crate::warehouse_types::main::type_tree::type_tree;
use crate::warehouse_types::models::TypeParser;

/// The type tree of `spelling` in `dialect`, None without a parser, or why the parser rejects it.
pub fn parse_warehouse_type(spelling: &str, dialect: &str) -> Result<Option<Value>, String> {
    match dialect {
        POSTGRES_DIALECT => type_tree(spelling, TypeParser::Postgres).map(Some),
        SNOWFLAKE_DIALECT => type_tree(spelling, TypeParser::Core(Dialect::Snowflake)).map(Some),
        BIGQUERY_DIALECT => type_tree(spelling, TypeParser::BigQuery).map(Some),
        _ => Ok(None),
    }
}
