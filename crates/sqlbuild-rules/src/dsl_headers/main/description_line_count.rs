//! `description_line_count`: the lines a description takes once wrapped at the line width.

use crate::dsl_headers::_helpers::text::line_count;
use crate::dsl_headers::_helpers::wrap::wrap_description;

/// The lines `description` takes wrapped at `line_width`, as the description-length rule counts them.
pub fn description_line_count(description: &str, line_width: i64) -> usize {
    let characters: Vec<char> = description.chars().collect();
    line_count(&wrap_description(&characters, line_width, line_width))
}
