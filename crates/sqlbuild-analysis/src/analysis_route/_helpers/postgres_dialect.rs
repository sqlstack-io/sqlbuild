//! PostgreSQL's native module: SQLQuack's PostgreSQL 18 parser reads its SQL.

use sqlquack_semantics::dialects::constants::POSTGRES;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax_postgres::{Parsed, StrictFailure, format_type, parse_strict, parse_type};

use crate::analysis_route::constants::{
    POSTGRES_DIALECT, POSTGRES_STUB_PREFIX, POSTGRES_TEMPORAL_SPELLINGS, POSTGRES_TYPE_SPELLINGS,
    UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{ParserGap, PostgresDialect};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeDialect};

impl NativeDialect for PostgresDialect {
    fn name(&self) -> &'static str {
        POSTGRES_DIALECT
    }

    fn stub_prefix(&self) -> &'static str {
        POSTGRES_STUB_PREFIX
    }

    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        parse_strict(sql).map_err(|failure| match failure {
            StrictFailure::Parse(error) => ParserGap {
                construct: error.message.clone(),
                message: error.message,
                offset: error.position,
                syntax: error.syntax,
            },
            StrictFailure::Uncovered { position } => ParserGap {
                construct: UNSCANNED_CONSTRUCT.to_owned(),
                message: UNSCANNED_CONSTRUCT.to_owned(),
                offset: position,
                syntax: false,
            },
        })
    }

    fn profile(&self) -> &'static dyn DialectProfile {
        &POSTGRES
    }

    /// The comparison shape of the type PostgreSQL names; `None` (unknown) when it cannot name it.
    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        let canonical: String = format_type(&parse_type(spelling).ok()?)?;
        Some(normalize_text_type(
            &fallback_spelling(&canonical),
            Some(TypeDialect::Postgres),
        ))
    }
}

/// PostgreSQL's `format_type` name with its multi-word names in the words SQLBuild's fallback reads.
fn fallback_spelling(canonical: &str) -> String {
    for (name, zone, respelt) in POSTGRES_TEMPORAL_SPELLINGS {
        if let Some(precision) = canonical
            .strip_prefix(name)
            .and_then(|rest| rest.strip_suffix(zone))
            .filter(|precision| precision.is_empty() || precision.starts_with('('))
        {
            return format!("{respelt}{precision}");
        }
    }
    for (name, respelt) in POSTGRES_TYPE_SPELLINGS {
        if let Some(rest) = canonical
            .strip_prefix(name)
            .filter(|rest| rest.is_empty() || rest.starts_with(['(', '[']))
        {
            return format!("{respelt}{rest}");
        }
    }
    canonical.to_owned()
}
