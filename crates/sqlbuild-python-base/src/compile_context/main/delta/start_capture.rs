//! Start capturing image facts for the compile about to run.

use crate::compile_context::_helpers::delta;

/// Start capturing image facts for the compile about to run.
#[inline]
pub fn start_capture() {
    delta::start_capture()
}
