//! The SQLQuack parsers that read warehouse type names.

use sqlquack_syntax::tokens::models::Dialect;

/// Which SQLQuack parser reads a type name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeParser {
    /// `sqlquack-syntax-postgres`'s type grammar.
    Postgres,
    /// A cast target read by `sqlquack-syntax-bigquery`.
    BigQuery,
    /// A cast target read by SQLQuack's core parser for the dialect.
    Core(Dialect),
}
