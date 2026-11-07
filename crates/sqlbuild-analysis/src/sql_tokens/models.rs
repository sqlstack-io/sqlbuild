//! How a fingerprint dialect is tokenized and which of its words fold.

use sqlquack_syntax::tokens::models::Dialect;

/// The reserved-keyword list a fingerprint dialect folds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeywordList {
    DuckDB,
    PostgreSQL,
    Snowflake,
    Generic,
}

/// A fingerprint dialect SQLQuack tokenizes: its tokenizer and its reserved keywords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SqlquackFingerprintDialect {
    pub(crate) name: &'static str,
    pub(crate) tokenizer: Dialect,
    pub(crate) keywords: KeywordList,
}
