//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::ClickHouseDialect;
use crate::analysis_route::types::NativeDialect;

/// Each dialect, by its adapter's analysis dialect name, whose SQL a native module analyses.
pub(crate) const NATIVE_DIALECTS: &[(&str, &dyn NativeDialect)] =
    &[(CLICKHOUSE_DIALECT, &ClickHouseDialect)];
/// The warning, or under `require_sql_analysis` the error, for a model analysis skips.
pub(crate) const SKIPPED_ANALYSIS_CODE: &str = "P026";
/// ClickHouse's top-level nullability wrapper, which the native profile spells as a flag.
pub(crate) const NULLABLE_WRAPPER: &str = "Nullable";
/// ClickHouse's top-level dictionary-encoding wrapper, which leaves the column's type as it is.
pub(crate) const LOW_CARDINALITY_WRAPPER: &str = "LowCardinality";
/// ClickHouse's analysis dialect name.
pub(crate) const CLICKHOUSE_DIALECT: &str = "clickhouse";
/// The stub prefix ClickHouse reads as one unquoted name.
pub(crate) const CLICKHOUSE_STUB_PREFIX: &str = "__sqlbuild_";
/// The construct of SQL holding more than one statement.
pub(crate) const STATEMENTS_CONSTRUCT: &str = "more than one statement";
/// Each SQL type alias whose name Polyglot respells for ClickHouse, case-insensitive, with the
/// spelling SQLBuild's type fallback reads as Polyglot's canonical name; `{}` keeps the arguments.
pub(crate) const CLICKHOUSE_TYPE_ALIASES: &[(&str, &str)] = &[
    ("BOOLEAN", "Bool"),
    ("CHAR", "String"),
    ("TEXT", "String"),
    ("VARCHAR", "String"),
    ("NUMERIC", "Decimal{}"),
    ("BINARY", "Nullable(BINARY{})"),
    ("TINYINT", "Nullable(Int8)"),
    ("SMALLINT", "Nullable(Int16)"),
    ("INT", "Nullable(Int32)"),
    ("INTEGER", "Nullable(Int32)"),
    ("BIGINT", "Nullable(Int64)"),
    ("REAL", "Nullable(Float32){}"),
    ("FLOAT", "Nullable(Float32){}"),
    ("DOUBLE", "Nullable(Float64){}"),
    ("DOUBLE PRECISION", "Nullable(Float64){}"),
    ("TIMESTAMP", "DateTime{}"),
];
/// The ClickHouse types whose arguments Polyglot keeps as written, aliases unrenamed.
pub(crate) const CLICKHOUSE_VERBATIM_TYPES: &[&str] = &["LowCardinality", "Tuple"];
