//! Stop capturing without keeping anything, for a project that turns delta compiles off.

use crate::compile_context::_helpers::delta;

/// Stop capturing without keeping anything, for a project that turns delta compiles off.
#[inline]
pub fn stop_capture() {
    delta::stop_capture()
}
