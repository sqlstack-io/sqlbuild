use sqlquack_syntax::expressions::statements::models::Command;
use sqlquack_syntax::{Expression, Parsed};

use crate::analysis_route::models::{LocatedGap, ParserGap, TypeMatch};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
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

/// A test dialect over the PostgreSQL parser that also refuses PIVOT, unfilled placeholders and
/// one-part stubs, keeps SQL after `RAW ` as raw text, and refuses the type `NOT_A_TYPE`.
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
        if let Some(text) = sql.strip_prefix("RAW ") {
            return Ok(Parsed {
                statements: vec![Expression::Command(Box::new(Command {
                    this: text.to_owned(),
                }))],
                gaps: Vec::new(),
            });
        }
        if let Some((offset, word)) = refused {
            return Err(ParserGap {
                construct: word.to_owned(),
                message: word.to_owned(),
                offset,
                syntax: false,
            });
        }
        sqlquack_syntax_postgres::parse(sql).map_err(|error| ParserGap {
            construct: error.message.clone(),
            message: error.message,
            offset: error.position,
            syntax: error.syntax,
        })
    }

    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        (!spelling.eq_ignore_ascii_case("NOT_A_TYPE")).then(|| normalize_text_type(spelling, None))
    }
}

pub(super) struct ReadWholeTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) expected_gap: Option<&'static str>,
}

pub(super) struct NativeTypeMatchTestCase {
    pub(super) description: &'static str,
    pub(super) declared: &'static str,
    pub(super) inferred: &'static str,
    pub(super) expected_match: TypeMatch,
}

pub(super) struct TypeMatchTestCase {
    pub(super) description: &'static str,
    pub(super) declared: &'static str,
    pub(super) inferred: &'static str,
    pub(super) inferred_nullable: bool,
    pub(super) expected_match: TypeMatch,
}
