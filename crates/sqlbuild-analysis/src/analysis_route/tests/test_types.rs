use sqlquack_semantics::dialects::constants::{DUCKDB, POSTGRES};
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax::expressions::statements::models::Command;
use sqlquack_syntax::{Dialect, Expression, ParseOptions, Parsed, parse_strict};

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

/// The PostgreSQL parser, refusing PIVOT, placeholders, one-part stubs and the type `NOT_A_TYPE`.
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

    fn profile(&self) -> &'static dyn DialectProfile {
        &POSTGRES
    }

    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        (!spelling.eq_ignore_ascii_case("NOT_A_TYPE")).then(|| normalize_text_type(spelling, None))
    }
}

/// DuckDB's shared parser under the DuckDB profile, as MotherDuck projects are analysed.
pub(super) struct CataloguedDialect;

impl NativeDialect for CataloguedDialect {
    fn name(&self) -> &'static str {
        "duckdb"
    }

    fn stub_prefix(&self) -> &'static str {
        "__sqlbuild_"
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        parse_strict(sql, Dialect::DuckDB, &ParseOptions::default())
            .map(|statements| Parsed {
                statements,
                gaps: Vec::new(),
            })
            .map_err(|failure| ParserGap {
                construct: failure.to_string(),
                message: failure.to_string(),
                offset: 0,
                syntax: false,
            })
    }

    fn profile(&self) -> &'static dyn DialectProfile {
        &DUCKDB
    }

    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        PivotlessDialect.normalize_type(spelling)
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
    pub(super) inferred: Option<&'static str>,
    pub(super) expected_match: TypeMatch,
}

pub(super) struct TypeMatchTestCase {
    pub(super) description: &'static str,
    pub(super) declared: &'static str,
    pub(super) inferred: Option<&'static str>,
    pub(super) inferred_nullable: bool,
    pub(super) expected_match: TypeMatch,
}

pub(super) struct FindingTestCase {
    pub(super) description: &'static str,
    pub(super) dialect: &'static dyn NativeDialect,
    pub(super) sql: &'static str,
    pub(super) extension_functions: &'static [&'static str],
    /// The code of each finding in order, with its line and column when the parse placed it.
    pub(super) expected_findings: &'static [(&'static str, Option<(usize, usize)>)],
}

pub(super) struct MacroBodyTestCase {
    pub(super) description: &'static str,
    pub(super) body: &'static str,
    /// `(construct, offset in the body)` of the gap, when the body is not read.
    pub(super) expected_gap: Option<(&'static str, usize)>,
}

pub(super) struct UnavailableAnalysisTestCase {
    pub(super) description: &'static str,
    pub(super) dialect: Option<&'static str>,
    pub(super) expected_message: &'static str,
}

/// Each output's name, transform code, confidence code and the columns it reads.
type ExpectedLineage = &'static [(&'static str, u8, u8, &'static [&'static str])];

pub(super) struct LineageTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    pub(super) expected_lineage: ExpectedLineage,
    pub(super) expected_row_sources: &'static [(&'static str, &'static str)],
}

pub(super) struct ColumnTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    /// Each output's name, type and nullability.
    pub(super) expected_columns: &'static [(&'static str, Option<&'static str>, &'static str)],
}

pub(super) struct StarTestCase {
    pub(super) description: &'static str,
    pub(super) sql: &'static str,
    /// Whether it projects a star, whether every star column is known, and the column count.
    pub(super) expected_star: (bool, bool, usize),
}
