//! What the next reuse record keeps of the patched compile, taken.

use crate::compile_context::_helpers::delta;
use crate::compile_context::models::RecordMerge;

/// What the next reuse record keeps of the patched compile, taken.
#[inline]
pub fn take_record_merge() -> Option<RecordMerge> {
    delta::take_record_merge()
}
