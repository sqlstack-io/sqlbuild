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

/// The SQLQuack tokenizer a fingerprint dialect reads its tokens with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FingerprintTokenizer {
    /// `sqlquack_syntax`'s lossless tokenizer for one dialect.
    Lossless(Dialect),
    /// The native PostgreSQL scanner of `sqlquack_syntax_postgres`.
    PostgresScanner,
}

/// A fingerprint dialect SQLQuack tokenizes: its tokenizer and its reserved keywords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SqlquackFingerprintDialect {
    pub(crate) name: &'static str,
    pub(crate) tokenizer: FingerprintTokenizer,
    pub(crate) keywords: KeywordList,
}
