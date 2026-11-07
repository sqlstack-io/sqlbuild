//! DuckDB's native module; its parser arm joins once the native DuckDB parser lands.

use crate::analysis_route::_helpers::type_spelling::respelt;
use crate::analysis_route::constants::DUCKDB_TYPE_SPELLING;
use crate::analysis_route::models::DuckDbDialect;

impl DuckDbDialect {
    /// A type spelt so the type fallback gives Polyglot's result, or `None` (unknown) where it can't.
    #[must_use]
    pub fn normalize_type(&self, spelling: &str) -> Option<String> {
        respelt(spelling, &DUCKDB_TYPE_SPELLING)
    }
}
