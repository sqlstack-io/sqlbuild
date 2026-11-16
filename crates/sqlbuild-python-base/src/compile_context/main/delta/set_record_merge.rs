//! Hand the next reuse record what it keeps of the patched compile.

use crate::compile_context::_helpers::delta;
use crate::compile_context::models::RecordMerge;

/// Hand the next reuse record what it keeps of the patched compile.
#[inline]
pub fn set_record_merge(merge: Option<RecordMerge>) {
    delta::set_record_merge(merge)
}
