//! Shapes of local scenario SQL conversion results.

/// The DuckDB SQL or None, the result columns, and the refusal text, as the Python binding returns them.
pub type LocalSqlParts = (Option<String>, Vec<(String, String)>, String);
