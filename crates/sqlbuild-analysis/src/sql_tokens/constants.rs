//! Fingerprint dialects and the function language fingerprinted like a query.

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
