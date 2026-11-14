//! Sentinel spellings, suppression texts and fixture-null markers.

/// Sentinel standing in for one interpolation site in lint and format SQL.
pub const LINT_SENTINEL_PREFIX: &str = sqlclean::constants::INTERPOLATION_PLACEHOLDER_PREFIX;
pub(crate) const SENTINEL_SUFFIX: &str = "__";
/// The keyword before a table function call that the parser reads only as a call.
pub(crate) const LATERAL_KEYWORD: &str = "LATERAL";
/// An audit's relation parameter, which a `SELECT *` may read alone.
pub(crate) const AUDIT_PARAMETER_SENTINEL_PREFIX: &str =
    sqlclean::constants::RELATION_PARAMETER_PLACEHOLDER_PREFIX;
pub(crate) const CONTEXT_SENTINEL_PREFIX: &str =
    sqlclean::constants::VALUE_PARAMETER_PLACEHOLDER_PREFIX;
pub(crate) const SQL_MACRO_PARAMETER_SENTINEL_PREFIX: &str =
    sqlclean::constants::MACRO_PARAMETER_PLACEHOLDER_PREFIX;
pub(crate) const CONTEXT_INTERPOLATION_TOKEN: &str = "@@CTX:";
pub(crate) const TEST_PARAMETER_SENTINEL_PREFIX: &str = "__sqlbuild_test_parameter_";
pub(crate) const TEST_PARAMETER_TOKEN: &str = "@param";

/// Every SQLBuild intrinsic call protected while formatting.
pub(crate) const FUNCTION_NAMES: [&str; 9] = [
    "__cursor_end",
    "__cursor_start",
    "__dbt_ref",
    "__empty_fixture",
    "__ref",
    "__seed",
    "__source",
    "__table_fn",
    "__udf",
];

/// Bytes every lint and format site pattern match starts with.
pub(crate) const SITE_START_BYTES: [u8; 3] = *b"@$_";
pub(crate) const LINT_SITE_PATTERN: &str =
    r"^(?:@@|\$\{|@|(?:__dbt_ref|__ref|__seed|__source|__table_fn|__udf)\()";
pub(crate) const FORMAT_SITE_PATTERN: &str = r"(?i)^(?:@@|\$\{|@|(?:__cursor_end|__cursor_start|__dbt_ref|__empty_fixture|__ref|__seed|__source|__table_fn|__udf)\s*\()";

/// SQLCLEAN044 advice pointing at SQLBuild constants, in place of SQLClean's generic advice.
pub(crate) const LONG_LITERAL_GUIDANCE: &str = "Define the value once as a constant and reference it with `@const`, in the narrowest scope that covers every use. (1) Only this model: add `constants (_status_pattern '<value>')` to its `MODEL (...)` header and write `@const(\"_status_pattern\")`. (2) Other files use the same value: declare `CONSTANT (name status_pattern, value '<value>');` in `_sqlbuild/_constants/` of the nearest folder that contains every use when they all sit directly in that folder, or in its `_sqlbuild/constants/` to include subfolders, and write `@const(\"status_pattern\")`. (3) Use top-level `constants/` only when the value is needed project-wide. Otherwise shorten the literal.";

/// Diagnostic code of every inline suppression directive finding.
pub const SUPPRESSION_CODE: &str = "SQBRSUPPRESSION000";
/// Text every directive contains, checked before scanning a file's comments.
pub(crate) const DIRECTIVE_MARKER: &[u8] = b"sqb:";
pub(crate) const BLOCK_COMMENT_OPENER: &str = "/*";
pub(crate) const BLOCK_COMMENT_CLOSER: &str = "*/";
/// Both comment openers are two bytes long.
pub(crate) const COMMENT_OPENER_BYTES: usize = 2;
pub(crate) const DIRECTIVE_PATTERN: &str =
    r"(?i)sqb:\s*ignore\b\s*(?P<code>[A-Za-z0-9_-]*)(?:\s+because\s+(?P<reason>.*\S))?";
pub(crate) const CROSS_JOIN_CODE: &str = "SQLCLEAN038";
pub(crate) const CROSS_JOIN_RELATION_PATTERN: &str = r#"(?i)\bcross\s+join\s+(?:__(?:ref|seed|source)\s*\(\s*['"](?P<dependency>[^'"]+)['"]\s*\)|(?P<identifier>[A-Za-z_][A-Za-z0-9_$.]*))"#;
pub(crate) const DIRECTIVE_MESSAGE: &str =
    "Inline Rule suppressions are not supported; declare the exception in sqlbuild_project.toml";
pub(crate) const RULE_PLACEHOLDER: &str = "<rule code>";
pub(crate) const REASON_PLACEHOLDER: &str = "<why this deviation is intentional>";
pub(crate) const DIRECTIVE_FIX_REASON: &str =
    "moving an exception into sqlbuild_project.toml requires review";

pub(crate) const FIXTURE_TYPED_NULL_PATTERN: &str = r"(?i)(?:CAST\s*\(\s*NULL\s+AS\b|NULL\s*::)";
pub(crate) const MODEL_FIXTURE_PATTERN: &str =
    r"(?i)\b__ref__(?P<name>[A-Za-z_][A-Za-z0-9_]*)\s+AS\s*\(";
pub(crate) const FIXTURE_CTE_MARKERS: [&str; 3] = ["__ref__", "__source__", "__seed__"];
pub(crate) const IGNORED_SEARCH_DIRECTORIES: [&str; 4] =
    [".git", ".venv", "target", "node_modules"];
pub(crate) const UNIT_TEST_ROOT: [&str; 2] = ["tests", "unit"];
pub(crate) const SQL_SUFFIX: &str = ".sql";

pub(crate) const FORMAT_API_VERSION: u32 = 1;
pub(crate) const MAX_FUNCTION_CALL_DEPTH: usize = 512;
pub(crate) const OMITTED_CEREMONIAL_SELECT_SQL: &str = "\nSELECT 1";
pub(crate) const OMITTED_SELECT_SUFFIX_PATTERN: &str = r"(?i)\s*\bSELECT\s+1\s*\z";
pub(crate) const FIXTURE_HEADER_KINDS: [&str; 2] = ["TEST", "SCENARIO"];
/// The header of a SQL macro file, whose body is an expression, a CTE list or a column list.
pub(crate) const MACRO_HEADER_KIND: &str = "MACRO";
/// The header of a SQL function file, whose body is an expression or, for a table function, a query.
pub(crate) const FUNCTION_HEADER_KIND: &str = "FUNCTION";
pub(crate) const LINE_COMMENT_PREFIX: &str = "--";
pub(crate) const DECLINED_WITHOUT_REASON: &str = "native formatter declined without a reason";
pub(crate) const MOVED_OMITTED_SELECT: &str =
    "native formatter moved the omitted ceremonial SELECT 1";
pub(crate) const DECLARATION_GROUP_DIRECTORY: &str = "_sqlbuild";
/// `GLOBAL_NAMED_DECLARATION_ROOTS`, in Python's dict order, with whether each holds audits.
pub(crate) const GLOBAL_NAMED_ROOTS: [(&[&str], bool); 5] = [
    (&["audits", "generic"], true),
    (&["audits", "singular"], true),
    (&["schemas"], false),
    (&["hooks", "sql"], false),
    (&["hooks", "python"], false),
];
/// `GROUPED_NAMED_DECLARATION_ROLES`, in Python's dict order, with whether each holds audits.
pub(crate) const GROUPED_NAMED_ROLES: [(&[&str], bool); 9] = [
    (&["audits", "generic"], true),
    (&["_audits", "generic"], true),
    (&["audits", "singular"], true),
    (&["schemas"], false),
    (&["_schemas"], false),
    (&["hooks", "sql"], false),
    (&["_hooks", "sql"], false),
    (&["hooks", "python"], false),
    (&["_hooks", "python"], false),
];

pub(crate) const LINT_API_VERSION: i64 = 1;
pub(crate) const UNUSED_CTE_CODE: &str = "SQLCLEAN005";
pub(crate) const LONG_LITERAL_CODE: &str = "SQLCLEAN044";
pub(crate) const MIXED_QUALIFICATION_CODE: &str = "SQLCLEAN028";
/// The reserved argument naming a CTE-list call's CTEs; it passes no relation.
pub(crate) const CTE_ARGUMENT: &str = "__cte";
/// The help for mixed qualification in a SQL macro body, where slot reads are qualified through an alias.
pub(crate) const MACRO_MIXED_QUALIFICATION_REMEDIATION: &str = "Use one consistent qualification style in this query scope. A plain slot read such as `@rel.column` counts as unqualified; to qualify it, read through an alias: `FROM @rel AS s`, then `s.@rel.column`.";
pub(crate) const NATIVE_LINT_ERROR_CODE: &str = "L003";
/// The warning code for SQL the native lint engine cannot parse in the project's dialect.
pub(crate) const LINT_UNAVAILABLE_CODE: &str = "L007";
pub(crate) const LINT_UNAVAILABLE_REMEDIATION: &str =
    "This SQL was not linted. Review it by hand until SQL lint supports this dialect.";
pub(crate) const PARSE_ERROR_POSITION_PATTERN: &str =
    r"^Parse error at line (?P<line>\d+), column (?P<column>\d+):";
pub(crate) const HARNESS_CTE_PREFIXES: [&str; 13] = [
    "__expected__",
    "__assert__",
    "__ref__",
    "__source__",
    "__seed__",
    "__dbt_ref__",
    "__macro__",
    "__macro_actual__",
    "__macro_expected__",
    "__udf_actual__",
    "__udf_expected__",
    "__table_fn_actual__",
    "__table_fn_expected__",
];
/// The CTE names a test or macro harness generates, which stay reachable when the body does not read them.
pub(crate) const FIXTURE_HARNESS_CTE_PREFIXES: [&str; 11] = [
    "__expected__",
    "__assert__",
    "__ref__",
    "__source__",
    "__seed__",
    "__dbt_ref__",
    "__macro_actual__",
    "__macro_expected__",
    "__udf_actual__",
    "__udf_expected__",
    "__table_fn_",
];
/// The table function an empty fixture input reads, as `SELECT * FROM __empty_fixture()`.
pub(crate) const EMPTY_FIXTURE_RELATION: &str = "__empty_fixture";
pub(crate) const PARSE_FAILURE_PREFIX: &str = "Native SQL parser could not analyze this body: ";
pub(crate) const PARSE_FAILURE_REMEDIATION: &str =
    "Use SQL syntax supported by the native parser or report the parser gap.";
pub(crate) const INVALID_DIAGNOSTIC: &str =
    "native lint diagnostic has invalid code, message, or source span";
pub(crate) const INVALID_FIX_REASON: &str =
    "native lint diagnostic has an invalid fix refusal reason";
pub(crate) const INVALID_FIX_OBJECT: &str = "native lint diagnostic fix must be an object";
pub(crate) const INVALID_FIX_EDIT: &str = "native lint diagnostic has an invalid fix edit";
pub(crate) const NOT_AN_OBJECT: &str = "native lint diagnostic must be an object";
pub(crate) const NON_OBJECT_RESPONSE: &str = "native lint engine returned a non-object response";
pub(crate) const UNSUPPORTED_VERSION: &str =
    "native lint engine returned an unsupported response version";
pub(crate) const INVALID_UNAVAILABLE: &str =
    "native lint response has a non-string unavailable reason";
pub(crate) const MISSING_DIAGNOSTICS: &str = "native lint response is missing a diagnostics list";
pub(crate) const INVALID_BATCH: &str = "native lint engine returned an invalid batch response";
pub(crate) const NON_CONTIGUOUS_FIX: &str =
    "fix overlaps generated SQL or a non-contiguous authored region";
pub(crate) const GENERATED_FIX: &str = "fix overlaps generated SQL";
pub(crate) const GENERATED_SQL_SUFFIX: &str = "(in generated SQL)";
pub(crate) const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";
pub const DEFAULT_LINE_WIDTH: i64 = 100;
pub const DEFAULT_MAX_DESCRIPTION_LINES: i64 = 10;
pub(crate) const DEFAULT_MAX_LITERAL_LENGTH: i64 = 100;
pub(crate) const DEFAULT_MAX_RANKING_ORDER_BY: i64 = 5;
pub(crate) const DEFAULT_DIALECT: &str = "generic";
/// Adapter names whose SQL dialect has another name; other adapters name their dialect.
pub(crate) const ADAPTER_DIALECT_TRANSLATIONS: [(&str, &str); 2] =
    [("motherduck", "duckdb"), ("sqlserver", "tsql")];
pub(crate) const MODELS_DIRECTORY: &str = "models";
/// Inherited and local declaration folders, whose files declare several headers.
pub(crate) const SCOPED_DECLARATION_DIRECTORIES: [&str; 6] = [
    "macros",
    "enums",
    "constants",
    "_macros",
    "_enums",
    "_constants",
];
/// `GLOBAL_NAMED_DECLARATION_ROOTS`, in Python's dict order, with their declaration kinds.
pub(crate) const GLOBAL_NAMED_ROOT_KINDS: [(&[&str], &str); 5] = [
    (&["audits", "generic"], "audit"),
    (&["audits", "singular"], "singular_audit"),
    (&["schemas"], "schema"),
    (&["hooks", "sql"], "sql_hook"),
    (&["hooks", "python"], "python_hook"),
];
/// `GROUPED_NAMED_DECLARATION_ROLES`, in Python's dict order, with their declaration kinds.
pub(crate) const GROUPED_NAMED_ROLE_KINDS: [(&[&str], &str); 9] = [
    (&["audits", "generic"], "audit"),
    (&["_audits", "generic"], "audit"),
    (&["audits", "singular"], "singular_audit"),
    (&["schemas"], "schema"),
    (&["_schemas"], "schema"),
    (&["hooks", "sql"], "sql_hook"),
    (&["_hooks", "sql"], "sql_hook"),
    (&["hooks", "python"], "python_hook"),
    (&["_hooks", "python"], "python_hook"),
];
/// The keyword whose presence makes a lint request carry relation names.
pub(crate) const CROSS_KEYWORD: &[u8] = b"cross";
/// What a SQL macro expression body is formatted after, and removed from again.
pub(crate) const EXPRESSION_SELECT_PREFIX: &str = "SELECT ";
/// The output name lint gives an expression macro body, so it is not an unnamed projection.
pub(crate) const LINTED_EXPRESSION_ALIAS: &str = "__sqlbuild_macro_expression__";
/// The characters that open a quoted string or identifier in formatted SQL.
pub(crate) const STRING_QUOTES: [char; 3] = ['\'', '"', '`'];

/// A CTE definition, capturing its `name`.
pub(crate) const CTE_DEFINITION_PATTERN: &str = r#"(?is)(?:\bWITH(?:\s+RECURSIVE)?|,)\s*["`\[]?(?P<name>[A-Za-z_][A-Za-z0-9_]*)["`\]]?(?:\s*\([^)]*\))?\s+AS\s*\("#;
