//! The previous compile's digest of a custom-rule fact about a resource the delta did not rerun.

use crate::compile_context::_helpers::delta;

/// The previous compile's digest of a custom-rule fact about a resource the delta did not rerun.
#[inline]
pub fn injected_fact_digest(key: &[String]) -> Option<String> {
    delta::injected_fact_digest(key)
}
