//! A header syntax error's detail, help and file line (when `header_line` is known).

use crate::sqltext::_helpers::model_headers;

/// A header syntax error's detail, help and file line (when `header_line` is known).
#[inline]
pub fn header_failure(
    error: &str,
    header: &str,
    header_line: Option<usize>,
) -> (String, Option<String>, Option<usize>) {
    model_headers::header_failure(error, header, header_line)
}
