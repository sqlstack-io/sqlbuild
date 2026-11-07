//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::{SnowflakeDialect, SqliteDialect};
use crate::analysis_route::types::NativeDialect;

/// Each dialect, by its adapter's analysis dialect name, whose SQL a native module analyses.
pub(crate) const NATIVE_DIALECTS: &[(&str, &dyn NativeDialect)] = &[
    (SNOWFLAKE_DIALECT, &SnowflakeDialect),
    (SQLITE_DIALECT, &SqliteDialect),
];
/// Snowflake's analysis dialect name.
pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
/// SQLite's analysis dialect name.
pub(crate) const SQLITE_DIALECT: &str = "sqlite";
/// The stub prefix Snowflake and SQLite read as one unquoted name.
pub(crate) const UNQUOTED_STUB_PREFIX: &str = "__sqlbuild_";
/// The construct of a byte the scanner reads as no token, blank or comment.
pub(crate) const UNSCANNED_CONSTRUCT: &str = "unscanned text";
/// The construct of SQL holding more than one statement, or none.
pub(crate) const STATEMENTS_CONSTRUCT: &str = "more than one statement";
/// The construct of a statement the parser kept as unparsed text.
pub(crate) const COMMAND_CONSTRUCT: &str = "unparsed statement";
/// The warning, or under `require_sql_analysis` the error, for a model analysis skips.
pub(crate) const SKIPPED_ANALYSIS_CODE: &str = "P026";
/// ClickHouse's top-level nullability wrapper, which the native profile spells as a flag.
pub(crate) const NULLABLE_WRAPPER: &str = "Nullable";
/// ClickHouse's top-level dictionary-encoding wrapper, which leaves the column's type as it is.
pub(crate) const LOW_CARDINALITY_WRAPPER: &str = "LowCardinality";
