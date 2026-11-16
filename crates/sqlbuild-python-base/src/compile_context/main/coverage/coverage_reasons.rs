//! Each model's partial-check reasons, in `order`, which pairs each model with its upstream.

use crate::compile_context::_helpers::coverage;
use crate::compile_context::models::ModelCoverage;

/// Each model's partial-check reasons, in `order`, which pairs each model with its upstream.
#[inline]
pub fn coverage_reasons(
    order: &[(String, Vec<String>)],
    facts: &[ModelCoverage],
) -> Vec<(String, Vec<String>)> {
    coverage::coverage_reasons(order, facts)
}
