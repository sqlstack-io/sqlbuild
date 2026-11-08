//! What a dialect's native analysis module provides.

use crate::analysis_route::models::ParserGap;

/// A dialect whose SQL SQLQuack analyses in place of Polyglot.
pub trait NativeDialect: Sync {
    /// The adapter's analysis dialect name, such as `postgres`.
    fn name(&self) -> &'static str;

    /// Whether the native parser reads `sql` whole: one statement, no gap and no unparsed bytes.
    fn read_whole(&self, sql: &str) -> Result<(), ParserGap>;

    /// The engine-accepted prefix of UDF and cursor stubs: `__sqlbuild_`, or `__sqlbuild.` for T-SQL.
    fn stub_prefix(&self) -> &'static str;
}
