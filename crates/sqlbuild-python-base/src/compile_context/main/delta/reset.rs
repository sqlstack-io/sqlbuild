//! Forget every capture, injection and pending record fact, after a compile ends either way.

use crate::compile_context::_helpers::delta;

/// Forget every capture, injection and pending record fact, after a compile ends either way.
#[inline]
pub fn reset() {
    delta::reset()
}
