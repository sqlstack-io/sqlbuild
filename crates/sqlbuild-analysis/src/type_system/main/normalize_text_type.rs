//! `normalize_text_type`: a type's comparison shape read from its text alone, without Polyglot.

use crate::type_system::_helpers::fallback::normalize_with_fallback;
use crate::type_system::models::{NormalizedType, TypeDialect};

/// `type_sql`'s name, family and parameters as the text fallback reads them under `dialect`.
#[must_use]
pub fn normalize_text_type(type_sql: &str, dialect: Option<TypeDialect>) -> NormalizedType {
    normalize_with_fallback(type_sql, dialect)
}
