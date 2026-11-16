//! Microbatch guidance.

use crate::discovery::_helpers::failures::guidance;

/// Microbatch guidance.
#[inline]
pub fn microbatch_guidance() -> (String, String) {
    guidance::microbatch_guidance()
}
