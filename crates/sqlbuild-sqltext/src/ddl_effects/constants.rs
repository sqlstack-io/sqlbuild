//! Statement keywords that decide how executed SQL affects cached relation metadata.

/// Statements that only read, unless they select into a new table.
pub(crate) const QUERY_STATEMENTS: &[&str] = &["SELECT", "WITH"];
/// Statements that read, change data or session state, but never relation existence or columns.
pub(crate) const READ_OR_DATA_STATEMENTS: &[&str] = &[
    "VALUES",
    "SHOW",
    "DESCRIBE",
    "DESC",
    "EXPLAIN",
    "INSERT",
    "UPDATE",
    "DELETE",
    "MERGE",
    "TRUNCATE",
    "BEGIN",
    "START",
    "SAVEPOINT",
    "RELEASE",
    "SET",
    "UNSET",
    "GRANT",
    "REVOKE",
    "COMMENT",
    "PUT",
    "GET",
    "LIST",
    "LS",
    "REMOVE",
    "RM",
    "ANALYZE",
];
/// Statements that change only the compute later statements run on.
pub(crate) const COMPUTE_ONLY_STATEMENTS: &[&[&str]] = &[&["USE", "WAREHOUSE"]];
/// `ALTER SESSION SET` before the parameter it sets.
pub(crate) const ALTER_SESSION_SET: &[&str] = &["ALTER", "SESSION", "SET"];
/// `ALTER SESSION UNSET` before the parameter it resets.
pub(crate) const ALTER_SESSION_UNSET: &[&str] = &["ALTER", "SESSION", "UNSET"];
/// Session parameters that change neither relations nor how their names resolve.
pub(crate) const NEUTRAL_SESSION_PARAMETERS: &[&[&str]] = &[
    &["QUERY_TAG"],
    &["TIMEZONE"],
    &["STATEMENT_TIMEOUT_IN_SECONDS"],
    &["USE_CACHED_RESULT"],
];
/// `SET` before the setting it changes.
pub(crate) const SET_WORD: &[&str] = &["SET"];
/// Scope modifiers allowed between `SET` and the setting.
pub(crate) const SET_SCOPE_MODIFIERS: &[&[&str]] = &[&["SESSION"], &["LOCAL"], &["GLOBAL"]];
/// Settings, such as DuckDB's and PostgreSQL's, that change how unqualified names resolve.
pub(crate) const NAME_RESOLUTION_SETTINGS: &[&[&str]] = &[&["search_path"], &["schema"]];
/// Statements that end the current transaction.
pub(crate) const TRANSACTION_END_STATEMENTS: &[&str] = &["COMMIT", "ROLLBACK", "END", "ABORT"];
/// Statements whose relation name is read from the statement.
pub(crate) const RELATION_DDL_STATEMENTS: &[&str] = &["CREATE", "DROP", "ALTER", "COPY"];
/// Modifier phrases allowed between `CREATE`/`DROP`/`ALTER` and `TABLE`/`VIEW`.
pub(crate) const RELATION_KIND_MODIFIERS: &[&[&str]] = &[
    &["OR", "REPLACE"],
    &["LOCAL"],
    &["GLOBAL"],
    &["TEMP"],
    &["TEMPORARY"],
    &["TRANSIENT"],
    &["VOLATILE"],
    &["SECURE"],
    &["RECURSIVE"],
    &["MATERIALIZED"],
    &["EXTERNAL"],
    &["DYNAMIC"],
    &["ICEBERG"],
    &["HYBRID"],
    &["UNLOGGED"],
    &["FOREIGN"],
    &["STREAMING"],
];
/// Relation kinds whose name follows the modifiers.
pub(crate) const RELATION_KINDS: &[&[&str]] = &[&["TABLE"], &["VIEW"]];
/// The optional existence guard of `CREATE`.
pub(crate) const IF_NOT_EXISTS: &[&str] = &["IF", "NOT", "EXISTS"];
/// The optional existence guard of `DROP` and `ALTER`.
pub(crate) const IF_EXISTS: &[&str] = &["IF", "EXISTS"];
/// Words that may follow the created relation's name.
pub(crate) const CREATE_FOLLOWING_WORDS: &[&[&str]] = &[
    &["AS"],
    &["CLONE"],
    &["SHALLOW"],
    &["DEEP"],
    &["LIKE"],
    &["COPY"],
    &["USING"],
    &["COMMENT"],
    &["CLUSTER"],
    &["PARTITION"],
    &["OPTIONS"],
    &["WITH"],
    &["TBLPROPERTIES"],
    &["LOCATION"],
    &["DATA_RETENTION_TIME_IN_DAYS"],
    &["CHANGE_TRACKING"],
];
/// The optional trailing word of `DROP`.
pub(crate) const DROP_FOLLOWING_WORDS: &[&[&str]] = &[&["CASCADE"], &["RESTRICT"], &["PURGE"]];
/// Words that may follow the altered relation's name.
pub(crate) const ALTER_FOLLOWING_WORDS: &[&[&str]] = &[
    &["ADD"],
    &["DROP"],
    &["ALTER"],
    &["MODIFY"],
    &["RENAME"],
    &["SWAP"],
    &["SET"],
    &["UNSET"],
    &["CLUSTER"],
    &["RECLUSTER"],
    &["OWNER"],
    &["SUSPEND"],
    &["RESUME"],
    &["REFRESH"],
];
/// Alter clauses that name a second relation.
pub(crate) const RENAMING_ALTERS: &[&[&str]] = &[&["RENAME", "TO"], &["SWAP", "WITH"]];
/// The word that must follow the target of `COPY INTO` and `SELECT ... INTO`.
pub(crate) const FROM_WORD: &[&str] = &["FROM"];
/// `COPY INTO` before the loaded relation.
pub(crate) const COPY_INTO: &[&str] = &["COPY", "INTO"];
/// The `INTO` keyword of `SELECT ... INTO`.
pub(crate) const INTO: &str = "INTO";
/// Statement leads whose object is never a relation.
pub(crate) const NEUTRAL_OBJECT_LEADS: &[&[&str]] = &[&["CREATE"], &["DROP"], &["ALTER"]];
/// The optional replacement phrase of a neutral object statement.
pub(crate) const OR_REPLACE: &[&str] = &["OR", "REPLACE"];
/// Modifiers allowed before a neutral object kind.
pub(crate) const NEUTRAL_OBJECT_MODIFIERS: &[&[&str]] =
    &[&["SECURE"], &["TEMP"], &["TEMPORARY"], &["AGGREGATE"]];
/// Object kinds whose statements never change relation existence or columns.
pub(crate) const NEUTRAL_OBJECT_KINDS: &[&[&str]] = &[
    &["FUNCTION"],
    &["PROCEDURE"],
    &["SEQUENCE"],
    &["STAGE"],
    &["FILE", "FORMAT"],
    &["INDEX"],
    &["UNIQUE", "INDEX"],
    &["WAREHOUSE"],
    &["MACRO"],
];
/// `CREATE SCHEMA IF NOT EXISTS` before the schema name.
pub(crate) const CREATE_SCHEMA_IF_NOT_EXISTS: &[&str] =
    &["CREATE", "SCHEMA", "IF", "NOT", "EXISTS"];
/// The statement separator.
pub(crate) const STATEMENT_SEPARATOR: char = ';';
/// The shortest name that can be wrapped in identifier quotes.
pub(crate) const QUOTED_IDENTIFIER_MIN_LENGTH: usize = 2;
