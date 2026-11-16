//! The whole project's notable facts: `stored` with the fresh models' replaced by `fresh`.

use crate::compile_context::_helpers::coverage;
use crate::compile_context::models::ModelCoverage;

/// The whole project's notable facts: `stored` with the fresh models' replaced by `fresh`.
#[inline]
pub fn merged_coverage(stored: &[ModelCoverage], fresh: &[ModelCoverage]) -> Vec<ModelCoverage> {
    coverage::merged_coverage(stored, fresh)
}
