//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::ClickHouseDialect;
use crate::analysis_route::types::NativeDialect;
use crate::type_system::models::TypeFamily;

/// Each dialect, by its adapter's analysis dialect name, whose SQL a native module analyses.
pub(crate) const NATIVE_DIALECTS: &[(&str, &dyn NativeDialect)] =
    &[(CLICKHOUSE_DIALECT, &ClickHouseDialect)];
/// The dialect a project with no analysis dialect is named by.
pub(crate) const GENERIC_DIALECT: &str = "generic";
/// The gap of SQL that holds more than one statement.
pub(crate) const MORE_THAN_ONE_STATEMENT: &str = "more than one statement";
/// The gap of SQL that holds no statement.
pub(crate) const NO_STATEMENT: &str = "no statement";
/// The gap of a statement the parser kept as raw text.
pub(crate) const UNPARSED_STATEMENT: &str = "unparsed statement";
/// The warning, or under `require_sql_analysis` the error, for a model analysis skips.
pub(crate) const SKIPPED_ANALYSIS_CODE: &str = "P026";
/// ClickHouse's top-level nullability wrapper, which the native profile spells as a flag.
pub(crate) const NULLABLE_WRAPPER: &str = "Nullable";
/// ClickHouse's top-level dictionary-encoding wrapper, which leaves the column's type as it is.
pub(crate) const LOW_CARDINALITY_WRAPPER: &str = "LowCardinality";
/// The finding of a function the dialect's catalogue does not know.
pub(crate) const UNKNOWN_FUNCTION_CODE: &str = "E202";
/// The severity of every finding the native analysis reports, as the Polyglot path blocks them.
pub(crate) const ERROR_SEVERITY: &str = "error";
/// What `macro_body` puts before a body to read it as the one projection of a query.
pub(crate) const MACRO_BODY_PREFIX: &str = "SELECT ";
/// The gap of a macro body that is not one expression.
pub(crate) const NOT_ONE_EXPRESSION: &str = "not one expression";
/// The prefixes that start a line comment in a native dialect that names no others.
pub(crate) const LINE_COMMENT_PREFIXES: &[&str] = &["--", "//"];
/// ClickHouse's analysis dialect name.
pub(crate) const CLICKHOUSE_DIALECT: &str = "clickhouse";
/// The stub prefix ClickHouse reads as one unquoted name.
pub(crate) const CLICKHOUSE_STUB_PREFIX: &str = "__sqlbuild_";
/// ClickHouse's line comment prefixes: `#` opens one only before a space or `!`.
pub(crate) const CLICKHOUSE_LINE_COMMENT_PREFIXES: &[&str] = &["--", "# ", "#!", "//"];
/// Polyglot's ClickHouse respelling of each SQL type alias (`{}` keeps the arguments) and family.
pub(crate) const CLICKHOUSE_TYPE_ALIASES: &[(&str, &str, Option<TypeFamily>)] = &[
    ("BOOLEAN", "Bool", None),
    ("CHAR", "String", None),
    ("TEXT", "String", None),
    ("VARCHAR", "String", None),
    ("NUMERIC", "Decimal{}", None),
    ("BINARY", "Nullable(BINARY{})", None),
    ("TINYINT", "Nullable(Int8)", Some(TypeFamily::Integer)),
    ("SMALLINT", "Nullable(Int16)", Some(TypeFamily::Integer)),
    ("INT", "Nullable(Int32)", Some(TypeFamily::Integer)),
    ("INTEGER", "Nullable(Int32)", Some(TypeFamily::Integer)),
    ("BIGINT", "Nullable(Int64)", Some(TypeFamily::Integer)),
    ("REAL", "Nullable(Float32){}", Some(TypeFamily::Float)),
    ("FLOAT", "Nullable(Float32){}", Some(TypeFamily::Float)),
    ("DOUBLE", "Nullable(Float64){}", Some(TypeFamily::Float)),
    (
        "DOUBLE PRECISION",
        "Nullable(Float64){}",
        Some(TypeFamily::Float),
    ),
    ("TIMESTAMP", "DateTime{}", Some(TypeFamily::Timestamp)),
];
/// The ClickHouse types whose arguments Polyglot keeps as written, aliases unrenamed.
pub(crate) const CLICKHOUSE_VERBATIM_TYPES: &[&str] = &["LowCardinality", "Tuple"];
