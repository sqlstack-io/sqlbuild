//! Start capturing described source columns; only a build arms it, so `sqb plan` records nothing.

use crate::observations::_helpers::source_capture;

/// Start capturing described source columns; only a build arms it, so `sqb plan` records nothing.
#[inline]
pub fn arm_source_capture() {
    source_capture::arm_source_capture()
}
