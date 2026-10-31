//! Names SQLQuack analysis matches.

pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
pub(crate) const DUCKDB_DIALECT: &str = "duckdb";
pub(crate) const QUOTE: char = '"';
pub(crate) const RELATION_PART_SEPARATOR: &str = ".";
/// How a table function call opens in model SQL.
pub(crate) const TABLE_FUNCTION_CALL: &str = "__table_fn(";
/// Passes of learning model columns from their own lineage; deeper chains stay open.
pub(crate) const MAX_LEARNING_PASSES: usize = 64;
