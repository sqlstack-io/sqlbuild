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

pub(super) struct ArmTypeMatchTestCase {
    pub(super) description: &'static str,
    pub(super) declared: &'static str,
    pub(super) inferred: &'static str,
    pub(super) inferred_nullable: bool,
    pub(super) expected_match: Option<TypeMatch>,
}

pub(super) struct TypeSpellingsTestCase {
    pub(super) description: &'static str,
    pub(super) spellings: &'static [&'static str],
    pub(super) expected_mismatches: &'static [&'static str],
}

pub(super) struct QueryShapeTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) expected_projections: Option<&'static [&'static str]>,
}

pub(super) struct FingerprintTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    /// Whether the fingerprint equals `READABLE`'s, or None for no fingerprint.
    pub(super) expected_same: Option<bool>,
}

pub(super) struct RawShapeTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    /// The root kind and whether it writes or locks, or None for a refusal.
    pub(super) expected_shape: Option<(&'static str, bool)>,
}

pub(super) struct RelationNamesTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    /// The qualified relations and temporary tables, or None when the SQL is not read.
    pub(super) expected_names: Option<(&'static [&'static str], &'static [&'static str])>,
}

pub(super) struct StarKeysTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) expected_relations: &'static [&'static str],
}
