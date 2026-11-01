//! Names SQLQuack analysis matches.

pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
pub(crate) const DUCKDB_DIALECT: &str = "duckdb";
pub(crate) const POSTGRES_DIALECT: &str = "postgres";
pub(crate) const POSTGRESQL_DIALECT: &str = "postgresql";
pub(crate) const QUOTE: char = '"';
pub(crate) const RELATION_PART_SEPARATOR: &str = ".";
/// How a table function call opens in model SQL.
pub(crate) const TABLE_FUNCTION_CALL: &str = "__table_fn(";
/// Passes of learning model columns from their own lineage; deeper chains stay open.
pub(crate) const MAX_LEARNING_PASSES: usize = 64;
/// The catalog's nullability spellings.
pub(crate) const NON_NULL: &str = "non_null";
pub(crate) const NULLABLE: &str = "nullable";
pub(crate) const UNKNOWN_NULLABILITY: &str = "unknown";
/// SQLBuild's spellings of unsized and sized text.
pub(crate) const TEXT_TYPE: &str = "TEXT";
pub(crate) const VARCHAR_TYPE: &str = "VARCHAR";
/// The adapter's nullability rule ids.
pub(crate) const FIRST_ARGUMENT_RULE: &str = "first_arg";
pub(crate) const CONDITIONAL_RESULT_RULE: &str = "conditional_result";
pub(crate) const SAFE_CAST_RULE: &str = "safe_cast";
/// Grouping syntax DuckDB's ordinary function catalog does not list, taking one argument or more.
pub(crate) const GROUPING_FUNCTIONS: [&str; 5] =
    ["grouping", "grouping_id", "grouping sets", "cube", "rollup"];
