//! Every project model with its upstream names, the fresh models' replacing the stored.

use crate::compile_context::_helpers::coverage;

/// Every project model with its upstream names, the fresh models' replacing the stored.
#[inline]
pub fn merged_order(
    stored: &[(String, Vec<String>)],
    fresh: &[(String, Vec<String>)],
) -> Vec<(String, Vec<String>)> {
    coverage::merged_order(stored, fresh)
}
