//! Python's `normalize_type`: the parsed type first, then the text fallback.

use crate::type_system::_helpers::dialects::coerce_type_dialect;
use crate::type_system::_helpers::fallback::normalize_with_fallback;
use crate::type_system::_helpers::parsed::{nesting_bound, normalize_parsed, parse_type};
use crate::type_system::_helpers::spelling::SpellingDialect;
use crate::type_system::constants::{DEEP_TYPE_STACK_BYTES, SHALLOW_TYPE_NESTING};
use crate::type_system::errors::TypeNormalizationError;
use crate::type_system::models::{TypeDialect, TypeNormalization};

/// Normalize one type in the named dialect.
pub fn normalize_type(
    type_sql: &str,
    dialect: &str,
) -> Result<TypeNormalization, TypeNormalizationError> {
    let spelling: SpellingDialect = SpellingDialect::named(dialect)
        .ok_or_else(|| TypeNormalizationError::UnknownDialect(dialect.to_owned()))?;
    let type_dialect: Option<TypeDialect> = coerce_type_dialect(dialect);
    let normalize = || match parse_type(type_sql, spelling) {
        Ok(tree) => TypeNormalization {
            normalized: normalize_parsed(&tree, spelling, type_dialect),
            parse_error: None,
        },
        Err(error) => TypeNormalization {
            normalized: normalize_with_fallback(type_sql, type_dialect),
            parse_error: Some(error),
        },
    };
    Ok(if nesting_bound(type_sql) > SHALLOW_TYPE_NESTING {
        stacker::grow(DEEP_TYPE_STACK_BYTES, normalize)
    } else {
        normalize()
    })
}
