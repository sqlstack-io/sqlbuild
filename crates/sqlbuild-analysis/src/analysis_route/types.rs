//! What a dialect's native analysis module provides.

use crate::analysis_route::models::ParserGap;

/// A dialect whose SQL SQLQuack analyses in place of Polyglot.
pub trait NativeDialect: Sync {
    /// The adapter's analysis dialect name, such as `postgres`, as diagnostics and normalisation read it.
    fn name(&self) -> &'static str;

    /// Whether the native parser reads `sql` whole: one statement, no gap and no unparsed bytes.
    ///
    /// # Errors
    ///
    /// The first construct the parser cannot read, and its byte offset in `sql`.
    fn read_whole(&self, sql: &str) -> Result<(), ParserGap>;
}
