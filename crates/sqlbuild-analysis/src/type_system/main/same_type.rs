//! `same_type`: equal normalized types, a Snowflake temporal precision left unstated matching any.

use crate::type_system::_helpers::dialects::coerce_type_dialect;
use crate::type_system::constants::{
    SNOWFLAKE_PRECISION_TEMPORAL_TYPES, TYPE_PARAMETERS_CLOSE, TYPE_PARAMETERS_OPEN,
    TYPE_WORD_SEPARATOR,
};
use crate::type_system::models::{NormalizedType, TypeDialect};

/// Whether `left` and `right` name the same type under `dialect`, up to an unstated precision.
#[must_use]
pub fn same_type(left: &NormalizedType, right: &NormalizedType, dialect: &str) -> bool {
    if left == right {
        return true;
    }
    if coerce_type_dialect(dialect) != Some(TypeDialect::Snowflake) {
        return false;
    }
    let (Some((left_base, left_precision)), Some((right_base, right_precision))) = (
        temporal_parts(&left.normalized_name),
        temporal_parts(&right.normalized_name),
    ) else {
        return false;
    };
    left_base == right_base
        && (left_precision.is_none()
            || right_precision.is_none()
            || left_precision == right_precision)
}

/// A Snowflake time or timestamp type's compact base name and its stated precision text.
fn temporal_parts(name: &str) -> Option<(String, Option<&str>)> {
    let (base, parameters): (&str, Option<&str>) = match name.split_once(TYPE_PARAMETERS_OPEN) {
        Some((base, rest)) => (base, rest.strip_suffix(TYPE_PARAMETERS_CLOSE)),
        None => (name, None),
    };
    let compact: String = base.trim().replace(TYPE_WORD_SEPARATOR, "");
    SNOWFLAKE_PRECISION_TEMPORAL_TYPES
        .contains(&compact.as_str())
        .then(|| (compact, parameters.map(str::trim)))
}
