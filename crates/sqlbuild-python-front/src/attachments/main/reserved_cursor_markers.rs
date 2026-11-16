//! The reserved executor bound markers no authored SQL may contain.

use crate::attachments::_helpers::cursor_intrinsics::RESERVED_CURSOR_MARKERS;

/// The reserved executor bound markers no authored SQL may contain.
pub fn reserved_cursor_markers() -> Vec<String> {
    RESERVED_CURSOR_MARKERS
        .iter()
        .map(|marker| (*marker).to_owned())
        .collect()
}
