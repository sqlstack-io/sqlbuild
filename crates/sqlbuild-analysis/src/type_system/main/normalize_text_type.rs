//! Python's `_normalize_with_fallback`, for callers that read type text without Polyglot.

use crate::type_system::_helpers::fallback::normalize_with_fallback;
use crate::type_system::models::{NormalizedType, TypeDialect};

/// Normalize type text without a parser, as SQLBuild's fallback does for `dialect`.
#[must_use]
pub fn normalize_text_type(type_sql: &str, dialect: Option<TypeDialect>) -> NormalizedType {
    normalize_with_fallback(type_sql, dialect)
}
