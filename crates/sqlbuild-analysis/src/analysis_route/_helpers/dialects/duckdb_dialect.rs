//! DuckDB's native module: SQLQuack's native DuckDB parser reads its SQL. Validation only.

use sqlquack_semantics::dialects::constants::DUCKDB;
use sqlquack_semantics::dialects::types::DialectProfile;
use sqlquack_syntax_duckdb::{Parsed, StrictFailure, parse_strict};

use crate::analysis_route::_helpers::dialects::type_spelling::respelt;
use crate::analysis_route::constants::{
    DUCKDB_DIALECT, DUCKDB_TYPE_SPELLING, UNQUOTED_STUB_PREFIX, UNSCANNED_CONSTRUCT,
};
use crate::analysis_route::models::{DuckDbDialect, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeDialect};

impl NativeDialect for DuckDbDialect {
    fn name(&self) -> &'static str {
        DUCKDB_DIALECT
    }

    fn stub_prefix(&self) -> &'static str {
        UNQUOTED_STUB_PREFIX
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
        &DUCKDB
    }

    /// The type as Polyglot normalised it, or `None` (unknown) where the text fallback can't match it.
    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        respelt(spelling, &DUCKDB_TYPE_SPELLING)
            .map(|respelt| normalize_text_type(&respelt, Some(TypeDialect::DuckDb)))
    }
}
