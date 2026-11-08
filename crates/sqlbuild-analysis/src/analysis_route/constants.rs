//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::ClickHouseDialect;
use crate::analysis_route::types::NativeDialect;
use crate::type_system::models::TypeFamily;

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
