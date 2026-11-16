//! Hold the delta's result for comparison once the full compile finishes.

use crate::compile_delta::_helpers::verify::ACTUAL;
use crate::compile_delta::_helpers::verify::EXPECTED;
use crate::compile_delta::_helpers::verify::locked;
use crate::compile_delta::models::Expected;

/// Hold the delta's result for comparison once the full compile finishes.
pub fn hold_expected(expected: Expected) {
    *locked(&EXPECTED) = Some(expected);
    *locked(&ACTUAL) = None;
}
