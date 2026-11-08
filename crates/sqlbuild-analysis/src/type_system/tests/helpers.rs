use crate::type_system::main::normalize_type::normalize_type;
use crate::type_system::models::{NormalizedType, PythonInteger, TypeNormalization};

pub(super) fn normalization_text(normalization: &TypeNormalization) -> String {
    let optional =
        |value: &Option<PythonInteger>| value.as_ref().map_or("-".to_owned(), ToString::to_string);
    let normalized = &normalization.normalized;
    format!(
        "{} | {} | {} | {} | {} | {}",
        normalized.normalized_name,
        normalized.family.as_str(),
        optional(&normalized.precision),
        optional(&normalized.scale),
        optional(&normalized.length),
        normalization.parse_error.as_deref().unwrap_or("-"),
    )
}

/// The normalized type of `type_sql` in `dialect`, which must be a known dialect.
pub(super) fn normalized_type(type_sql: &str, dialect: &str) -> NormalizedType {
    normalize_type(type_sql, dialect)
        .expect("the dialect is known")
        .normalized
}
