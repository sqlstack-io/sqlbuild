//! `type_tree`: a type name as one of SQLQuack's parsers reads it.

use serde_json::Value;

use crate::warehouse_types::_helpers::type_trees::{bigquery_type, core_type, postgres_type};
use crate::warehouse_types::models::TypeParser;

/// The type tree `parser` reads from `spelling`, or why it rejects it.
pub fn type_tree(spelling: &str, parser: TypeParser) -> Result<Value, String> {
    match parser {
        TypeParser::Postgres => postgres_type(spelling),
        TypeParser::BigQuery => bigquery_type(spelling),
        TypeParser::Core(dialect) => core_type(spelling, dialect),
    }
}
