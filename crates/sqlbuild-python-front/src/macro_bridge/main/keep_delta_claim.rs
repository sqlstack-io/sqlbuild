//! Keep the delta compile's claim: it finished, so no full compile of this invocation follows.

use crate::macro_bridge::_helpers::first_compile::FIRST_COMPILE;
use crate::macro_bridge::_helpers::first_compile::locked;

/// Keep the delta compile's claim: it finished, so no full compile of this invocation follows.
pub fn keep_delta_claim() {
    locked(&FIRST_COMPILE).lent_to_delta = false;
}
