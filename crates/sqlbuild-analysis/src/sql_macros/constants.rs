//! Error codes and keywords of SQL macros.

/// A macro call that does not say whether it calls a SQL or a Python macro.
pub const BARE_MACRO_CALL_CODE: &str = "P014";
/// A SQL macro file whose header or body is not a valid SQL macro.
pub const INVALID_DEFINITION_CODE: &str = "P015";
/// A reference function in a SQL macro body.
pub const BODY_REFERENCE_CODE: &str = "P016";
/// A call whose arguments do not match the macro's parameters.
pub const ARGUMENT_CODE: &str = "P017";
/// The unknown-column code an argument finding supersedes inside its call's span.
pub const SUPERSEDED_ANALYSIS_CODE: &str = crate::semantic_checks::constants::UNKNOWN_COLUMN_CODE;
/// A CTE or column a macro inserts that clashes with the caller's.
pub const NAME_CLASH_CODE: &str = "P018";
/// SQL macros that call each other in a cycle.
pub const CYCLE_CODE: &str = "P019";
/// A SQL macro and a Python macro calling each other, or one called as the other.
pub const MIXED_KIND_CODE: &str = "P020";
/// A CTE-list or column-list call outside a `WITH` list or select list.
pub const MISPLACED_CALL_CODE: &str = "P021";
/// An `@name` in a SQL macro body naming no parameter, or a parameter written without `@`.
pub const PARAMETER_REFERENCE_CODE: &str = "P022";
/// A runtime parameter a SQL macro body uses where SQL requires a literal.
pub const LITERAL_REQUIRED_CODE: &str = "P023";
/// An expression macro whose body's known type is not its declared `returns`.
pub const RETURNS_MISMATCH_CODE: &str = "P024";
/// An expression macro body that misuses the type of an `ANY` family argument.
pub const BODY_TYPE_CODE: &str = "P025";
/// A declaration a SQL macro body uses that is not visible from the macro file.
pub const BODY_SCOPE_CODE: &str = "S026";

/// The keyword a SQL macro file starts with.
pub(crate) const HEADER_KEYWORD: &str = "MACRO";
pub(crate) const DESCRIPTION_KEY: &str = "description";
pub(crate) const ARGUMENTS_KEY: &str = "arguments";
pub(crate) const RETURNS_KEY: &str = "returns";
pub(crate) const CONST_KEYWORD: &str = "CONST";
pub(crate) const DEFAULT_KEYWORD: &str = "DEFAULT";
pub(crate) const RELATION_KEYWORD: &str = "RELATION";
/// The type word of a `RELATION` slot that groups caller-chosen columns.
pub(crate) const COLUMNS_KEYWORD: &str = "COLUMNS";
/// The prefix of the word a single-column slot read stands as while a body is parsed.
pub(crate) const SLOT_PLACEHOLDER_PREFIX: &str = "__sqlbuild_slot__";
/// Where a `COLUMNS` slot may stand, as messages list them.
pub(crate) const GROUP_POSITIONS: &str =
    "a select list, PARTITION BY, GROUP BY, ORDER BY or USING (...)";
pub(crate) const OF_KEYWORD: &str = "OF";
pub(crate) const TYPE_KEYWORD: &str = "TYPE";
pub(crate) const ENUM_KEYWORD: &str = "ENUM";
pub(crate) const LIST_KEYWORD: &str = "LIST";
pub(crate) const SET_KEYWORD: &str = "SET";
/// Names a parameter may not take, because `@name` already means something else.
pub(crate) const RESERVED_PARAMETER_NAMES: [&str; 6] =
    ["sql", "py", "const", "enum", "param", CTE_ARGUMENT];
/// The reserved call argument naming the CTEs a CTE-list macro call inserts.
pub const CTE_ARGUMENT: &str = "__cte";
/// What joins a call's `__cte` name to the name of each CTE but the last.
pub(crate) const CTE_NAME_SEPARATOR: &str = "__";
/// Type names that would make a parameter raw, untyped text.
pub(crate) const RAW_TYPE_NAMES: [&str; 3] = ["RAW", "SQL", "EXPRESSION"];
/// The type of a parameter that takes any scalar expression and checks no type.
pub(crate) const ANY_KEYWORD: &str = "ANY";
/// Reference functions a SQL macro body may not call.
pub(crate) const REFERENCE_FUNCTIONS: [&str; 3] = ["__ref", "__source", "__seed"];
/// The prefix of a SQL macro call.
pub const SQL_CALL_PREFIX: &str = "@sql.";
/// The prefix of a Python macro call.
pub const PYTHON_CALL_PREFIX: &str = "@py.";
/// SQL words that are literals on their own.
pub(crate) const NULL_WORD: &str = "NULL";
pub(crate) const BOOLEAN_WORDS: [&str; 2] = ["TRUE", "FALSE"];
/// Tokens a named argument needs at least: its name, `:=` and a value.
pub(crate) const NAMED_ARGUMENT_MIN_TOKENS: usize = 3;
/// Tokens a `RELATION` column needs at least: its name and a type.
pub(crate) const RELATION_COLUMN_MIN_TOKENS: usize = 2;
/// The kind word of a SQL macro call, as in `@sql.name(...)`.
pub(crate) const SQL_KIND_WORD: &str = "sql";
/// The noun naming an inserted CTE in clash errors.
pub(crate) const CTE_NOUN: &str = "CTE";
/// Clause keywords a call position reads.
pub(crate) const WITH_KEYWORD: &str = "WITH";
pub(crate) const SELECT_KEYWORD: &str = "SELECT";
/// Punctuation and operators the scans compare against.
pub(crate) const COMMA: &str = ",";
pub(crate) const AT_SIGN: &str = "@";
/// The function whose precision some adapters need as a literal.
pub(crate) const ROUND_FUNCTION: &str = "ROUND";
/// The argument index of `ROUND` precision.
pub(crate) const ROUND_PRECISION_ARGUMENT: usize = 1;
/// The first byte of a multi-byte UTF-8 character, which may continue an identifier.
pub(crate) const NON_ASCII_BYTE: u8 = 0x80;
/// Declaration role folders that publish to their owner folder and below.
pub(crate) const INHERITED_ROLES: [&str; 3] = ["macros", "enums", "constants"];
/// Declaration role folders that publish to their owner folder only.
pub(crate) const LOCAL_ROLES: [&str; 3] = ["_macros", "_enums", "_constants"];
/// The folder grouping a resource folder's declaration roles.
pub(crate) const DECLARATION_GROUP: &str = "_sqlbuild";
/// The alias prefix of each argument a type probe selects.
pub(crate) const PROBE_COLUMN_PREFIX: &str = "__sqlbuild_argument_";
/// Clause keywords that end the `FROM ... WHERE ...` part a probe selects through.
pub(crate) const PROBE_TAIL_ENDS: [&str; 11] = [
    "GROUP",
    "HAVING",
    "QUALIFY",
    "WINDOW",
    "ORDER",
    "LIMIT",
    "UNION",
    "INTERSECT",
    "EXCEPT",
    "OFFSET",
    "FETCH",
];
/// The string type name inference reports, and the name messages show for it.
pub(crate) const INFERRED_TEXT_TYPE: &str = "TEXT";
pub(crate) const VARCHAR_TYPE: &str = "VARCHAR";
pub(crate) const FROM_KEYWORD: &str = "FROM";
/// Words that end a select item rather than alias it.
pub(crate) const ITEM_ENDS: [&str; 4] = ["FROM", "WHERE", "UNION", "ORDER"];
/// The relation whose columns stand in for a macro's arguments while its body is typed.
pub(crate) const STAND_IN_RELATION: &str = "__sqlbuild_macro_arguments";
/// The analysis error codes, by number, of operands and arguments of a wrong type.
pub(crate) const TYPE_ERROR_CODES: (usize, usize) = (210, 219);
/// The analysis warning codes, by number, of implicit conversions.
pub(crate) const TYPE_WARNING_CODES: (usize, usize) = (210, 216);
pub(crate) const STAR_PROJECTION: &str = "*";
/// The type hint analysis reports for a column whose type it does not know.
pub(crate) const UNKNOWN_TYPE_HINT: &str = "UNKNOWN";
/// Words before a name in a `FROM` clause after which the name is a relation, not an alias.
pub(crate) const ALIAS_PRECEDING_KEYWORDS: [&str; 13] = [
    "FROM", "JOIN", "ON", "AND", "OR", "NOT", "WHERE", "USING", "LATERAL", "IN", "IS", "BY",
    "SELECT",
];
/// The literal a typed marker still types, though it is one bare word.
pub(crate) const NULL_KEYWORD: &str = "NULL";
/// The largest DECIMAL precision each dialect that bounds it holds, by analysis dialect name.
pub(crate) const DECIMAL_PRECISION_LIMITS: [(&str, u64); 2] = [("duckdb", 38), ("postgres", 1000)];
/// BigQuery's most integer digits and scale of a `NUMERIC`.
pub(crate) const BIGQUERY_NUMERIC_LIMITS: (u32, u32) = (29, 9);
/// BigQuery's most integer digits and scale of a `BIGNUMERIC`.
pub(crate) const BIGQUERY_BIGNUMERIC_LIMITS: (u32, u32) = (38, 38);
/// BigQuery's spellings of `BIGNUMERIC`.
pub(crate) const BIGQUERY_BIG_DECIMAL_NAMES: [&str; 2] = ["BIGNUMERIC", "BIGDECIMAL"];
/// How a relation function such as `__ref("orders")` starts.
pub(crate) const RELATION_FUNCTION_PREFIX: &str = "__";
