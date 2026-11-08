//! Why a rule cannot have a SQL syntax tree.

use std::fmt::{self, Display};

/// A dialect SQLQuack does not parse, SQL it rejects, or SQL it read only as opaque text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SqlTreeError {
    /// No SQLQuack parser reads this dialect yet.
    UnavailableDialect(String),
    /// The parser read the SQL but kept part of it as unparsed text or refused its depth.
    Unanalysable(String),
    /// The parser rejected the SQL.
    Parse(String),
    /// The parse worker could not start or panicked.
    Worker(String),
}

impl Display for SqlTreeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnavailableDialect(dialect) => write!(
                formatter,
                "SQL syntax trees are unavailable for the {dialect} dialect: SQLQuack does not parse it yet"
            ),
            Self::Unanalysable(reason) => {
                write!(formatter, "SQL syntax tree is unavailable: {reason}")
            }
            Self::Parse(message) => write!(formatter, "SQL could not be parsed: {message}"),
            Self::Worker(message) => write!(formatter, "SQL syntax tree worker failed: {message}"),
        }
    }
}

impl std::error::Error for SqlTreeError {}
