//! Whether the running compile captures image facts.

use crate::compile_context::_helpers::delta;

/// Whether the running compile captures image facts.
#[inline]
pub fn capturing() -> bool {
    delta::capturing()
}
