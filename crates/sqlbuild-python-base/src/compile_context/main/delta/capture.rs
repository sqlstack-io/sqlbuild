//! Record into the capture, when one is running.

use crate::compile_context::_helpers::delta;
use crate::compile_context::models::ImageCapture;

/// Record into the capture, when one is running.
#[inline]
pub fn capture(record: impl FnOnce(&mut ImageCapture)) {
    delta::capture(record)
}
