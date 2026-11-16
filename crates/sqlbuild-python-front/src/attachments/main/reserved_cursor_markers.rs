//! The reserved executor bound markers no authored SQL may contain.

use crate::attachments::_helpers::cursor_intrinsics;

/// The reserved executor bound markers no authored SQL may contain.
#[inline]
pub fn reserved_cursor_markers() -> Vec<String> {
    cursor_intrinsics::reserved_cursor_markers()
}
