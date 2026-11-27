//! Whether a header string is the value of a `description` key, which holds prose.

use crate::header_strings::models::DescriptionKeys;

/// Whether the string at `start` directly follows a `description` key, in any letter case.
pub fn follows_description_key<T: Copy + PartialEq + From<u8> + Into<u32>>(
    text: &[T],
    start: usize,
) -> bool {
    DescriptionKeys::new(text).follows(start)
}
