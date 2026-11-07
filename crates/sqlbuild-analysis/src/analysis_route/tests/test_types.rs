use sqlquack_semantics::dialects::constants::POSTGRES;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::Parsed;

use crate::analysis_route::models::{LocatedGap, ParserGap, TypeMatch};
use crate::analysis_route::types::NativeDialect;

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

    fn read_whole(&self, sql: &str) -> Result<(), ParserGap> {
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
            None => Ok(()),
        }
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        self.read_whole(sql).map(|()| Parsed {
            statements: Vec::new(),
            gaps: Vec::new(),
        })
    }

    fn profile(&self) -> &'static dyn DialectProfile {
        &POSTGRES
    }

    fn normalize_type(&self, _spelling: &str) -> Option<String> {
        None
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

pub(super) struct NormalizeTypeTestCase {
    pub(super) spelling: &'static str,
    pub(super) expected: Option<&'static str>,
}

pub(super) struct ParseFailureTestCase {
    pub(super) description: &'static str,
    pub(super) sql: String,
    pub(super) expected_syntax: bool,
    /// The byte offset of a syntax error; a limit has none worth comparing.
    pub(super) expected_offset: Option<usize>,
}
