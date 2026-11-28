//! The extent of one quoted header string, for scanners that only need where it ends.

use crate::header_strings::models::DescriptionKeys;

/// The offset just past the header string opening at `start`, or `None` when it never closes.
pub fn header_quoted_end<T: Copy + PartialEq + From<u8> + Into<u32>>(
    text: &[T],
    start: usize,
) -> Option<usize> {
    DescriptionKeys::new(text).quoted_end(start)
}
