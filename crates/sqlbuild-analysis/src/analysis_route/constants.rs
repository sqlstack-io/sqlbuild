//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::TsqlDialect;
use crate::analysis_route::types::NativeDialect;

/// Each dialect, by its adapter's analysis dialect name, whose SQL a native module analyses.
pub(crate) const NATIVE_DIALECTS: &[(&str, &dyn NativeDialect)] = &[(TSQL_DIALECT, &TsqlDialect)];
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
/// SQL Server's analysis dialect name.
pub(crate) const TSQL_DIALECT: &str = "tsql";
/// The stub prefix SQL Server reads as a schema: `__sqlbuild.udf_x(...)`, `__sqlbuild.cursor_start()`.
pub(crate) const TSQL_STUB_PREFIX: &str = "__sqlbuild.";
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
