pub(crate) const REF_STUB: &str = "__sqlbuild_rules_ref_";
pub(crate) const SOURCE_STUB: &str = "__sqlbuild_rules_source_";
pub(crate) const CALL_STUB: &str = "__sqlbuild_rules_call_";
pub(crate) const TABLE_FUNCTION_STUB: &str = "__sqlbuild_table_function_";
pub(crate) const UDF_STUB: &str = "__sqlbuild_udf_";
pub(crate) const PLACEHOLDER_PREFIX: &[u8] = b"@@@";
pub(crate) const PLACEHOLDER_VALUE: &str = "NULL";
pub(crate) const MAX_FUNCTION_CALL_DEPTH: usize = 128;
/// The deepest statement built-in rules read, Polyglot's former AST-depth limit.
pub(crate) const MAX_AST_DEPTH: usize = 512;
pub(crate) const UDF_CALL: &str = "__udf";
pub(crate) const TABLE_FUNCTION_CALL: &str = "__table_fn";
pub(crate) const MIN_CAST_TOKENS: usize = 2;
pub(crate) const TRIPLE_QUOTE_LENGTH: usize = 3;
/// Dialect names read with the shared DuckDB grammar; `generic` matches the fingerprint tokenizer.
pub(crate) const DUCKDB_DIALECTS: [&str; 3] = ["duckdb", "motherduck", "generic"];
/// Dialect names read with the shared PostgreSQL grammar.
pub(crate) const POSTGRES_DIALECTS: [&str; 2] = ["postgres", "postgresql"];
/// The Snowflake dialect name.
pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
/// The SQLite dialect name.
pub(crate) const SQLITE_DIALECT: &str = "sqlite";
/// Snowflake calls with a date-part argument, as (name, argument count, date-part index).
pub(crate) const SNOWFLAKE_DATE_PART_ARGUMENTS: [(&str, usize, usize); 5] = [
    ("DATEDIFF", 3, 0),
    ("DATE_DIFF", 3, 0),
    ("TIMEDIFF", 3, 0),
    ("TIMESTAMPDIFF", 3, 0),
    ("LAST_DAY", 2, 1),
];
