//! What a dialect's native analysis module provides.

use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::Parsed;

use crate::analysis_route::models::ParserGap;

/// A dialect whose SQL SQLQuack analyses in place of Polyglot.
pub trait NativeDialect: Sync {
    /// The adapter's analysis dialect name, such as `postgres`.
    fn name(&self) -> &'static str;

    /// Every statement of `sql` with the parser's named gaps, or the first error.
    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap>;

    /// The dialect's sqlquack-semantics profile: name resolution, typing and scope rules.
    fn profile(&self) -> &'static dyn DialectProfile;

    /// A type as written in this dialect, spelt as SQLBuild compares types; `None` when unknown.
    fn normalize_type(&self, spelling: &str) -> Option<String>;

    /// Whether the native parser reads `sql` whole: one statement, no gap and no unparsed bytes.
    fn read_whole(&self, sql: &str) -> Result<(), ParserGap>;

    /// The engine-accepted prefix of UDF and cursor stubs: `__sqlbuild_`, or `__sqlbuild.` for T-SQL.
    fn stub_prefix(&self) -> &'static str;
}
