//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::TsqlDialect;
use crate::analysis_route::types::NativeDialect;

/// Each dialect, by its adapter's analysis dialect name, whose SQL a native module analyses.
pub(crate) const NATIVE_DIALECTS: &[(&str, &dyn NativeDialect)] = &[(TSQL_DIALECT, &TsqlDialect)];
/// SQL Server's analysis dialect name.
pub(crate) const TSQL_DIALECT: &str = "tsql";
/// The stub prefix SQL Server reads as a schema: `__sqlbuild.udf_x(...)`, `__sqlbuild.cursor_start()`.
pub(crate) const TSQL_STUB_PREFIX: &str = "__sqlbuild.";
/// The construct of SQL holding more than one statement.
pub(crate) const STATEMENTS_CONSTRUCT: &str = "more than one statement";
/// SQL Server 2022's built-in type names (`sys.types` where `is_user_defined = 0`, 16.0.4305.5).
pub(crate) const TSQL_BUILT_IN_TYPES: [&str; 34] = [
    "BIGINT",
    "BINARY",
    "BIT",
    "CHAR",
    "DATE",
    "DATETIME",
    "DATETIME2",
    "DATETIMEOFFSET",
    "DECIMAL",
    "FLOAT",
    "GEOGRAPHY",
    "GEOMETRY",
    "HIERARCHYID",
    "IMAGE",
    "INT",
    "MONEY",
    "NCHAR",
    "NTEXT",
    "NUMERIC",
    "NVARCHAR",
    "REAL",
    "SMALLDATETIME",
    "SMALLINT",
    "SMALLMONEY",
    "SQL_VARIANT",
    "SYSNAME",
    "TEXT",
    "TIME",
    "TIMESTAMP",
    "TINYINT",
    "UNIQUEIDENTIFIER",
    "VARBINARY",
    "VARCHAR",
    "XML",
];
/// Built-in type synonyms `sys.types` does not list, which SQL Server 16.0.4305.5 accepts.
pub(crate) const TSQL_TYPE_SYNONYMS: [&str; 1] = ["ROWVERSION"];
/// SQLBuild's logical types SQL Server's parser refuses, which its adapter writes as built-in ones.
pub(crate) const LOGICAL_TYPES: [&str; 4] = ["BOOLEAN", "DOUBLE", "JSON", "STRING"];
/// Polyglot's T-SQL canonical name of each alias the text fallback keeps as written.
pub(crate) const TSQL_TYPE_ALIASES: [(&str, &str); 8] = [
    ("INT", "INTEGER"),
    ("NUMERIC", "DECIMAL"),
    ("DOUBLE", "FLOAT"),
    ("DOUBLE PRECISION", "FLOAT"),
    ("JSON", "NVARCHAR(MAX)"),
    ("CHARACTER VARYING", "VARCHAR"),
    ("NATIONAL CHARACTER VARYING", "VARCHAR"),
    ("NATIONAL CHAR", "NCHAR"),
];
/// The warning, or under `require_sql_analysis` the error, for a model analysis skips.
pub(crate) const SKIPPED_ANALYSIS_CODE: &str = "P026";
/// ClickHouse's top-level nullability wrapper, which the native profile spells as a flag.
pub(crate) const NULLABLE_WRAPPER: &str = "Nullable";
/// ClickHouse's top-level dictionary-encoding wrapper, which leaves the column's type as it is.
pub(crate) const LOW_CARDINALITY_WRAPPER: &str = "LowCardinality";
