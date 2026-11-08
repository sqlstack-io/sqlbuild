//! What a dialect's native analysis module provides.

use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::{Expression, Parsed};

use crate::analysis_route::constants::{STATEMENTS_CONSTRUCT, UNPARSED_CONSTRUCT};
use crate::analysis_route::models::ParserGap;

/// A dialect whose SQL SQLQuack parses and analyses in place of Polyglot.
pub trait NativeDialect: Sync {
    /// The adapter's analysis dialect name, such as `postgres`.
    fn name(&self) -> &'static str;

    /// The engine-accepted prefix of UDF and cursor stubs: `__sqlbuild_`, or `__sqlbuild.` for T-SQL.
    fn stub_prefix(&self) -> &'static str;

    /// Every statement of `sql` with the parser's named gaps, or the first error.
    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap>;

    /// The dialect's sqlquack-semantics profile: name resolution, typing and scope rules.
    fn profile(&self) -> &'static dyn DialectProfile;

    /// A type as written in this dialect, spelt as SQLBuild compares types; `None` when unknown.
    fn normalize_type(&self, spelling: &str) -> Option<String>;

    /// One statement, no gaps, every byte covered, and not a lone `Command` (text a parser kept
    /// unparsed): `parse`, then those rules.
    fn read_whole(&self, sql: &str) -> Result<Expression, ParserGap> {
        let mut parsed: Parsed = self.parse(sql)?;
        if let Some(gap) = parsed.gaps.first() {
            return Err(ParserGap {
                construct: gap.construct.to_owned(),
                message: gap.construct.to_owned(),
                offset: gap.start,
                syntax: false,
            });
        }
        match (parsed.statements.pop(), parsed.statements.is_empty()) {
            (Some(Expression::Command(_)), true) => Err(ParserGap {
                construct: UNPARSED_CONSTRUCT.to_owned(),
                message: UNPARSED_CONSTRUCT.to_owned(),
                offset: 0,
                syntax: false,
            }),
            (Some(statement), true) => Ok(statement),
            _ => Err(ParserGap {
                construct: STATEMENTS_CONSTRUCT.to_owned(),
                message: STATEMENTS_CONSTRUCT.to_owned(),
                offset: sql.len(),
                syntax: false,
            }),
        }
    }
}
