//! What a dialect's native analysis module provides.

use sqlquack_syntax::{Expression, Parsed};

use crate::analysis_route::constants::{MORE_THAN_ONE_STATEMENT, NO_STATEMENT, UNPARSED_STATEMENT};
use crate::analysis_route::models::{ParserGap, TypeMatch};
use crate::type_system::models::NormalizedType;

/// A dialect whose SQL SQLQuack analyses in place of Polyglot.
pub trait NativeDialect: Sync {
    /// The adapter's analysis dialect name, such as `postgres`.
    fn name(&self) -> &'static str;

    /// The engine-accepted prefix of UDF and cursor stubs: `__sqlbuild_`, or `__sqlbuild.` for T-SQL.
    fn stub_prefix(&self) -> &'static str;

    /// Every statement of `sql` with the parser's named gaps, or the first error that stops it.
    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap>;

    /// A type as written in this dialect, as SQLBuild compares types; `None` when the dialect
    /// cannot spell it or its engine refuses it, and nothing is compared against it.
    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType>;

    /// The one statement of `sql` when the parser reads it whole: no gap, and not raw text.
    fn read_whole(&self, sql: &str) -> Result<Expression, ParserGap> {
        let parsed: Parsed = self.parse(sql)?;
        if let Some(gap) = parsed.gaps.first() {
            return Err(ParserGap {
                construct: gap.construct.to_owned(),
                message: gap.construct.to_owned(),
                offset: gap.start,
                syntax: false,
            });
        }
        let mut statements = parsed.statements.into_iter();
        let (Some(statement), None) = (statements.next(), statements.next()) else {
            let construct: &str = if sql.trim().is_empty() {
                NO_STATEMENT
            } else {
                MORE_THAN_ONE_STATEMENT
            };
            return Err(whole_gap(construct, 0));
        };
        match statement {
            Expression::Command(_) => Err(whole_gap(UNPARSED_STATEMENT, 0)),
            statement => Ok(statement),
        }
    }

    /// Whether a declared and an inferred type match, nullability apart; `NotCompared` when
    /// `normalize_type` gives `None` for either. An override must keep that rule.
    fn type_match(&self, declared: &str, inferred: &str, inferred_nullable: bool) -> TypeMatch {
        let _ = inferred_nullable;
        match (self.normalize_type(declared), self.normalize_type(inferred)) {
            (Some(declared), Some(inferred)) => TypeMatch::Compared {
                same_type: declared == inferred,
                nullable_into_non_null: false,
            },
            _ => TypeMatch::NotCompared,
        }
    }
}

/// A gap `read_whole` names itself, which no parser reports as invalid syntax.
fn whole_gap(construct: &str, offset: usize) -> ParserGap {
    ParserGap {
        construct: construct.to_owned(),
        message: construct.to_owned(),
        offset,
        syntax: false,
    }
}
