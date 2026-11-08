//! The native dialects and the diagnostic a model the native parser cannot read takes.

use crate::analysis_route::models::{DuckDbDialect, SnowflakeDialect, SqliteDialect, TypeSpelling};
use crate::analysis_route::types::NativeDialect;

/// Each dialect, by its adapter's analysis dialect name, whose SQL a native module analyses.
pub(crate) const NATIVE_DIALECTS: &[(&str, &dyn NativeDialect)] = &[
    (DUCKDB_DIALECT, &DuckDbDialect),
    (SNOWFLAKE_DIALECT, &SnowflakeDialect),
    (SQLITE_DIALECT, &SqliteDialect),
];
/// DuckDB's (and MotherDuck's) analysis dialect name.
pub(crate) const DUCKDB_DIALECT: &str = "duckdb";
/// Snowflake's analysis dialect name.
pub(crate) const SNOWFLAKE_DIALECT: &str = "snowflake";
/// SQLite's analysis dialect name.
pub(crate) const SQLITE_DIALECT: &str = "sqlite";
/// The stub prefix Snowflake and SQLite read as one unquoted name.
pub(crate) const UNQUOTED_STUB_PREFIX: &str = "__sqlbuild_";
/// The word that makes SQLBuild's type fallback call a type it cannot split into name and flat parameters a timestamp.
pub(crate) const FALLBACK_TIMESTAMP_WORD: &str = "TIMESTAMP";
/// The most words a type name spans, as in `TIMESTAMP WITH TIME ZONE`.
pub(crate) const MAX_TYPE_NAME_WORDS: usize = 4;
/// The construct of a byte the scanner reads as no token, blank or comment.
pub(crate) const UNSCANNED_CONSTRUCT: &str = "unscanned text";
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
/// DuckDB's type names as SQLBuild compares them.
pub(crate) const DUCKDB_TYPE_SPELLING: TypeSpelling = TypeSpelling {
    aliases: &[
        ("BINARY", "BLOB"),
        ("BLOB", "VARBINARY"),
        ("BOOL", "BOOLEAN"),
        ("BYTEA", "BLOB"),
        ("CHAR", "TEXT"),
        ("CHARACTER", "TEXT"),
        ("CHARACTER VARYING", "TEXT"),
        ("DOUBLE PRECISION", "DOUBLE"),
        ("HUGEINT", "INT128"),
        ("INT64", "BIGINT"),
        ("INTEGER", "INT"),
        ("NUMERIC", "DECIMAL"),
        ("ROW", "STRUCT"),
        ("STRING", "TEXT"),
        ("TIME WITH TIME ZONE", "TIMETZ"),
        ("TIMESTAMP WITH TIME ZONE", "TIMESTAMPTZ"),
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
        ("CHAR VARYING", "VARCHAR"),
        ("CHARACTER", "CHAR"),
        ("CHARACTER VARYING", "VARCHAR"),
        ("DOUBLE PRECISION", "DOUBLE"),
        ("INTEGER", "INT"),
        ("NCHAR VARYING", "VARCHAR"),
        ("NUMERIC", "DECIMAL"),
        ("REAL", "FLOAT"),
        ("TEXT", "VARCHAR"),
        ("TIMESTAMP_LTZ", "TIMESTAMPLTZ"),
        ("TIMESTAMP_NTZ", "TIMESTAMPNTZ"),
        ("TIMESTAMP_TZ", "TIMESTAMPTZ"),
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
        ("BOOL", "BOOLEAN"),
        ("CHAR", "TEXT"),
        ("CHARACTER", "TEXT"),
        ("DOUBLE", "REAL"),
        ("DOUBLE PRECISION", "REAL"),
        ("INT", "INTEGER"),
        ("NUMERIC", "DECIMAL"),
        ("SMALLINT", "INTEGER"),
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
