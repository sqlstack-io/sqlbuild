//! `declared_accepts`: whether a declared type accepts an inferred one (D31).

use crate::type_system::models::{NormalizedType, PythonInteger, TypeFamily};

/// Whether a declared type accepts an inferred one: equal, or a lossless NUMBER widening, an
/// inferred `NUMBER(q,s)` into a declared `NUMBER(p,s)` with `q <= p` and the same scale.
#[must_use]
pub fn declared_accepts(declared: &NormalizedType, inferred: &NormalizedType) -> bool {
    let digits = |value: &Option<PythonInteger>| -> Option<u64> {
        value
            .as_ref()
            .and_then(|value| value.as_str().parse::<u64>().ok())
    };
    declared == inferred
        || (inferred.family == TypeFamily::Decimal
            && declared.family == TypeFamily::Decimal
            && inferred.scale.is_some()
            && inferred.scale == declared.scale
            && matches!(
                (digits(&inferred.precision), digits(&declared.precision)),
                (Some(narrow), Some(wide)) if narrow <= wide
            ))
}
