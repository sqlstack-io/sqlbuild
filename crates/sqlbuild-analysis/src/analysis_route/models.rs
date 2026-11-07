//! The route a dialect's analysis takes, and what stops a native parser reading SQL whole.

use crate::analysis_route::types::NativeDialect;

/// The engine that analyses a dialect's SQL.
#[derive(Clone, Copy)]
pub enum AnalysisRoute {
    /// Polyglot, as before the dialect's native module.
    Legacy,
    /// The dialect's native module; a model it cannot read whole is not analysed.
    Native(&'static dyn NativeDialect),
}

/// The first construct a native parser cannot read, and its byte offset in the SQL it read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParserGap {
    /// The construct, such as `PIVOT`, or the parser's error message.
    pub construct: String,
    /// The parser's message as the engine words it.
    pub message: String,
    pub offset: usize,
    /// True when the parser reports invalid syntax; false for an unsupported construct or a limit.
    pub syntax: bool,
}

/// A gap with the line and column, from 1, of its offset in the analysed SQL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocatedGap {
    pub construct: String,
    pub line: usize,
    pub column: usize,
}

/// How an inferred column type meets its declared type: the type itself, then its nullability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeMatch {
    pub same_type: bool,
    /// The inferred column may be null, but its declared type does not allow it.
    pub nullable_into_non_null: bool,
}

/// BigQuery, whose SQL SQLQuack's GoogleSQL parser reads.
#[derive(Clone, Copy, Debug)]
pub struct BigQueryDialect;
