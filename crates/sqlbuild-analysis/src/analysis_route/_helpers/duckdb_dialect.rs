//! DuckDB's native module; its parser arm joins once the native DuckDB parser lands.

use sqlquack_syntax::{Dialect, Parsed};

use crate::analysis_route::_helpers::snowflake_dialect::shared_parsed;
use crate::analysis_route::_helpers::type_spelling::respelt;
use crate::analysis_route::_helpers::whole::one_statement;
use crate::analysis_route::constants::{DUCKDB_DIALECT, DUCKDB_TYPE_SPELLING, UNQUOTED_STUB_PREFIX};
use crate::analysis_route::models::{DuckDbDialect, ParserGap};
use crate::analysis_route::types::NativeDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeDialect};

impl NativeDialect for DuckDbDialect {
    fn name(&self) -> &'static str {
        DUCKDB_DIALECT
    }

    fn read_whole(&self, sql: &str) -> Result<(), ParserGap> {
        let parsed: Parsed = self.parse(sql)?;
        one_statement(&parsed.statements, &parsed.gaps, sql.len())
    }

    fn stub_prefix(&self) -> &'static str {
        UNQUOTED_STUB_PREFIX
    }

    /// Validation placeholder over the shared parser's DuckDB mode, not for merging: the arm's
    /// parser is the native DuckDB one.
    fn parse(&self, sql: &str) -> Result<Parsed, ParserGap> {
        shared_parsed(sql, Dialect::DuckDB)
    }

    /// The type as Polyglot normalised it, or `None` (unknown) where the text fallback can't match it.
    fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        respelt(spelling, &DUCKDB_TYPE_SPELLING)
            .map(|respelt| normalize_text_type(&respelt, Some(TypeDialect::DuckDb)))
    }
}
