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
pub(crate) const TABLE_WRAPPER: &str = "TABLE(";
pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
pub(crate) const MIN_CAST_TOKENS: usize = 2;
pub(crate) const TRIPLE_QUOTE_LENGTH: usize = 3;
/// Snowflake calls with a date-part argument, as (name, argument count, date-part index).
pub(crate) const SNOWFLAKE_DATE_PART_ARGUMENTS: [(&str, usize, usize); 5] = [
    ("DATEDIFF", 3, 0),
    ("DATE_DIFF", 3, 0),
    ("TIMEDIFF", 3, 0),
    ("TIMESTAMPDIFF", 3, 0),
    ("LAST_DAY", 2, 1),
];
