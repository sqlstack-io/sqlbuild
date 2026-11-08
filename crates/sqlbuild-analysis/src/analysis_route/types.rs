//! What a dialect's native analysis module provides.

use sqlquack_syntax::{Expression, Parsed};

use crate::analysis_route::constants::{STATEMENTS_CONSTRUCT, UNPARSED_STATEMENT_CONSTRUCT};
use crate::analysis_route::models::ParserGap;

/// A dialect whose SQL SQLQuack analyses in place of Polyglot.
pub trait NativeDialect: Sync {
    /// The adapter's analysis dialect name, such as `postgres`.
    fn name(&self) -> &'static str;

    /// Every statement of `sql` with the parser's named gaps, or the first error.
    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap>;

    /// The one statement of `sql`, with no gap and not a lone unparsed command.
    fn read_whole(&self, sql: &str) -> Result<Expression, ParserGap> {
        let mut parsed: Parsed = self.parse(sql)?;
        if let Some(gap) = parsed.gaps.first() {
            return Err(ParserGap {
                construct: gap.construct.to_owned(),
                offset: gap.start,
            });
        }
        let unread = |construct: &str| ParserGap {
            construct: construct.to_owned(),
            offset: sql.len(),
        };
        if parsed.statements.len() != 1 {
            return Err(unread(STATEMENTS_CONSTRUCT));
        }
        match parsed.statements.remove(0) {
            Expression::Command(_) => Err(unread(UNPARSED_STATEMENT_CONSTRUCT)),
            statement => Ok(statement),
        }
    }

    /// The engine-accepted prefix of UDF and cursor stubs: `__sqlbuild_`, or `__sqlbuild.` for T-SQL.
    fn stub_prefix(&self) -> &'static str;
}
