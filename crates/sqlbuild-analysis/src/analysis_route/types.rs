//! What a dialect's native analysis module provides.

use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::{Expression, Parsed};

use crate::analysis_route::_helpers::native_analysis::native_analysis;
use crate::analysis_route::constants::{
    MACRO_BODY_PREFIX, MORE_THAN_ONE_STATEMENT, NO_STATEMENT, NOT_ONE_EXPRESSION,
    UNPARSED_STATEMENT,
};
use crate::analysis_route::models::{AnalysisInput, NativeAnalysis, ParserGap, TypeMatch};
use crate::type_system::models::NormalizedType;

/// A dialect whose SQL SQLQuack analyses in place of Polyglot.
pub trait NativeDialect: Sync {
    /// The adapter's analysis dialect name, such as `postgres`.
    fn name(&self) -> &'static str;

    /// The engine-accepted prefix of UDF and cursor stubs: `__sqlbuild_`, or `__sqlbuild.` for T-SQL.
    fn stub_prefix(&self) -> &'static str;

    /// Every statement of `sql` with the parser's named gaps, or the first error that stops it.
    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap>;

    /// The dialect's sqlquack-semantics profile: how its names resolve, compare and type.
    fn profile(&self) -> &'static dyn DialectProfile;

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
        match (statements.next(), statements.next()) {
            (None, _) => Err(whole_gap(NO_STATEMENT, 0)),
            (Some(_), Some(_)) => Err(whole_gap(MORE_THAN_ONE_STATEMENT, 0)),
            (Some(Expression::Command(_)), None) => Err(whole_gap(UNPARSED_STATEMENT, 0)),
            (Some(statement), None) => Ok(statement),
        }
    }

    /// The outputs, lineage and findings of the query `input.sql`, read whole, against
    /// `input.schema`; findings are in SQLBuild's codes, as the Polyglot path reports them.
    fn analyse(&self, input: &AnalysisInput<'_>) -> Result<NativeAnalysis, ParserGap> {
        let statement: Expression = self.read_whole(input.sql)?;
        Ok(native_analysis(self, &statement, input))
    }

    /// A macro body's one expression, read as the projection of `SELECT <body>`; a gap's offset
    /// is in `body`.
    fn macro_body(&self, body: &str) -> Result<Expression, ParserGap> {
        let statement: Expression = self
            .read_whole(&format!("{MACRO_BODY_PREFIX}{body}"))
            .map_err(|gap| ParserGap {
                offset: gap.offset.saturating_sub(MACRO_BODY_PREFIX.len()),
                ..gap
            })?;
        let Expression::Select(select) = statement else {
            return Err(whole_gap(NOT_ONE_EXPRESSION, 0));
        };
        if select.from.is_some() {
            return Err(whole_gap(NOT_ONE_EXPRESSION, 0));
        }
        match <[Expression; 1]>::try_from(select.expressions) {
            Ok([expression]) => Ok(expression),
            Err(_) => Err(whole_gap(NOT_ONE_EXPRESSION, 0)),
        }
    }

    /// Whether a declared and an inferred type match, nullability apart; `NotCompared` when the
    /// inferred type is unknown or `normalize_type` gives `None` for either side. An override must
    /// keep that rule, and may still say whether the declared type alone forbids null.
    fn type_match(
        &self,
        declared: &str,
        inferred: Option<&str>,
        inferred_nullable: bool,
    ) -> TypeMatch {
        let _ = inferred_nullable;
        match (
            self.normalize_type(declared),
            inferred.and_then(|inferred| self.normalize_type(inferred)),
        ) {
            (Some(declared), Some(inferred)) => TypeMatch::Compared {
                same_type: declared == inferred,
                nullable_into_non_null: false,
            },
            _ => TypeMatch::NotCompared {
                nullable_into_non_null: false,
            },
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
