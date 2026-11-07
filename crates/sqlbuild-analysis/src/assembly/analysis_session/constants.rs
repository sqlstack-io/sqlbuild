//! Wire values shared with Python's model analysis.

/// The native engine's request for Python's legacy type recovery.
pub(crate) const LEGACY_FALLBACK: &str = "native project type recovery requires legacy fallback";
/// Python logs this for a malformed per-model response.
pub(crate) const INVALID_NATIVE_RESPONSE: &str = "invalid native response";
/// Python's `UNKNOWN` column type.
pub(crate) const UNKNOWN_TYPE: &str = "UNKNOWN";
/// Python's `InferredNullability.UNKNOWN`.
pub(crate) const UNKNOWN_NULLABILITY: &str = "unknown";
/// Python's `InferredNullability` values by native code.
pub(crate) const NULLABILITY_BY_CODE: [&str; 3] = ["unknown", "non_null", "nullable"];
/// The transform and confidence code counts Python's enum tuples accept.
pub(crate) const TRANSFORM_CODES: u64 = 6;
pub(crate) const CONFIDENCE_CODES: u64 = 3;
/// Python's `COMPACT_ANALYSIS_RESPONSE_LENGTH`: a member's `[template, mappings]`.
pub(crate) const RESPONSE_LENGTH: usize = 2;
/// Python's `COMPACT_ANALYSIS_LEGACY_RESPONSE_LENGTH`: `[rows, has_star, flag]`.
pub(crate) const LEGACY_RESPONSE_LENGTH: usize = 3;
/// Python's `COMPACT_ANALYSIS_FACT_LENGTH`.
pub(crate) const FACT_LENGTH: usize = 6;
/// Python's `COMPACT_ANALYSIS_SOURCE_LENGTH`.
pub(crate) const SOURCE_LENGTH: usize = 3;
/// Python's `BINDING_SEVERITIES`.
pub(crate) const BINDING_SEVERITIES: [&str; 2] = ["error", "warning"];
/// Python's default binding message.
pub(crate) const DEFAULT_BINDING_MESSAGE: &str = "SQL binding failed";
/// Analysis workers per compact batch, as Python requests.
pub(crate) const COMPACT_WORKERS: usize = 4;
/// Members a large batch analyses at a time, bounding the native response a run holds.
pub(crate) const BATCH_CHUNK_MEMBERS: usize = 512;
/// Python's `NATIVE_DIALECT_ALIASES`.
pub(crate) const DIALECT_ALIASES: [(&str, &str); 3] = [
    ("postgres", "postgresql"),
    ("motherduck", "duckdb"),
    ("sqlserver", "tsql"),
];
/// The quote that marks an exact identifier.
pub(crate) const QUOTED_IDENTIFIER_DELIMITER: char = '"';
/// Python's `ColumnTransformKind` compact codes.
pub(crate) const TRANSFORM_DIRECT: u8 = 0;
pub(crate) const TRANSFORM_CAST: u8 = 1;
pub(crate) const TRANSFORM_EXPRESSION: u8 = 2;
pub(crate) const TRANSFORM_AGGREGATION: u8 = 3;
pub(crate) const TRANSFORM_STAR: u8 = 4;
pub(crate) const TRANSFORM_CONSTANT: u8 = 5;
/// Python's `ColumnLineageConfidence` compact codes.
pub(crate) const CONFIDENCE_UNKNOWN: u8 = 0;
pub(crate) const CONFIDENCE_HIGH: u8 = 1;
pub(crate) const CONFIDENCE_MEDIUM: u8 = 2;
/// The `analyze_query` facts Python's re-analysis reads.
pub(crate) const SELECT_SHAPE: &str = "select";
pub(crate) const SET_OPERATION_SHAPE: &str = "set_operation";
pub(crate) const CAST_TRANSFORM: &str = "cast";
pub(crate) const FILTER_CONTEXT: &str = "filter";
pub(crate) const NULL_KEYWORD: &str = "NULL";
pub(crate) const RESOLVED_SOURCE_CONFIDENCE: &str = "resolved";
pub(crate) const WILDCARD: &str = "*";
/// SQLBuild's `analyze_query` complexity guard.
pub(crate) const MAX_FUNCTION_CALL_DEPTH: usize = 512;
/// The dialects whose dynamic pivots the compiler proves, and those with DuckDB's PIVOT form.
pub(crate) const SUPPORTED_PIVOT_DIALECTS: [&str; 3] = ["duckdb", "motherduck", "snowflake"];
pub(crate) const SIMPLIFIED_PIVOT_DIALECTS: [&str; 2] = ["duckdb", "motherduck"];
pub(crate) const DUCKDB_DIALECT: &str = "duckdb";
pub(crate) const MOTHERDUCK_DIALECT: &str = "motherduck";
/// The `to_dict()` node keys Python's dynamic pivot proof walks.
pub(crate) const PIVOT_AST_KIND: &str = "pivot";
pub(crate) const SELECT_AST_KIND: &str = "select";
pub(crate) const QUERY_AST_KINDS: [&str; 2] = ["query", "subquery"];
pub(crate) const CAST_AST_KIND: &str = "cast";
pub(crate) const COLUMN_AST_KIND: &str = "column";
pub(crate) const TABLE_AST_KIND: &str = "table";
pub(crate) const FUNCTION_AST_KIND: &str = "function";
pub(crate) const ALIAS_AST_KIND: &str = "alias";
pub(crate) const STAR_AST_KIND: &str = "star";
pub(crate) const DYNAMIC_VALUE_SOURCE_KINDS: [&str; 3] = ["pivot_any", "select", "query"];
pub(crate) const DEPENDENCY_FUNCTION_NAMES: [&str; 3] = ["__ref", "__source", "__seed"];
pub(crate) const TYPE_PRESERVING_AGGREGATES: [&str; 3] = ["ANY_VALUE", "MAX", "MIN"];
/// Python's `_render_type` names for cast targets.
pub(crate) const RENDERED_TYPE_NAMES: [(&str, &str); 12] = [
    ("big_int", "BIGINT"),
    ("bool", "BOOLEAN"),
    ("boolean", "BOOLEAN"),
    ("decimal", "DECIMAL"),
    ("double", "DOUBLE"),
    ("float", "FLOAT"),
    ("int", "INT"),
    ("integer", "INT"),
    ("text", "TEXT"),
    ("timestamp", "TIMESTAMP"),
    ("var_char", "VARCHAR"),
    ("varchar", "VARCHAR"),
];
/// The stack a standalone dynamic pivot proof parses on, as the analysis workers have.
pub(crate) const PIVOT_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;
/// Python's `COMPACT_RELATION_STUB_PREFIX`: the relation name a shared query binds instead.
pub(crate) const RELATION_STUB_PREFIX: &str = "__sqlbuild_project_input_";
/// Python's `COMPACT_REFERENCE_MARKER_PATTERN`: a reference call and the name it reads.
pub(crate) const REFERENCE_MARKER_PATTERN: &str =
    r#"__(ref|seed|source|dbt_ref)\((?:"[^"]+"\s*,\s*)?"([^"]+)"\)"#;
/// Python's `COMPACT_UNSTUBBED_REFERENCE_KIND`, a reference kind sharing never stubs.
pub(crate) const UNSTUBBED_REFERENCE_KIND: &str = "dbt_ref";
/// Python's `MIN_SHARED_BINDING_QUERY_MEMBERS`.
pub(crate) const MIN_SHARED_MEMBERS: usize = 2;
/// Text whose presence sends Python's qualifier search to its token scanner.
pub(crate) const QUALIFIED_SCAN_TOKENS: [&str; 5] = ["\"", "`", "[", "--", "/*"];
/// Python's `str.isspace` over ASCII.
pub(crate) const ASCII_SPACES: &str = " \t\n\r\x0b\x0c";
/// Stack for one standalone CTE fact recovery, whose AST walks recurse with query depth.
pub(crate) const CTE_FACT_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;
/// The wheel's node kinds Python's CTE fact recovery reads.
pub(crate) const ANNOTATED_AST_KIND: &str = "annotated";
pub(crate) const LITERAL_AST_KIND: &str = "literal";
pub(crate) const NULL_AST_KIND: &str = "null";
pub(crate) const CONCAT_AST_KIND: &str = "concat";
pub(crate) const SUBSTRING_AST_KIND: &str = "substring";
pub(crate) const COALESCE_AST_KIND: &str = "coalesce";
pub(crate) const IF_FUNC_AST_KIND: &str = "if_func";
pub(crate) const CASE_AST_KIND: &str = "case";
pub(crate) const TRY_CAST_AST_KIND: &str = "try_cast";
pub(crate) const COUNT_AST_KIND: &str = "count";
pub(crate) const IS_NULL_AST_KIND: &str = "is_null";
pub(crate) const SET_OPERATION_AST_KINDS: [&str; 3] = ["union", "intersect", "except"];
pub(crate) const CAST_AST_KINDS: [&str; 2] = ["cast", "try_cast"];
pub(crate) const TYPE_PASSTHROUGH_AST_KINDS: [&str; 4] =
    ["max", "min", "window_function", "within_group"];
/// Python's `POLYGLOT_BOOLEAN_RESULT_KINDS`.
pub(crate) const BOOLEAN_RESULT_AST_KINDS: [&str; 18] = [
    "boolean",
    "and",
    "between",
    "eq",
    "exists",
    "gt",
    "gte",
    "ilike",
    "in",
    "is",
    "is_null",
    "like",
    "lt",
    "lte",
    "neq",
    "not",
    "or",
    "regexp_like",
];
pub(crate) const NULLIF_FUNCTION_NAME: &str = "NULLIF";
pub(crate) const STRING_LITERAL_TYPE: &str = "string";
pub(crate) const BINARY_OPERAND_COUNT: usize = 2;
pub(crate) const AGGREGATE_AST_KINDS: [&str; 7] = [
    "avg",
    "count",
    "max",
    "min",
    "sum",
    "array_agg",
    "string_agg",
];
/// The type Python's set operation slots give a bare NULL.
pub(crate) const NULL_SET_OPERATION_TYPE: &str = "__SQLBUILD_NULL_SET_OPERATION_TYPE__";
pub(crate) const BOOLEAN_TYPE: &str = "BOOLEAN";
pub(crate) const TEXT_TYPE: &str = "TEXT";
pub(crate) const CUSTOM_TYPE_NAME: &str = "CUSTOM";
pub(crate) const TIMESTAMP_TYPE_NAME: &str = "timestamp";
pub(crate) const TIMESTAMP_TZ_TYPE: &str = "TIMESTAMPTZ";
pub(crate) const DECIMAL_TYPE: &str = "DECIMAL";
pub(crate) const VARCHAR_DATA_TYPES: [&str; 2] = ["var_char", "varchar"];
/// Python's `_polyglot_type_name` names, matched case-sensitively as Python's dict is.
pub(crate) const POLYGLOT_TYPE_NAMES: [(&str, &str); 13] = [
    ("big_int", "BIGINT"),
    ("bool", "BOOLEAN"),
    ("boolean", "BOOLEAN"),
    ("date", "DATE"),
    ("decimal", "DECIMAL"),
    ("double", "DOUBLE"),
    ("float", "FLOAT"),
    ("int", "INT"),
    ("integer", "INT"),
    ("text", "TEXT"),
    ("timestamp", "TIMESTAMP"),
    ("var_char", "TEXT"),
    ("varchar", "TEXT"),
];
/// Python's join sides that make relations nullable.
pub(crate) const JOIN_LEFT: &str = "LEFT";
pub(crate) const JOIN_RIGHT: &str = "RIGHT";
pub(crate) const JOIN_FULL: &str = "FULL";
/// Ids of the adapter nullability rules Python registers, by the function it calls.
pub(crate) const FIRST_ARG_RULE: &str = "first_arg";
pub(crate) const CONDITIONAL_RESULT_RULE: &str = "conditional_result";
/// The rule id of `safe_cast_nullability`.
pub(crate) const SAFE_CAST_RULE: &str = "safe_cast";
/// The rule id of a project-local adapter's own rule, which the callback runs.
pub(crate) const PYTHON_RULE: &str = "python";
pub(crate) const NON_NULL_NULLABILITY: &str = "non_null";
pub(crate) const NULLABLE_NULLABILITY: &str = "nullable";
/// Bumped whenever the stored outcome or a key's fields change; older entries never match.
pub(crate) const ANALYSIS_CACHE_FORMAT: &str = "native-model-analysis-v3";
/// A stored varint byte at or above this value continues into the next byte.
pub(crate) const VARINT_CONTINUATION: u64 = 0x80;
/// The value bits each stored varint byte carries.
pub(crate) const VARINT_PAYLOAD_MASK: u8 = 0x7f;
/// How many value bits each stored varint byte carries.
pub(crate) const VARINT_PAYLOAD_BITS: u32 = 7;
/// The most bytes one stored `u64` varint takes.
pub(crate) const VARINT_MAX_BYTES: usize = 10;
/// Payload kinds and literal types Python's fixture projection facts read.
pub(crate) const NULL_PAYLOAD_KIND: &str = "null";
pub(crate) const LITERAL_PAYLOAD_KIND: &str = "literal";
pub(crate) const ARRAY_PAYLOAD_KIND: &str = "array_func";
pub(crate) const STRUCTURED_PAYLOAD_KINDS: [&str; 2] = ["parse_json", "struct"];
pub(crate) const FUNCTION_PAYLOAD_KIND: &str = "function";
pub(crate) const SELECT_PAYLOAD_KEY: &str = "select";
/// Set operations whose output columns are named and typed by their first branch.
pub(crate) const SET_OPERATION_PAYLOAD_KEYS: [&str; 3] = ["union", "intersect", "except"];
/// `{literal_type: type}` Python's `_simple_expression_type` maps a literal to.
pub(crate) const LITERAL_FIXTURE_TYPES: [(&str, &str); 3] = [
    ("string", "VARCHAR"),
    ("number", "NUMBER"),
    ("boolean", "BOOLEAN"),
];
/// `{function name: type}` Python's `_simple_expression_type` maps a function to.
pub(crate) const FUNCTION_FIXTURE_TYPES: [(&str, &str); 3] = [
    ("ARRAY_CONSTRUCT", "ARRAY"),
    ("OBJECT_CONSTRUCT", "OBJECT"),
    ("PARSE_JSON", "VARIANT"),
];
pub(crate) const ARRAY_FIXTURE_TYPE: &str = "ARRAY";
pub(crate) const VARIANT_FIXTURE_TYPE: &str = "VARIANT";
/// Dialect, payload keys and table args Python's SQL-test `_parsed_reads` reads.
pub(crate) const HELPER_READS_DIALECT: &str = "generic";
pub(crate) const CTE_LIST_PAYLOAD_KEY: &str = "ctes";
pub(crate) const CTE_ALIAS_PAYLOAD_KEY: &str = "alias";
pub(crate) const NAME_PAYLOAD_KEY: &str = "name";
pub(crate) const SCHEMA_TABLE_ARG: &str = "schema";
pub(crate) const CATALOG_TABLE_ARG: &str = "catalog";
/// The set-operation keywords `\b(?:UNION|INTERSECT|EXCEPT)\b` searches for, in lowercase.
pub(crate) const SET_OPERATION_KEYWORDS: [&str; 3] = ["union", "intersect", "except"];
/// The analysis relation name prefix of a table-function call.
pub(crate) const TABLE_FUNCTION_ANALYSIS_PREFIX: &str = "__sqlbuild_table_function_";
pub(crate) const REF_KIND: &str = "ref";
pub(crate) const UDF_KIND: &str = "udf";
pub(crate) const TABLE_FUNCTION_KIND: &str = "table_fn";
/// Python's `_LINEAGE_RESOURCE_TYPES`: reference kind and the resource type lineage names.
pub(crate) const LINEAGE_RESOURCE_TYPES: [(&str, &str); 4] = [
    ("ref", "model"),
    ("source", "source"),
    ("seed", "seed"),
    ("table_fn", "table_fn"),
];
/// The name codes SQLQuack reports in polyglot's place for a names-only dialect.
pub(crate) const SQLQUACK_NAME_CODES: [&str; 4] = ["B002", "B003", "B004", "B005"];
/// Analysis dialects whose names SQLQuack checks, polyglot checking everything else.
pub(crate) const SQLQUACK_NAMES_ONLY_DIALECTS: [&str; 2] = ["postgres", "postgresql"];
/// Polyglot's column-count and function codes, which it reports after names.
pub(crate) const TYPE_CHECK_CODES: [&str; 2] = ["B101", "B102"];
/// Polyglot's type-check and scope-rule code number ranges, which it reports after names.
pub(crate) const TYPE_CHECK_CODE_RANGES: [std::ops::RangeInclusive<u16>; 2] =
    [210..=219, 230..=234];
/// The words of SQLBuild's own B101 rows, a built-in spelled as the dialect does not accept.
pub(crate) const SPELLING_SUGGESTION: &str = "; did you mean ";
/// The severity of a diagnostic that does not fail the model.
pub(crate) const WARNING_SEVERITY: &str = "warning";
