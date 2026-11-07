//! Fingerprint dialects and the function language fingerprinted like a query.

use sqlquack_syntax::tokens::models::Dialect;

use crate::sql_tokens::models::{KeywordList, SqlquackFingerprintDialect};

/// The fingerprint dialect when a project names none.
pub(crate) const GENERIC_FINGERPRINT_DIALECT: &str = "generic";
/// Python's `NATIVE_DIALECT_ALIASES`.
pub(crate) const FINGERPRINT_DIALECT_ALIASES: [(&str, &str); 3] = [
    ("postgres", "postgresql"),
    ("motherduck", "duckdb"),
    ("sqlserver", "tsql"),
];
/// The function language whose definitions are fingerprinted like queries.
pub(crate) const SQL_FUNCTION_LANGUAGE: &str = "sql";
/// Fingerprint dialects SQLQuack tokenizes; the generic stream uses its DuckDB tokenizer.
pub(crate) const SQLQUACK_FINGERPRINT_DIALECTS: [SqlquackFingerprintDialect; 6] = [
    SqlquackFingerprintDialect {
        name: "generic",
        tokenizer: Dialect::DuckDB,
        keywords: KeywordList::Generic,
    },
    SqlquackFingerprintDialect {
        name: "duckdb",
        tokenizer: Dialect::DuckDB,
        keywords: KeywordList::DuckDB,
    },
    SqlquackFingerprintDialect {
        name: "postgres",
        tokenizer: Dialect::PostgreSQL,
        keywords: KeywordList::PostgreSQL,
    },
    SqlquackFingerprintDialect {
        name: "postgresql",
        tokenizer: Dialect::PostgreSQL,
        keywords: KeywordList::PostgreSQL,
    },
    SqlquackFingerprintDialect {
        name: "snowflake",
        tokenizer: Dialect::Snowflake,
        keywords: KeywordList::Snowflake,
    },
    SqlquackFingerprintDialect {
        name: "sqlite",
        tokenizer: Dialect::SQLite,
        keywords: KeywordList::Generic,
    },
];
/// The token that follows a called function's name.
pub(crate) const CALL_OPENER: &str = "(";
/// The token that closes a call's arguments or a column list.
pub(crate) const CALL_CLOSER: &str = ")";
/// Keywords after which a word followed by `(` names a relation and its column list.
pub(crate) const COLUMN_LIST_RELATION_KEYWORDS: [&str; 2] = ["INTO", "TABLE"];
/// The keyword between a CTE's column list and its body.
pub(crate) const CTE_BODY_PREFIX: &str = "AS";
/// The tokens that can follow `AS` after a CTE's column list.
pub(crate) const CTE_BODY_OPENERS: [&str; 3] = ["(", "MATERIALIZED", "NOT"];
/// The prefix of SQLBuild placeholder calls such as `__ref` and `__source`.
pub(crate) const PLACEHOLDER_PREFIX: &str = "__";
