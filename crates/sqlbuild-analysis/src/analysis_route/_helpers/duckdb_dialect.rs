//! DuckDB's native module; its parser arm joins once the native DuckDB parser lands.

use crate::analysis_route::_helpers::type_spelling::respelt;
use crate::analysis_route::constants::DUCKDB_TYPE_SPELLING;
use crate::analysis_route::models::DuckDbDialect;
use crate::type_system::main::normalize_text_type::normalize_text_type;
use crate::type_system::models::{NormalizedType, TypeDialect};

impl DuckDbDialect {
    /// The type as Polyglot normalised it, or `None` (unknown) where the text fallback can't match it.
    #[must_use]
    pub fn normalize_type(&self, spelling: &str) -> Option<NormalizedType> {
        respelt(spelling, &DUCKDB_TYPE_SPELLING)
            .map(|respelt| normalize_text_type(&respelt, Some(TypeDialect::DuckDb)))
    }
}
