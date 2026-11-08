//! The route a dialect's analysis takes, and what stops a native parser reading SQL whole.

use crate::analysis_route::types::NativeDialect;

/// The engine that analyses a dialect's SQL.
#[derive(Clone, Copy)]
pub enum AnalysisRoute {
    /// Polyglot, as before the dialect's native module.
    Legacy,
    /// The dialect's native module; a model it cannot read whole is not analysed.
    Native(&'static dyn NativeDialect),
    /// No native module reads the dialect: nothing is analysed, and no Polyglot path stands in.
    Unavailable,
}

/// The first construct or error that stops a native parser, at its byte offset in the SQL it read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParserGap {
    /// The construct, such as `PIVOT`, or the parser's error message.
    pub construct: String,
    /// The parser's bare message, without the line and column an error's `Display` adds.
    pub message: String,
    pub offset: usize,
    /// Whether the parser reports invalid syntax; false for an unsupported construct or a limit.
    pub syntax: bool,
}

/// A gap with the line and column, from 1, of its offset in the analysed SQL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocatedGap {
    pub construct: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub syntax: bool,
}

/// How an inferred column type meets its declared type: the type itself, then its nullability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeMatch {
    /// One side is a type the dialect cannot spell or refuses: no finding and no match.
    NotCompared,
    Compared {
        same_type: bool,
        /// The inferred column may be null, but its declared type does not allow it.
        nullable_into_non_null: bool,
    },
}
