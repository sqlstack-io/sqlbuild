//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::BigQueryDialect;
use crate::analysis_route::types::NativeDialect;

/// Each dialect, by its adapter's analysis dialect name, whose SQL a native module analyses.
pub(crate) const NATIVE_DIALECTS: &[(&str, &dyn NativeDialect)] =
    &[(BIGQUERY_DIALECT, &BigQueryDialect)];
/// BigQuery's analysis dialect name.
pub(crate) const BIGQUERY_DIALECT: &str = "bigquery";
/// The stub prefix BigQuery reads as one unquoted name.
pub(crate) const BIGQUERY_STUB_PREFIX: &str = "__sqlbuild_";
/// The construct of a byte the BigQuery scanner reads as no token, blank or comment.
pub(crate) const UNSCANNED_CONSTRUCT: &str = "unscanned text";
/// The query a type spelling completes, with `)`, so BigQuery's parser reads it as a cast target.
pub(crate) const CAST_TYPE_PREFIX: &str = "SELECT CAST(NULL AS ";
/// The name of BigQuery's RANGE type.
pub(crate) const RANGE_TYPE_NAME: &str = "RANGE";
/// The element types of a BigQuery RANGE.
pub(crate) const RANGE_ELEMENT_TYPES: &[&str] = &["DATE", "DATETIME", "TIMESTAMP"];
/// The base type parameters are written in.
pub(crate) const DECIMAL_RADIX: u32 = 10;
/// NUMERIC's largest scale and its most digits before the point.
pub(crate) const NUMERIC_DIGITS: (u32, u32) = (9, 29);
/// BIGNUMERIC's largest scale and its most digits before the point.
pub(crate) const BIGNUMERIC_DIGITS: (u32, u32) = (38, 38);
/// Polyglot's BigQuery names for a whole type's scalar spellings, which SQLBuild compares (D460).
pub(crate) const POLYGLOT_TOP_LEVEL_TYPE_NAMES: &[(&str, &str)] = &[
    ("BIGDECIMAL", "NUMERIC"),
    ("BIGINT", "INT64"),
    ("BOOL", "BOOLEAN"),
    ("DECIMAL", "NUMERIC"),
    ("INT", "INT64"),
    ("INTEGER", "INT64"),
    ("SMALLINT", "INT64"),
    ("TIMESTAMP", "DATETIME"),
];
/// Polyglot's BigQuery names for scalar spellings inside ARRAY and STRUCT.
pub(crate) const POLYGLOT_NESTED_TYPE_NAMES: &[(&str, &str)] = &[
    ("BOOL", "BOOLEAN"),
    ("DECIMAL", "NUMERIC"),
    ("INT", "INT64"),
    ("INTEGER", "INT64"),
    ("SMALLINT", "INT64"),
    ("TIMESTAMP", "DATETIME"),
];
/// The whole types whose MAX parameter Polyglot drops; BYTES and nested types keep it.
pub(crate) const POLYGLOT_MAX_DROPPED: &[&str] = &["BIGDECIMAL", "BIGNUMERIC", "STRING"];
/// The construct of SQL holding more than one statement.
pub(crate) const STATEMENTS_CONSTRUCT: &str = "more than one statement";
/// The warning, or under `require_sql_analysis` the error, for a model analysis skips.
pub(crate) const SKIPPED_ANALYSIS_CODE: &str = "P026";
/// ClickHouse's top-level nullability wrapper, which the native profile spells as a flag.
pub(crate) const NULLABLE_WRAPPER: &str = "Nullable";
/// ClickHouse's top-level dictionary-encoding wrapper, which leaves the column's type as it is.
pub(crate) const LOW_CARDINALITY_WRAPPER: &str = "LowCardinality";
