use crate::analysis_route::models::{LocatedGap, ParserGap, TypeMatch};
use crate::analysis_route::types::NativeDialect;
use sqlquack_syntax::expressions::names::models::Null;
use sqlquack_syntax::{Expression, Parsed};

pub(super) struct NativeGapTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) placeholders: &'static [(&'static str, &'static str)],
    pub(super) expected_gap: Option<LocatedGap>,
}

pub(super) struct RouteTestCase {
    pub(super) description: &'static str,
    pub(super) dialect: Option<&'static str>,
    pub(super) expected_legacy: bool,
}

pub(super) struct SkippedAnalysisTestCase {
    pub(super) description: &'static str,
    pub(super) model: &'static str,
    pub(super) dialect: &'static str,
    pub(super) gap: LocatedGap,
    pub(super) expected_code: &'static str,
    pub(super) expected_message: &'static str,
}

/// A test dialect whose parser refuses PIVOT, unfilled placeholders and one-part stubs.
pub(super) struct PivotlessDialect;

impl NativeDialect for PivotlessDialect {
    fn name(&self) -> &'static str {
        "postgres"
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        let refused = ["PIVOT", "@@@", "__sqlbuild_udf_", "__cursor_", "@'"]
            .iter()
            .filter_map(|word| sql.find(word).map(|offset| (offset, *word)))
            .min();
        match refused {
            Some((offset, word)) => Err(ParserGap {
                construct: word.to_owned(),
                offset,
            }),
            None => Ok(Parsed {
                statements: vec![Expression::Null(Null)],
                gaps: Vec::new(),
            }),
        }
    }

    fn stub_prefix(&self) -> &'static str {
        "__sqlbuild."
    }
}

pub(super) struct TypeMatchTestCase {
    pub(super) description: &'static str,
    pub(super) declared: &'static str,
    pub(super) inferred: &'static str,
    pub(super) inferred_nullable: bool,
    pub(super) expected_match: TypeMatch,
}
