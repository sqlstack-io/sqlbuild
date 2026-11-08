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
/// The query a type spelling completes, with `)`, so Databricks' parser reads it as a cast target.
pub(crate) const DATABRICKS_CAST_TYPE_PREFIX: &str = "SELECT CAST(NULL AS ";
/// DECIMAL's precision and scale where Databricks' spelling leaves them out.
pub(crate) const DATABRICKS_DEFAULT_DECIMAL: (u32, u32) = (10, 0);
/// The type Databricks gives every string expression, a cast to CHAR(n) or VARCHAR(n) included.
pub(crate) const DATABRICKS_STRING_TYPE: &str = "STRING";
/// Databricks' one boolean type.
pub(crate) const DATABRICKS_BOOLEAN_TYPE: &str = "BOOLEAN";
/// Databricks' one date type.
pub(crate) const DATABRICKS_DATE_TYPE: &str = "DATE";
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
/// The finding of a column no referenced relation has.
pub(crate) const UNKNOWN_COLUMN_CODE: &str = "E201";
/// The severity of every finding the native analysis reports, as the Polyglot path blocks them.
pub(crate) const ERROR_SEVERITY: &str = "error";
/// What `macro_body` puts before a body to read it as the one projection of a query.
pub(crate) const MACRO_BODY_PREFIX: &str = "SELECT ";
/// The gap of a macro body that is not one expression.
pub(crate) const NOT_ONE_EXPRESSION: &str = "not one expression";
