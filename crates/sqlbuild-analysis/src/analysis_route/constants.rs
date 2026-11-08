//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::DatabricksDialect;
use crate::analysis_route::types::NativeDialect;

/// Each dialect, by its adapter's analysis dialect name, whose SQL a native module analyses.
pub(crate) const NATIVE_DIALECTS: &[(&str, &dyn NativeDialect)] =
    &[(DATABRICKS_DIALECT, &DatabricksDialect)];
/// Databricks' analysis dialect name.
pub(crate) const DATABRICKS_DIALECT: &str = "databricks";
/// The stub prefix Databricks reads as one unquoted name.
pub(crate) const DATABRICKS_STUB_PREFIX: &str = "__sqlbuild_";
/// The construct of a byte the native scanner reads as no token, blank or comment.
pub(crate) const UNSCANNED_CONSTRUCT: &str = "unscanned text";
/// Databricks type words Polyglot spells another way, so the route's text fallback matches it.
/// The space in DOUBLE PRECISION is taken out before the fallback compares names.
pub(crate) const DATABRICKS_TYPE_ALIASES: &[(&str, &str)] = &[
    ("INTEGER", "INT"),
    ("REAL", "FLOAT"),
    ("DOUBLE PRECISION", "DOUBLE"),
    ("NUMERIC", "DECIMAL"),
    ("TIMESTAMP_LTZ", "TIMESTAMPLTZ"),
    ("TIMESTAMP_NTZ", "TIMESTAMPNTZ"),
];
/// The construct of a statement a parser kept as raw text.
pub(crate) const UNPARSED_CONSTRUCT: &str = "unparsed statement";
/// The construct of SQL holding other than one statement.
pub(crate) const STATEMENTS_CONSTRUCT: &str = "more than one statement";
/// The warning, or under `require_sql_analysis` the error, for a model analysis skips.
pub(crate) const SKIPPED_ANALYSIS_CODE: &str = "P026";
/// ClickHouse's top-level nullability wrapper, which the native profile spells as a flag.
pub(crate) const NULLABLE_WRAPPER: &str = "Nullable";
/// ClickHouse's top-level dictionary-encoding wrapper, which leaves the column's type as it is.
pub(crate) const LOW_CARDINALITY_WRAPPER: &str = "LowCardinality";
