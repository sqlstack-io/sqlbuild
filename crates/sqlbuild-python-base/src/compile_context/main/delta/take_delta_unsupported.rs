//! Take the reason the delta compile was ruled out, if any.

use crate::compile_context::_helpers::delta;

/// Take the reason the delta compile was ruled out, if any.
#[inline]
pub fn take_delta_unsupported() -> Option<String> {
    delta::take_delta_unsupported()
}
