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

/// The first construct or error that stops a native parser, and its byte offset in the SQL it read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParserGap {
    /// The construct, such as `PIVOT`, or the parser's error message.
    pub construct: String,
    /// The parser's message as the engine words it.
    pub message: String,
    pub offset: usize,
    /// Whether the parser reports invalid syntax, rather than an unsupported construct or a limit.
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

/// SQL Server, whose SQL SQLQuack's T-SQL parser reads.
#[derive(Clone, Copy, Debug)]
pub struct TsqlDialect;

/// DuckDB and MotherDuck, whose type spellings the native route compares as Polyglot did.
#[derive(Clone, Copy, Debug)]
pub struct DuckDbDialect;

/// Snowflake, whose SQL SQLQuack's Snowflake parser reads.
#[derive(Clone, Copy, Debug)]
pub struct SnowflakeDialect;

/// SQLite, whose SQL SQLQuack's native SQLite parser reads.
#[derive(Clone, Copy, Debug)]
pub struct SqliteDialect;

/// How a dialect's type names are respelt before SQLBuild's type fallback reads them.
#[derive(Clone, Copy, Debug)]
pub struct TypeSpelling {
    /// Each name, upper-cased and single-spaced, and the name it is compared as.
    pub aliases: &'static [(&'static str, &'static str)],
    /// Further aliases that apply only inside another type, such as a list's element.
    pub nested_aliases: &'static [(&'static str, &'static str)],
    /// Names whose parameters are dropped when they are renamed.
    pub sizeless: &'static [&'static str],
    /// Names whose parameterised spelling, as a whole type, the fallback cannot spell as compared.
    pub sizeless_whole: &'static [&'static str],
    /// Names kept as written when they have no parameters.
    pub bare_kept: &'static [&'static str],
    /// Types whose parameters are named fields.
    pub fielded: &'static [&'static str],
    /// Types whose parameters are compared as written.
    pub verbatim: &'static [&'static str],
    /// Whether `[]` array suffixes are part of the dialect's type syntax.
    pub arrays: bool,
}
