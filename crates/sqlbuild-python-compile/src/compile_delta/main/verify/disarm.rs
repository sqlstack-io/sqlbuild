//! Forget any held result.

use crate::compile_delta::_helpers::verify::ACTUAL;
use crate::compile_delta::_helpers::verify::EXPECTED;
use crate::compile_delta::_helpers::verify::locked;

/// Forget any held result.
pub fn disarm() {
    *locked(&EXPECTED) = None;
    *locked(&ACTUAL) = None;
}
