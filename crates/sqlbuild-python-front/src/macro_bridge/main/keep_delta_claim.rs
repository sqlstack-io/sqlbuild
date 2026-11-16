//! Keep the delta compile's claim: it finished, so no full compile of this invocation follows.

use crate::macro_bridge::_helpers::first_compile;

/// Keep the delta compile's claim: it finished, so no full compile of this invocation follows.
#[inline]
pub fn keep_delta_claim() {
    first_compile::keep_delta_claim()
}
