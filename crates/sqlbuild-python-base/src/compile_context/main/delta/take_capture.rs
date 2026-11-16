//! Stop capturing and take what was captured.

use crate::compile_context::_helpers::delta;
use crate::compile_context::models::ImageCapture;

/// Stop capturing and take what was captured.
#[inline]
pub fn take_capture() -> Option<ImageCapture> {
    delta::take_capture()
}
