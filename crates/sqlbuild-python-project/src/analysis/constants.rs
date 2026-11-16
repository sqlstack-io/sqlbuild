//! Public constants of analysis.

/// Dialects whose quoted identifiers bind case-sensitively unless the session says otherwise.
pub const CASE_SENSITIVE_BINDING_DIALECTS: [&str; 2] = ["snowflake", "postgresql"];

/// Adapter dialect names and the native dialect each validates as.
pub const NATIVE_DIALECT_ALIASES: [(&str, &str); 3] = [
    ("postgres", "postgresql"),
    ("motherduck", "duckdb"),
    ("sqlserver", "tsql"),
];
