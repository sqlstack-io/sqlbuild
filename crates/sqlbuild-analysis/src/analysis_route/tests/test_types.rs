use sqlquack_semantics::dialects::constants::POSTGRES;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::expressions::names::models::Identifier;
use sqlquack_syntax::{Expression, Parsed};

use crate::analysis_route::models::{LocatedGap, ParserGap, TypeMatch};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::_helpers::fallback::normalize_with_fallback;
use crate::type_system::models::NormalizedType;

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

    fn stub_prefix(&self) -> &'static str {
        "__sqlbuild."
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        let refused = ["PIVOT", "@@@", "__sqlbuild_udf_", "__cursor_", "@'"]
            .iter()
            .filter_map(|word| sql.find(word).map(|offset| (offset, *word)))
            .min();
        match refused {
            Some((offset, word)) => Err(ParserGap {
                construct: word.to_owned(),
                message: word.to_owned(),
                offset,
                syntax: false,
            }),
            None => Ok(Parsed {
                statements: vec![Expression::Identifier(Identifier::new(sql.to_owned()))],
                gaps: Vec::new(),
            }),
        }
    }

    fn profile(&self) -> &'static dyn DialectProfile {
        &POSTGRES
    }

    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        Some(normalize_with_fallback(spelling, None))
    }
}

pub(super) struct TypeMatchTestCase {
    pub(super) description: &'static str,
    pub(super) declared: &'static str,
    pub(super) inferred: &'static str,
    pub(super) inferred_nullable: bool,
    pub(super) expected_match: TypeMatch,
}

pub(super) struct ReadWholeTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) expected_gap: Option<(&'static str, usize, bool)>,
}

pub(super) struct NormalizeTypeTestCase {
    pub(super) description: &'static str,
    pub(super) spelling: &'static str,
}
