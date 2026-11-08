//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::{
    BigQueryDialect, ClickHouseDialect, DatabricksDialect, DuckDbDialect, PostgresDialect,
    SnowflakeDialect, SqliteDialect, TsqlDialect, TypeSpelling,
};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::models::TypeFamily;

/// Each dialect, by its adapter's analysis dialect name, whose SQL a native module analyses.
pub(crate) const NATIVE_DIALECTS: &[(&str, &dyn NativeDialect)] = &[
    (BIGQUERY_DIALECT, &BigQueryDialect),
    (CLICKHOUSE_DIALECT, &ClickHouseDialect),
    (DATABRICKS_DIALECT, &DatabricksDialect),
    (DUCKDB_DIALECT, &DuckDbDialect),
    (POSTGRES_DIALECT, &PostgresDialect),
    (SNOWFLAKE_DIALECT, &SnowflakeDialect),
    (SQLITE_DIALECT, &SqliteDialect),
    (TSQL_DIALECT, &TsqlDialect),
];
/// BigQuery's analysis dialect name.
pub(crate) const BIGQUERY_DIALECT: &str = "bigquery";
/// ClickHouse's analysis dialect name.
pub(crate) const CLICKHOUSE_DIALECT: &str = "clickhouse";
/// Databricks' analysis dialect name.
pub(crate) const DATABRICKS_DIALECT: &str = "databricks";
/// DuckDB's (and MotherDuck's) analysis dialect name.
pub(crate) const DUCKDB_DIALECT: &str = "duckdb";
/// PostgreSQL's analysis dialect name.
pub(crate) const POSTGRES_DIALECT: &str = "postgres";
/// Snowflake's analysis dialect name.
pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
/// SQLite's analysis dialect name.
pub(crate) const SQLITE_DIALECT: &str = "sqlite";
/// The stub prefix BigQuery, ClickHouse, Databricks, PostgreSQL, Snowflake and SQLite read as one unquoted name.
pub(crate) const UNQUOTED_STUB_PREFIX: &str = "__sqlbuild_";
/// The word that makes SQLBuild's type fallback call a type it cannot split into name and flat parameters a timestamp.
pub(crate) const FALLBACK_TIMESTAMP_WORD: &str = "TIMESTAMP";
/// The most words a type name spans, as in `TIMESTAMP WITH LOCAL TIME ZONE`.
pub(crate) const MAX_TYPE_NAME_WORDS: usize = 5;
/// `format_type`'s time and timestamp names, `(name, zone words, respelt)`, around any precision.
pub(crate) const POSTGRES_TEMPORAL_SPELLINGS: &[(&str, &str, &str)] = &[
    ("timestamp", " without time zone", "TIMESTAMP"),
    ("timestamp", " with time zone", "TIMESTAMPTZ"),
    ("time", " without time zone", "TIME"),
    ("time", " with time zone", "TIMETZ"),
];
/// `format_type`'s other multi-word names, as SQLBuild's type fallback spells them.
pub(crate) const POSTGRES_TYPE_SPELLINGS: &[(&str, &str)] = &[
    ("character varying", "VARCHAR"),
    ("character", "CHAR"),
    ("double precision", "DOUBLE"),
    ("bit varying", "VARBIT"),
];
/// The construct of a byte the scanner reads as no token, blank or comment.
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
/// DuckDB's type names as SQLBuild compares them.
pub(crate) const DUCKDB_TYPE_SPELLING: TypeSpelling = TypeSpelling {
    aliases: &[
        ("BINARY", "BLOB"),
        ("BLOB", "VARBINARY"),
        ("BOOL", "BOOLEAN"),
        ("BYTEA", "BLOB"),
        ("CHAR", "TEXT"),
        ("CHAR VARYING", "TEXT"),
        ("CHARACTER", "TEXT"),
        ("CHARACTER VARYING", "TEXT"),
        ("DOUBLE PRECISION", "DOUBLE"),
        ("HUGEINT", "INT128"),
        ("INT64", "BIGINT"),
        ("INTEGER", "INT"),
        ("NCHAR VARYING", "TEXT"),
        ("NTEXT", "TEXT"),
        ("NUMERIC", "DECIMAL"),
        ("ROW", "STRUCT"),
        ("STRING", "TEXT"),
        ("TIME WITH TIME ZONE", "TIMETZ"),
        ("TIME WITHOUT TIME ZONE", "TIME"),
        ("TIMESTAMP WITH LOCAL TIME ZONE", "TIMESTAMPLTZ"),
        ("TIMESTAMP WITH TIME ZONE", "TIMESTAMPTZ"),
        ("TIMESTAMP WITHOUT TIME ZONE", "TIMESTAMP"),
        ("TIMESTAMP_LTZ", "TIMESTAMPLTZ"),
        ("TIMESTAMP_NTZ", "TIMESTAMPNTZ"),
        ("TIMESTAMP_TZ", "TIMESTAMPTZ"),
        ("UHUGEINT", "UINT128"),
        ("UINT16", "USMALLINT"),
        ("UINT32", "UINTEGER"),
        ("UINT64", "UBIGINT"),
        ("UINT8", "UTINYINT"),
        ("VARBINARY", "BLOB"),
        ("VARCHAR", "TEXT"),
    ],
    nested_aliases: &[("DATETIME", "TIMESTAMP"), ("NVARCHAR", "TEXT")],
    sizeless: &["CHAR", "CHARACTER"],
    sizeless_whole: &["CHAR", "CHARACTER", "TIMESTAMP"],
    bare_kept: &["ROW"],
    fielded: &["STRUCT", "UNION"],
    verbatim: &["LIST"],
    arrays: true,
};
/// Snowflake's type names as SQLBuild compares them.
pub(crate) const SNOWFLAKE_TYPE_SPELLING: TypeSpelling = TypeSpelling {
    aliases: &[
        ("BINARY VARYING", "VARBINARY"),
        ("BOOL", "BOOLEAN"),
        ("BYTEA", "VARBINARY"),
        ("CHAR VARYING", "VARCHAR"),
        ("CHARACTER", "CHAR"),
        ("CHARACTER VARYING", "VARCHAR"),
        ("DOUBLE PRECISION", "DOUBLE"),
        ("HUGEINT", "INT128"),
        ("INTEGER", "INT"),
        ("JSON", "VARIANT"),
        ("NCHAR VARYING", "VARCHAR"),
        ("NTEXT", "VARCHAR"),
        ("NUMERIC", "DECIMAL"),
        ("REAL", "FLOAT"),
        ("TEXT", "VARCHAR"),
        ("TIME WITH TIME ZONE", "TIMEWITHTIMEZONE"),
        ("TIME WITHOUT TIME ZONE", "TIME"),
        ("TIMESTAMP WITH LOCAL TIME ZONE", "TIMESTAMPLTZ"),
        ("TIMESTAMP WITH TIME ZONE", "TIMESTAMPTZ"),
        ("TIMESTAMP WITHOUT TIME ZONE", "TIMESTAMPNTZ"),
        ("TIMESTAMP_LTZ", "TIMESTAMPLTZ"),
        ("TIMESTAMP_NTZ", "TIMESTAMPNTZ"),
        ("TIMESTAMP_TZ", "TIMESTAMPTZ"),
        ("TIMETZ", "TIMEWITHTIMEZONE"),
        ("UHUGEINT", "UINT128"),
        ("UINT16", "USMALLINT"),
        ("UINT32", "UINTEGER"),
        ("UINT64", "UBIGINT"),
        ("UINT8", "UTINYINT"),
    ],
    nested_aliases: &[],
    sizeless: &[],
    sizeless_whole: &["TIMESTAMP", "TIMESTAMP_TZ", "TIMESTAMPTZ"],
    bare_kept: &[],
    fielded: &["OBJECT"],
    verbatim: &[],
    arrays: false,
};
/// SQLite's type names as SQLBuild compares them.
pub(crate) const SQLITE_TYPE_SPELLING: TypeSpelling = TypeSpelling {
    aliases: &[
        ("BINARY", "BLOB"),
        ("BOOL", "BOOLEAN"),
        ("BYTEA", "BLOB"),
        ("CHAR", "TEXT"),
        ("CHAR VARYING", "TEXT"),
        ("CHARACTER", "TEXT"),
        ("CHARACTER VARYING", "TEXT"),
        ("DOUBLE", "REAL"),
        ("DOUBLE PRECISION", "REAL"),
        ("HUGEINT", "INT128"),
        ("INT", "INTEGER"),
        ("INT64", "BIGINT"),
        ("NCHAR VARYING", "TEXT"),
        ("NTEXT", "TEXT"),
        ("NUMERIC", "DECIMAL"),
        ("SMALLINT", "INTEGER"),
        ("TIME WITH TIME ZONE", "TIMEWITHTIMEZONE"),
        ("TIME WITHOUT TIME ZONE", "TIME"),
        ("TIMESTAMP WITH LOCAL TIME ZONE", "TIMESTAMPLTZ"),
        ("TIMESTAMP WITH TIME ZONE", "TIMESTAMPTZ"),
        ("TIMESTAMP WITHOUT TIME ZONE", "TIMESTAMP"),
        ("TIMESTAMP_LTZ", "TIMESTAMPLTZ"),
        ("TIMESTAMP_NTZ", "TIMESTAMPNTZ"),
        ("TIMESTAMP_TZ", "TIMESTAMPTZ"),
        ("TIMETZ", "TIMEWITHTIMEZONE"),
        ("UHUGEINT", "UINT128"),
        ("UINT16", "USMALLINT"),
        ("UINT32", "UINTEGER"),
        ("UINT64", "UBIGINT"),
        ("UINT8", "UTINYINT"),
        ("VARBINARY", "BLOB"),
        ("VARCHAR", "TEXT"),
    ],
    nested_aliases: &[],
    sizeless: &["CHAR", "CHARACTER"],
    sizeless_whole: &[
        "BIGINT",
        "CHAR",
        "CHARACTER",
        "DOUBLE",
        "DOUBLE PRECISION",
        "FLOAT",
        "INT",
        "INTEGER",
        "MEDIUMINT",
        "REAL",
        "SMALLINT",
        "TINYINT",
    ],
    bare_kept: &[],
    fielded: &[],
    verbatim: &[],
    arrays: false,
};
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
