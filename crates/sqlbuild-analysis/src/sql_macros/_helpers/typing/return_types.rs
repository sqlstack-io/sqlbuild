//! A SQL macro's declared `returns` type checked against the adapter's SQL analysis dialect.

use std::num::IntErrorKind;

use crate::analysis_route::types::NativeDialect;
use crate::sql_macros::_helpers::typing::returns_check::adapter_dialect;
use crate::sql_macros::constants::{
    BIGQUERY_BIG_DECIMAL_NAMES, BIGQUERY_BIGNUMERIC_LIMITS, BIGQUERY_DIALECT,
    BIGQUERY_NUMERIC_LIMITS, DECIMAL_PRECISION_LIMITS,
};
use crate::type_system::models::{NormalizedType, PythonInteger, TypeFamily};

/// Why `returns` is not a type the adapter's SQL analysis dialect reads and can hold.
pub(crate) fn returns_type_error(
    returns: &str,
    (adapter, parser): (&str, &dyn NativeDialect),
    context: &str,
) -> Option<String> {
    let dialect_name: &str = adapter_dialect(adapter);
    let reason: String = match parser.normalize_type(returns) {
        Some(normalized) => decimal_error(&normalized, parser.name())
            .or_else(|| bigquery_numeric_error(returns, &normalized, parser.name()))?,
        None => format!("the {} parser does not know this type", parser.name()),
    };
    Some(format!(
        "{context} type '{returns}' is not valid for adapter '{adapter}' SQL analysis dialect \
         '{dialect_name}': {reason}"
    ))
}

/// The decimal's precision and scale, where `normalized` is a decimal with a stated precision.
fn decimal_digits(normalized: &NormalizedType) -> Option<(u64, u64)> {
    if normalized.family != TypeFamily::Decimal {
        return None;
    }
    Some((
        normalized.precision.as_ref().and_then(digit_count)?,
        normalized.scale.as_ref().and_then(digit_count).unwrap_or(0),
    ))
}

/// A stated precision or scale; one too large for `u64` counts as `u64::MAX`, past every limit.
fn digit_count(value: &PythonInteger) -> Option<u64> {
    match value.as_str().parse::<u64>() {
        Ok(count) => Some(count),
        Err(error) if *error.kind() == IntErrorKind::PosOverflow => Some(u64::MAX),
        Err(_) => None,
    }
}

/// Why a BigQuery `NUMERIC(P, S)` or `BIGNUMERIC(P, S)` has more digits than the type holds.
fn bigquery_numeric_error(
    type_sql: &str,
    normalized: &NormalizedType,
    dialect: &str,
) -> Option<String> {
    if dialect != BIGQUERY_DIALECT {
        return None;
    }
    let (precision, scale) = decimal_digits(normalized)?;
    let big: bool = BIGQUERY_BIG_DECIMAL_NAMES
        .iter()
        .any(|name| type_sql.trim_start().to_ascii_uppercase().starts_with(name));
    let (max_integer_digits, max_scale) = if big {
        BIGQUERY_BIGNUMERIC_LIMITS
    } else {
        BIGQUERY_NUMERIC_LIMITS
    };
    let integer_digits: u64 = precision.saturating_sub(scale);
    (integer_digits > u64::from(max_integer_digits) || scale > u64::from(max_scale)).then(|| {
        format!(
            "precision {precision} and scale {scale} exceed BigQuery's limits of {max_integer_digits} \
             integer digits and scale {max_scale}"
        )
    })
}

/// Why a decimal has a precision or scale `dialect` cannot hold.
fn decimal_error(normalized: &NormalizedType, dialect: &str) -> Option<String> {
    let (precision, scale) = decimal_digits(normalized)?;
    if scale > precision {
        return Some(format!(
            "DECIMAL scale {scale} is larger than its precision {precision}"
        ));
    }
    let (_, max) = DECIMAL_PRECISION_LIMITS
        .iter()
        .find(|(name, _)| *name == dialect)?;
    (precision == 0 || precision > *max)
        .then(|| format!("DECIMAL precision {precision} exceeds the target range 1..={max}"))
}
